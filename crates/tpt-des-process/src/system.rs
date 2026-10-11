//! The process executor: async coroutines scheduled by the simulation
//! clock.
//!
//! Architecture: all state lives behind one `Rc<RefCell<Shared>>`. The
//! executor never holds the lock while polling a future — it takes the
//! pinned future out, polls it, and puts it back — so process bodies (and
//! the operation futures in [`ops`](crate::ops)) can freely lock the state during
//! their own polls without re-entrancy.

use crate::ops::{Acquire, Delay, WaitUntil};
use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::rc::Rc;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Waker};
use std::sync::Arc;
use tpt_math_prob_core::Rng as _;

pub use crate::ops::Interrupt;

/// Identifies a spawned process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u64);

/// What a process body returns.
pub type ProcessOutput = Result<(), Interrupt>;

/// The async context handed to process bodies.
///
/// Cloned freely; every method takes the internal lock only for the
/// duration of the call, so never hold a borrow across an `await`.
#[derive(Clone)]
pub struct Sim {
    pub(crate) shared: Rc<RefCell<Shared>>,
}

impl core::fmt::Debug for Sim {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let shared = self.shared.borrow();
        f.debug_struct("Sim")
            .field("now", &shared.now)
            .field("processes", &shared.procs.len())
            .finish()
    }
}

impl Sim {
    /// Current simulation time.
    pub fn now(&self) -> f64 {
        self.shared.borrow().now
    }

    /// Move the simulation clock forward to `t` (never backwards).
    ///
    /// Hybrid bridges call this to keep the discrete clock synchronized
    /// with continuous integration time.
    pub fn set_time(&self, t: f64) {
        let mut shared = self.shared.borrow_mut();
        shared.now = shared.now.max(t);
    }

    /// Next uniform `f64` in `[0, 1)` from the seeded simulation RNG.
    pub fn uniform(&self) -> f64 {
        self.shared.borrow_mut().rng.next_f64()
    }

    /// Spawn a process from an async closure receiving this handle.
    pub fn spawn<F, Fut>(&self, body: F) -> ProcessId
    where
        F: FnOnce(Sim) -> Fut,
        Fut: Future<Output = ProcessOutput> + 'static,
    {
        self.spawn_future(body(self.clone()))
    }

    /// Spawn an already-built future as a process.
    pub fn spawn_future<Fut>(&self, body: Fut) -> ProcessId
    where
        Fut: Future<Output = ProcessOutput> + 'static,
    {
        let mut shared = self.shared.borrow_mut();
        let pid = ProcessId(shared.next_pid);
        shared.next_pid += 1;
        shared.procs.insert(pid, ProcEntry::new(Box::pin(body)));
        shared.ready.push_back(pid);
        pid
    }

    /// Suspend the calling process for `duration` seconds.
    ///
    /// Resolves `Err(Interrupt)` if the process is interrupted first.
    pub fn delay(&self, duration: f64) -> Delay {
        Delay::new(self.clone(), self.current_pid(), duration)
    }

    /// Acquire `amount` units of `resource` (all-or-nothing, FIFO waiters).
    ///
    /// # Panics
    ///
    /// If `amount` exceeds the resource's capacity: such a request could
    /// never be granted and would block forever.
    pub fn acquire(&self, resource: u64, amount: usize) -> Acquire {
        if let Some(&capacity) = self.shared.borrow().capacities.get(&resource) {
            assert!(
                amount <= capacity,
                "acquire of {amount} units exceeds capacity {capacity} of resource {resource}"
            );
        }
        Acquire::new(self.clone(), self.current_pid(), resource, amount)
    }

    /// Return `amount` units to `resource` and wake the FIFO head if its
    /// request now fits.
    ///
    /// Free units are clamped at the resource's capacity, so a release can
    /// never create units. Releasing an unregistered resource is a no-op.
    pub fn release(&self, resource: u64, amount: usize) {
        let mut shared = self.shared.borrow_mut();
        let Some(&capacity) = shared.capacities.get(&resource) else {
            return;
        };
        let free = shared.resources.entry(resource).or_insert(0);
        *free = free.saturating_add(amount).min(capacity);
        wake_front(&mut shared, resource);
    }

    /// Units currently free of `resource`.
    pub fn available(&self, resource: u64) -> usize {
        self.shared
            .borrow()
            .resources
            .get(&resource)
            .copied()
            .unwrap_or(0)
    }

    /// Suspend until `predicate` returns true; re-checked on every
    /// [`notify_all`](Sim::notify_all).
    pub fn wait_until<F>(&self, predicate: F) -> WaitUntil
    where
        F: Fn(&Sim) -> bool + 'static,
    {
        WaitUntil::new(self.clone(), self.current_pid(), predicate)
    }

    /// Re-check every registered `wait_until` predicate and wake the
    /// processes whose predicates now hold. Call after any state change a
    /// condition might depend on.
    ///
    /// Predicates run with no internal borrow held, so they may read the
    /// simulation state through `&Sim` (e.g. [`Sim::available`]). They must
    /// not register new waits or call `notify_all` themselves.
    pub fn notify_all(&self) {
        let conditions = core::mem::take(&mut self.shared.borrow_mut().conditions);
        let mut kept = Vec::with_capacity(conditions.len());
        let mut woken = Vec::new();
        for condition in conditions {
            if (condition.predicate)(self) {
                woken.extend(condition.procs);
            } else {
                kept.push(condition);
            }
        }
        let mut shared = self.shared.borrow_mut();
        // Conditions registered while predicates ran were appended to the
        // (empty) registry; keep them after the survivors.
        kept.append(&mut shared.conditions);
        shared.conditions = kept;
        shared.ready.extend(woken);
    }

    /// Interrupt `target`: its pending operation resolves with
    /// [`Interrupt`]. The coroutine body decides to recover (continue
    /// awaiting) or abort (return the error). No-op if already finished.
    ///
    /// A process may interrupt itself while running.
    pub fn interrupt(&self, target: ProcessId) {
        let mut shared = self.shared.borrow_mut();
        if !is_alive(&shared, target) {
            return;
        }
        shared.interrupted.insert(target);
        cancel_pending(&mut shared, target);
        shared.ready.push_back(target);
    }

    /// True if the process finished (normally or by returning an error).
    pub fn is_finished(&self, pid: ProcessId) -> bool {
        !is_alive(&self.shared.borrow(), pid)
    }

    /// How many processes are still alive (finished ones are dropped).
    pub fn process_count(&self) -> usize {
        self.shared.borrow().procs.len()
    }

    fn current_pid(&self) -> ProcessId {
        let shared = self.shared.borrow();
        match shared.polling {
            Some(pid) => pid,
            None => panic!(
                "blocking operation (delay/acquire/wait_until) used outside a spawned process;                  create it inside a body passed to `spawn`"
            ),
        }
    }
}

pub(crate) struct ProcEntry {
    pub(crate) future: Pin<Box<dyn Future<Output = ProcessOutput>>>,
    pub(crate) finished: bool,
}

impl ProcEntry {
    fn new(future: Pin<Box<dyn Future<Output = ProcessOutput>>>) -> Self {
        Self {
            future,
            finished: false,
        }
    }
}

pub(crate) struct Condition {
    pub(crate) id: u64,
    pub(crate) predicate: Box<dyn Fn(&Sim) -> bool>,
    pub(crate) procs: Vec<ProcessId>,
}

/// Mutable simulation state, behind one mutex.
pub(crate) struct Shared {
    pub(crate) now: f64,
    pub(crate) rng: tpt_des_core::SimRng,
    pub(crate) procs: BTreeMap<ProcessId, ProcEntry>,
    pub(crate) ready: VecDeque<ProcessId>,
    pub(crate) resources: BTreeMap<u64, usize>,
    /// Total units per resource; `release` never lifts `resources` above it.
    pub(crate) capacities: BTreeMap<u64, usize>,
    pub(crate) resource_waiters: BTreeMap<u64, VecDeque<(ProcessId, usize)>>,
    pub(crate) conditions: Vec<Condition>,
    pub(crate) timers: tpt_des_core::EventCalendar<tpt_des_core::Seconds, ProcessId>,
    pub(crate) next_pid: u64,
    pub(crate) next_condition: u64,
    /// Processes with a pending interrupt (SimPy-style flag: delivered at
    /// the next poll of a pending operation). Kept outside `ProcEntry`
    /// because the executor removes entries from `procs` while polling.
    pub(crate) interrupted: BTreeSet<ProcessId>,
    /// Live delay timers per process, for cancellation.
    pub(crate) pending_timers: BTreeMap<ProcessId, tpt_des_core::EventId>,
    /// The process currently being polled (context for `current_pid`).
    pub(crate) polling: Option<ProcessId>,
}

/// A process is alive while it is registered and unfinished. The process
/// being polled is removed from `procs` for the duration of its poll, so it
/// is checked via `polling` — this keeps self-interruption working.
pub(crate) fn is_alive(shared: &Shared, pid: ProcessId) -> bool {
    shared.polling == Some(pid) || shared.procs.get(&pid).is_some_and(|e| !e.finished)
}

/// Cancel every pending registration of `pid` (timers, resource queues,
/// condition lists).
pub(crate) fn cancel_pending(shared: &mut Shared, pid: ProcessId) {
    if let Some(event) = shared.pending_timers.remove(&pid) {
        shared.timers.cancel(event);
    }
    let resources: Vec<u64> = shared.resource_waiters.keys().copied().collect();
    for resource in resources {
        if let Some(queue) = shared.resource_waiters.get_mut(&resource) {
            queue.retain(|&(w, _)| w != pid);
        }
        // Removing a queued head may let the next waiter through.
        wake_front(shared, resource);
    }
    for condition in &mut shared.conditions {
        condition.procs.retain(|&p| p != pid);
    }
    shared.conditions.retain(|c| !c.procs.is_empty());
}

/// Make the FIFO head of `resource`'s queue runnable if its request fits
/// the currently free units. The head itself takes the units when it is
/// polled (see [`Acquire`]), so waking never reserves capacity on anyone's
/// behalf.
pub(crate) fn wake_front(shared: &mut Shared, resource: u64) {
    let available = shared.resources.get(&resource).copied().unwrap_or(0);
    let head = shared
        .resource_waiters
        .get(&resource)
        .and_then(|q| q.front().copied());
    if let Some((pid, units)) = head {
        if units <= available && !shared.ready.contains(&pid) {
            shared.ready.push_back(pid);
        }
    }
}

/// The simulation engine: spawn async processes and run the event loop.
#[derive(Clone, Debug)]
pub struct ProcessSystem {
    sim: Sim,
}

impl Default for ProcessSystem {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ProcessSystem {
    /// A system with the given RNG seed.
    pub fn new(seed: u64) -> Self {
        let shared = Shared {
            now: 0.0,
            rng: tpt_des_core::SimRng::new(seed),
            procs: BTreeMap::new(),
            ready: VecDeque::new(),
            resources: BTreeMap::new(),
            capacities: BTreeMap::new(),
            resource_waiters: BTreeMap::new(),
            conditions: Vec::new(),
            timers: tpt_des_core::EventCalendar::new(),
            next_pid: 0,
            next_condition: 0,
            interrupted: BTreeSet::new(),
            pending_timers: BTreeMap::new(),
            polling: None,
        };
        Self {
            sim: Sim {
                shared: Rc::new(RefCell::new(shared)),
            },
        }
    }

    /// The per-process handle (captured by spawned bodies).
    pub fn handle(&self) -> Sim {
        self.sim.clone()
    }

    /// Current simulation time.
    pub fn now(&self) -> f64 {
        self.sim.now()
    }

    /// Move the simulation clock forward (hybrid bridges).
    pub fn set_time(&self, t: f64) {
        self.sim.set_time(t)
    }

    /// Spawn a process from an async closure receiving the handle.
    pub fn spawn<F, Fut>(&self, body: F) -> ProcessId
    where
        F: FnOnce(Sim) -> Fut,
        Fut: Future<Output = ProcessOutput> + 'static,
    {
        self.sim.spawn(body)
    }

    /// Interrupt a process.
    pub fn interrupt(&self, target: ProcessId) {
        self.sim.interrupt(target)
    }

    /// True if the process finished.
    pub fn is_finished(&self, pid: ProcessId) -> bool {
        self.sim.is_finished(pid)
    }

    /// Register a resource with initial free units.
    pub fn add_resource(&self, resource: u64, units: usize) {
        let mut shared = self.sim.shared.borrow_mut();
        shared.resources.insert(resource, units);
        shared.capacities.insert(resource, units);
    }

    /// Run until every process finished, or events are exhausted (which
    /// includes deadlocked condition waits).
    pub fn run(&mut self) {
        self.run_until(f64::INFINITY);
    }

    /// Run until `horizon`, or until events are exhausted.
    pub fn run_until(&mut self, horizon: f64) {
        enum Action {
            Poll(ProcessId),
            Pump,
            Stop,
        }
        loop {
            // Runnable processes run at the current instant before any timer
            // advances the clock (see `pump_one`).
            let action = {
                let mut shared = self.sim.shared.borrow_mut();
                if let Some(&pid) = shared.ready.front() {
                    Action::Poll(pid)
                } else {
                    match shared.timers.next_time() {
                        Some(t) if t <= horizon => Action::Pump,
                        _ => Action::Stop,
                    }
                }
            };
            match action {
                Action::Poll(pid) => {
                    let popped = {
                        let mut shared = self.sim.shared.borrow_mut();
                        shared.ready.pop_front()
                    };
                    if popped == Some(pid) {
                        self.poll_one(pid);
                    }
                }
                Action::Pump => self.pump_one(horizon),
                Action::Stop => break,
            }
        }
    }

    /// Fire the earliest pending timer: advance the clock to it and make its
    /// process runnable. Firing one timer at a time keeps the clock from
    /// running ahead of processes that are already runnable at an earlier
    /// instant.
    fn pump_one(&self, horizon: f64) {
        let mut shared = self.sim.shared.borrow_mut();
        if let Some(popped) = shared.timers.pop_until(horizon) {
            shared.now = shared.now.max(popped.time);
            let pid = popped.payload;
            shared.pending_timers.remove(&pid);
            shared.ready.push_back(pid);
        }
    }

    fn poll_one(&self, pid: ProcessId) {
        // Take the future out so bodies can re-enter the lock freely.
        let mut entry = {
            let mut shared = self.sim.shared.borrow_mut();
            shared.polling = Some(pid);
            shared.procs.remove(&pid)
        };
        let Some(mut entry) = entry.take() else {
            let mut shared = self.sim.shared.borrow_mut();
            shared.polling = None;
            return;
        };
        if !entry.finished {
            let waker = noop_waker();
            let mut cx = Context::from_waker(&waker);
            if entry.future.as_mut().poll(&mut cx).is_ready() {
                entry.finished = true;
            }
        }

        let mut shared = self.sim.shared.borrow_mut();
        shared.polling = None;
        // Finished futures are dropped; `is_finished` treats a missing entry
        // as finished, so no state is lost.
        if !entry.finished {
            shared.procs.insert(pid, entry);
        }
    }
}

struct NoopWake;

impl std::task::Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn noop_waker() -> Waker {
    Waker::from(Arc::new(NoopWake))
}
