//! The seize-delay-release pattern with timeout and cancellation support.
//!
//! [`ResourceController`] coordinates several [`ResourcePool`]s: entities
//! *seize* units (all-or-nothing), *delay* while holding them, and
//! *release*; waiters queue FIFO or by priority, optional preemption evicts
//! lower-priority holders, and every pending request can be cancelled —
//! which is how a caller implements timeouts (schedule a timer event with
//! `tpt-des-core`; on fire, `cancel` the request).
//!
//! # Examples
//!
//! Seize, queue, release, and automatic re-granting:
//!
//! ```
//! use tpt_des_resource::{ResourceController, SeizeOutcome, SeizeRequest};
//!
//! let mut rc = ResourceController::new();
//! rc.add_pool(0, 1).unwrap();
//!
//! // E0 seizes the only unit.
//! assert!(matches!(rc.seize(0, SeizeRequest::new(0, 1)), Ok(SeizeOutcome::Granted(_))));
//! // E1 must wait.
//! assert!(matches!(rc.seize(1, SeizeRequest::new(0, 1)), Ok(SeizeOutcome::Queued(_))));
//!
//! // E0 releases; the controller hands the unit to E1.
//! let granted = rc.release(0, 0).unwrap();
//! assert_eq!(granted, vec![1]);
//! ```
//!
//! Deadlock detection across seized resources:
//!
//! ```
//! use tpt_des_resource::{ResourceController, SeizeRequest};
//!
//! let mut rc = ResourceController::new();
//! rc.add_pool(0, 1).unwrap();
//! rc.add_pool(1, 1).unwrap();
//! rc.seize(0, SeizeRequest::new(0, 1)).unwrap(); // E0 holds R0
//! rc.seize(1, SeizeRequest::new(1, 1)).unwrap(); // E1 holds R1
//! rc.seize(0, SeizeRequest::new(1, 1)).unwrap(); // E0 waits for R1
//! rc.seize(1, SeizeRequest::new(0, 1)).unwrap(); // E1 waits for R0
//! assert!(rc.detect_deadlock().is_some());
//! ```

use crate::deadlock::WaitForGraph;
use crate::error::ResourceError;
use crate::pool::ResourcePool;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

/// Identifies a pending seize request (for cancellation / timeouts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

/// How waiting requests compete for freed units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreemptPolicy {
    /// Waiters queue in FIFO order; no preemption ever.
    #[default]
    None,
    /// Waiters queue by ascending priority, FIFO among equals; no
    /// preemption.
    Priority,
    /// Waiters compete by priority, and a sufficiently urgent request may
    /// preempt units from lower-priority holders (least urgent holder first).
    PreemptivePriority,
}

/// A seize request: which resource, how many units, at what priority
/// (lower number = more urgent, mirroring the event calendar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeizeRequest {
    /// The resource to seize from.
    pub resource: u64,
    /// Number of units (≥ 1).
    pub amount: usize,
    /// Priority (lower = more urgent).
    pub priority: i64,
}

impl SeizeRequest {
    /// A single-unit, priority-0 request for `resource`.
    pub fn new(resource: u64, amount: usize) -> Self {
        Self {
            resource,
            amount,
            priority: 0,
        }
    }

    /// Set the priority (lower = more urgent).
    pub fn with_priority(mut self, priority: i64) -> Self {
        self.priority = priority;
        self
    }
}

/// The result of a seize attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeizeOutcome {
    /// Granted immediately. The payload lists `(entity, units)` evictions
    /// performed to satisfy the request (empty without preemption) — the
    /// caller is responsible for interrupting those entities.
    Granted(Vec<(u64, usize)>),
    /// Not enough units: the request is queued. The [`RequestId`] can be
    /// used to cancel it (this is how timeouts are implemented).
    Queued(RequestId),
}

/// One waiting (queued) request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Waiting {
    id: u64,
    entity: u64,
    resource: u64,
    amount: usize,
    priority: i64,
}

/// Coordinates resource pools, waiting requests, and deadlock detection.
pub struct ResourceController {
    pools: BTreeMap<u64, ResourcePool>,
    /// entity → resource → units held.
    holdings: BTreeMap<u64, BTreeMap<u64, usize>>,
    /// entity → base priority (for preemption decisions); default 0.
    priorities: BTreeMap<u64, i64>,
    /// Whether waiters compete by priority (Priority / Fifo policies).
    priority_queue: bool,
    /// Whether preemption is allowed (Fifo policy).
    preemptive: bool,
    /// resource → queued requests, keyed by (priority, request id). Under FIFO
    /// the priority part is always 0, so the key order is arrival order.
    waiting: BTreeMap<u64, BTreeMap<(i64, u64), Waiting>>,
    /// request id → (resource, priority key), so `cancel` finds its entry
    /// without scanning every queue.
    waiting_index: BTreeMap<u64, (u64, i64)>,
    wait_graph: WaitForGraph,
    next_request: u64,
}

impl fmt::Debug for ResourceController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceController")
            .field("pools", &self.pools)
            .field("waiting", &self.waiting_index.len())
            .finish()
    }
}

impl Default for ResourceController {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceController {
    /// An empty controller with the default [`PreemptPolicy::None`].
    pub fn new() -> Self {
        Self::with_policy(PreemptPolicy::None)
    }

    /// An empty controller with the given policy.
    pub fn with_policy(policy: PreemptPolicy) -> Self {
        Self {
            pools: BTreeMap::new(),
            holdings: BTreeMap::new(),
            priorities: BTreeMap::new(),
            priority_queue: matches!(
                policy,
                PreemptPolicy::Priority | PreemptPolicy::PreemptivePriority
            ),
            preemptive: matches!(policy, PreemptPolicy::PreemptivePriority),
            waiting: BTreeMap::new(),
            waiting_index: BTreeMap::new(),
            wait_graph: WaitForGraph::default(),
            next_request: 0,
        }
    }

    /// Register a resource with the given capacity.
    pub fn add_pool(&mut self, resource: u64, capacity: usize) -> Result<(), ResourceError> {
        let pool = ResourcePool::new(capacity)?;
        self.pools.insert(resource, pool);
        Ok(())
    }

    /// Set an entity's base priority (for preemption decisions).
    pub fn set_priority(&mut self, entity: u64, priority: i64) {
        self.priorities.insert(entity, priority);
    }

    fn priority_of(&self, entity: u64) -> i64 {
        self.priorities.get(&entity).copied().unwrap_or(0)
    }

    /// Units `entity` currently holds of `resource`.
    pub fn held(&self, entity: u64, resource: u64) -> usize {
        self.holdings
            .get(&entity)
            .and_then(|m| m.get(&resource))
            .copied()
            .unwrap_or(0)
    }

    /// Number of requests waiting for `resource`.
    pub fn waiting_count(&self, resource: u64) -> usize {
        self.waiting.get(&resource).map_or(0, BTreeMap::len)
    }

    /// Read-only access to the wait-for graph.
    pub fn wait_graph(&self) -> &WaitForGraph {
        &self.wait_graph
    }

    /// Detect a deadlock: a cycle in the wait-for graph. Returns the
    /// involved entities in cyclic order if deadlocked.
    pub fn detect_deadlock(&self) -> Option<Vec<u64>> {
        self.wait_graph.find_cycle()
    }

    /// Try to seize units for `entity` (all-or-nothing).
    ///
    /// Under [`PreemptPolicy::PreemptivePriority`], an unsatisfiable request
    /// may preempt lower-priority holders; the evictions are reported in
    /// the [`SeizeOutcome::Granted`] payload.
    ///
    /// Requests never jump ahead of waiters they should queue behind: under
    /// FIFO, any existing waiter for the resource blocks an immediate grant;
    /// under the priority policies, only waiters at least as urgent do. This
    /// prevents starvation of large requests by a stream of small ones.
    ///
    /// # Errors
    ///
    /// [`ResourceError::InvalidParameter`] if `amount` is zero or exceeds the
    /// resource's capacity (such a request could never be granted).
    /// [`ResourceError::UnknownResource`] if the resource is not registered.
    pub fn seize(
        &mut self,
        entity: u64,
        request: SeizeRequest,
    ) -> Result<SeizeOutcome, ResourceError> {
        if request.amount == 0 {
            return Err(ResourceError::InvalidParameter);
        }
        let capacity = self
            .pools
            .get(&request.resource)
            .ok_or(ResourceError::UnknownResource)?
            .capacity();
        if request.amount > capacity {
            return Err(ResourceError::InvalidParameter);
        }

        if !self.blocked_by_waiters(request.resource, request.priority)
            && self.try_take(request.resource, request.amount)
        {
            self.record_hold(entity, request.resource, request.amount);
            return Ok(SeizeOutcome::Granted(Vec::new()));
        }

        if self.preemptive {
            let evictions = self.preempt_for(entity, &request)?;
            if !evictions.is_empty() {
                // Side effect must run in release builds too: keep it outside the assert.
                let took = self.try_take(request.resource, request.amount);
                debug_assert!(took);
                self.record_hold(entity, request.resource, request.amount);
                return Ok(SeizeOutcome::Granted(evictions));
            }
        }

        // Queue the request.
        let id = RequestId(self.next_request);
        self.next_request += 1;
        let key = (self.queue_priority(request.priority), id.0);
        self.waiting.entry(request.resource).or_default().insert(
            key,
            Waiting {
                id: id.0,
                entity,
                resource: request.resource,
                amount: request.amount,
                priority: request.priority,
            },
        );
        self.waiting_index.insert(id.0, (request.resource, key.0));
        self.wait_graph.add_request(entity, request.resource);
        Ok(SeizeOutcome::Queued(id))
    }

    /// The priority part of a waiter's queue key: its priority under the
    /// priority policies, and 0 under FIFO so that only request ids order it.
    fn queue_priority(&self, priority: i64) -> i64 {
        if self.priority_queue {
            priority
        } else {
            0
        }
    }

    /// True if a waiter for `resource` must be served before a new request
    /// of `priority`: any waiter under FIFO, or one at least as urgent under
    /// the priority policies. The most urgent waiter sorts first, so one
    /// lookup decides it.
    fn blocked_by_waiters(&self, resource: u64, priority: i64) -> bool {
        match self.waiting.get(&resource).and_then(|q| q.keys().next()) {
            None => false,
            Some(&(head_priority, _)) => !self.priority_queue || head_priority <= priority,
        }
    }

    fn available_units(&self, resource: u64) -> usize {
        self.pools.get(&resource).map_or(0, ResourcePool::available)
    }

    fn try_take(&mut self, resource: u64, amount: usize) -> bool {
        self.pools
            .get_mut(&resource)
            .expect("resource existence checked by callers")
            .acquire(amount)
    }

    /// Free `request.amount` units by evicting lower-priority holders.
    /// Returns the evictions, or empty if not possible.
    fn preempt_for(
        &mut self,
        requester: u64,
        request: &SeizeRequest,
    ) -> Result<Vec<(u64, usize)>, ResourceError> {
        let available = self
            .pools
            .get(&request.resource)
            .expect("known")
            .available();
        let needed = request.amount.saturating_sub(available);
        if needed == 0 {
            return Ok(Vec::new());
        }

        // Candidates: other entities holding the resource at lower urgency.
        let mut candidates: Vec<(u64, usize, i64)> = self
            .holdings
            .iter()
            .filter_map(|(&holder, res)| res.get(&request.resource).map(|&u| (holder, u)))
            .filter(|(holder, _)| *holder != requester)
            .map(|(holder, units)| (holder, units, self.priority_of(holder)))
            .filter(|(_, _, priority)| *priority > request.priority)
            .collect();
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        // Evict the least urgent (highest priority number) first.
        candidates.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));

        let mut freed = 0usize;
        let mut evictions: Vec<(u64, usize)> = Vec::new();
        for (holder, units, _) in candidates {
            if freed >= needed {
                break;
            }
            let take = units.min(needed - freed);
            freed += take;
            evictions.push((holder, take));
        }
        if freed < needed {
            return Ok(Vec::new());
        }
        for (holder, units) in &evictions {
            self.strip_hold(*holder, request.resource, *units);
            self.pools
                .get_mut(&request.resource)
                .expect("known")
                .release(*units);
        }
        Ok(evictions)
    }

    fn record_hold(&mut self, entity: u64, resource: u64, units: usize) {
        self.holdings
            .entry(entity)
            .or_default()
            .entry(resource)
            .and_modify(|u| *u += units)
            .or_insert(units);
        self.wait_graph.add_holder(resource, entity);
    }

    fn strip_hold(&mut self, entity: u64, resource: u64, units: usize) {
        if let Some(res) = self.holdings.get_mut(&entity) {
            if let Some(u) = res.get_mut(&resource) {
                *u = u.saturating_sub(units);
                if *u == 0 {
                    res.remove(&resource);
                    self.wait_graph.remove_holder(resource, entity);
                }
            }
        }
    }

    /// Release all units `entity` holds of `resource`, then re-grant
    /// waiting requests. Returns the entities granted as a result.
    pub fn release(&mut self, entity: u64, resource: u64) -> Result<Vec<u64>, ResourceError> {
        let units = self.held(entity, resource);
        if units == 0 {
            return Err(ResourceError::ReleaseExceedsHold);
        }
        self.strip_hold(entity, resource, units);
        let pool = self
            .pools
            .get_mut(&resource)
            .ok_or(ResourceError::UnknownResource)?;
        pool.release(units);
        Ok(self.grant_ready(resource))
    }

    /// Re-grant queued requests for `resource`: repeatedly pick the best
    /// satisfiable waiter (priority then FIFO when the policy ranks by
    /// priority; pure FIFO otherwise). Returns the entities granted.
    pub fn grant_ready(&mut self, resource: u64) -> Vec<u64> {
        let mut granted = Vec::new();
        let Some(queue) = self.waiting.remove(&resource) else {
            return granted;
        };
        // One pass in grant order. Availability only falls during the pass, so
        // a waiter that does not fit now cannot fit later in the same call.
        // Granting in key order therefore picks the same waiters as repeatedly
        // choosing the most urgent satisfiable one.
        let mut kept = BTreeMap::new();
        for (key, w) in queue {
            if w.amount <= self.available_units(resource) && self.try_take(resource, w.amount) {
                self.record_hold(w.entity, resource, w.amount);
                self.wait_graph.remove_request(w.entity, resource);
                self.waiting_index.remove(&w.id);
                granted.push(w.entity);
            } else {
                kept.insert(key, w);
            }
        }
        if !kept.is_empty() {
            self.waiting.insert(resource, kept);
        }
        granted
    }

    /// Cancel a queued request (timeout handling): returns true if it was
    /// still waiting. Pending evictions are unaffected — preemption is
    /// immediate by design.
    pub fn cancel(&mut self, entity: u64, id: RequestId) -> bool {
        let Some(&(resource, priority)) = self.waiting_index.get(&id.0) else {
            return false;
        };
        let Some(queue) = self.waiting.get_mut(&resource) else {
            return false;
        };
        match queue.get(&(priority, id.0)) {
            Some(w) if w.entity == entity => {}
            _ => return false,
        }
        queue.remove(&(priority, id.0));
        if queue.is_empty() {
            self.waiting.remove(&resource);
        }
        self.waiting_index.remove(&id.0);
        self.wait_graph.remove_request(entity, resource);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seize_release_and_regrant() {
        let mut rc = ResourceController::new();
        rc.add_pool(0, 2).unwrap();
        assert!(matches!(
            rc.seize(10, SeizeRequest::new(0, 2)),
            Ok(SeizeOutcome::Granted(_))
        ));
        assert!(matches!(
            rc.seize(11, SeizeRequest::new(0, 1)),
            Ok(SeizeOutcome::Queued(_))
        ));
        let granted = rc.release(10, 0).unwrap();
        assert_eq!(granted, vec![11]);
        assert_eq!(rc.held(11, 0), 1);
        assert!(rc.detect_deadlock().is_none());
    }

    #[test]
    fn fifo_waiter_order_preserved() {
        let mut rc = ResourceController::new();
        rc.add_pool(0, 1).unwrap();
        rc.seize(0, SeizeRequest::new(0, 1)).unwrap();
        rc.seize(1, SeizeRequest::new(0, 1)).unwrap();
        rc.seize(2, SeizeRequest::new(0, 1)).unwrap();
        assert_eq!(rc.release(0, 0).unwrap(), vec![1]);
        assert_eq!(rc.release(1, 0).unwrap(), vec![2]);
    }

    #[test]
    fn priority_waiter_jumps_queue() {
        let mut rc = ResourceController::with_policy(PreemptPolicy::Priority);
        rc.add_pool(0, 1).unwrap();
        rc.seize(0, SeizeRequest::new(0, 1)).unwrap();
        rc.seize(1, SeizeRequest::new(0, 1)).unwrap(); // priority 0
        rc.seize(2, SeizeRequest::new(0, 1).with_priority(-5))
            .unwrap(); // urgent
        assert_eq!(rc.release(0, 0).unwrap(), vec![2]);
        assert_eq!(rc.release(2, 0).unwrap(), vec![1]);
    }

    #[test]
    fn preemption_evicts_lower_priority_holder() {
        let mut rc = ResourceController::with_policy(PreemptPolicy::PreemptivePriority);
        rc.add_pool(0, 1).unwrap();
        rc.set_priority(1, 10); // low-urgency holder
        rc.set_priority(2, -10); // high-urgency requester
        rc.seize(1, SeizeRequest::new(0, 1)).unwrap();
        let outcome = rc.seize(2, SeizeRequest::new(0, 1)).unwrap();
        match outcome {
            SeizeOutcome::Granted(evicted) => assert_eq!(evicted, vec![(1, 1)]),
            other => panic!("expected preemption, got {other:?}"),
        }
        assert_eq!(rc.held(1, 0), 0);
        assert_eq!(rc.held(2, 0), 1);
    }

    /// Regression: the preempting request's take must happen in release
    /// builds too. Before the fix, the take lived inside `debug_assert!`,
    /// so release builds left the unit free and a third requester was
    /// granted the same unit (double grant). Run with `--release` to see it.
    #[test]
    fn preemption_take_is_not_skipped_in_release() {
        let mut rc = ResourceController::with_policy(PreemptPolicy::PreemptivePriority);
        rc.add_pool(0, 1).unwrap();
        rc.set_priority(1, 10);
        rc.set_priority(2, -10);
        rc.seize(1, SeizeRequest::new(0, 1)).unwrap();
        assert!(matches!(
            rc.seize(2, SeizeRequest::new(0, 1)),
            Ok(SeizeOutcome::Granted(_))
        ));
        // The single unit is now held by entity 2, so entity 3 must queue.
        assert!(matches!(
            rc.seize(3, SeizeRequest::new(0, 1)),
            Ok(SeizeOutcome::Queued(_))
        ));
        assert_eq!(rc.held(3, 0), 0);
    }

    #[test]
    fn no_preemption_without_policy() {
        let mut rc = ResourceController::new();
        rc.add_pool(0, 1).unwrap();
        rc.set_priority(1, 10);
        rc.set_priority(2, -10);
        rc.seize(1, SeizeRequest::new(0, 1)).unwrap();
        assert!(matches!(
            rc.seize(2, SeizeRequest::new(0, 1)),
            Ok(SeizeOutcome::Queued(_))
        ));
    }

    #[test]
    fn cross_seize_deadlock_detected_and_cleared() {
        let mut rc = ResourceController::new();
        rc.add_pool(0, 1).unwrap();
        rc.add_pool(1, 1).unwrap();
        rc.seize(0, SeizeRequest::new(0, 1)).unwrap();
        rc.seize(1, SeizeRequest::new(1, 1)).unwrap();
        assert!(rc.detect_deadlock().is_none());
        rc.seize(0, SeizeRequest::new(1, 1)).unwrap();
        rc.seize(1, SeizeRequest::new(0, 1)).unwrap();
        let cycle = rc.detect_deadlock().expect("deadlock");
        assert_eq!(cycle.len(), 2);
        // Timeout path: E1 cancels its request; the cycle disappears.
        assert!(rc.cancel(1, RequestId(1)));
        assert!(rc.detect_deadlock().is_none());
    }

    #[test]
    fn cancel_unknown_request_is_noop() {
        let mut rc = ResourceController::new();
        rc.add_pool(0, 1).unwrap();
        assert!(!rc.cancel(9, RequestId(0)));
        rc.seize(0, SeizeRequest::new(0, 1)).unwrap();
        assert!(matches!(
            rc.seize(1, SeizeRequest::new(0, 1)),
            Ok(SeizeOutcome::Queued(_))
        ));
        assert!(!rc.cancel(1, RequestId(42))); // wrong id
        assert!(rc.cancel(1, RequestId(0))); // the queued one
    }

    #[test]
    fn release_errors() {
        let mut rc = ResourceController::new();
        rc.add_pool(0, 1).unwrap();
        assert_eq!(rc.release(0, 0), Err(ResourceError::ReleaseExceedsHold));
        assert_eq!(
            rc.seize(0, SeizeRequest::new(99, 1)),
            Err(ResourceError::UnknownResource)
        );
        assert_eq!(
            rc.seize(0, SeizeRequest::new(0, 0)),
            Err(ResourceError::InvalidParameter)
        );
    }
}
