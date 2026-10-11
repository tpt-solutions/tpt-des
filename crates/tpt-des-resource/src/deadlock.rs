//! Deadlock detection on the resource allocation graph.
//!
//! The checkpoint in `todo.md` asked whether `tpt-math-graph` already
//! exposes cycle detection: it does not (only `toposort`, shortest paths,
//! and max-flow), so this module implements the small local detector.
//!
//! The model: entities *hold* units of resources and *wait for* resources.
//! A wait-for edge `e₁ → e₂` exists when `e₁` waits for a resource that `e₂`
//! holds. A deadlock is exactly a cycle in the wait-for graph, and
//! [`WaitForGraph::find_cycle`] returns one such cycle (the involved entities) if one
//! exists.
//!
//! # Examples
//!
//! The classic two-entity cross-seize deadlock:
//!
//! ```
//! use tpt_des_resource::deadlock::WaitForGraph;
//!
//! let mut g = WaitForGraph::default();
//! // E0 holds R0 and wants R1; E1 holds R1 and wants R0.
//! g.add_holder(0, 0);        // resource 0 held by entity 0
//! g.add_holder(1, 1);        // resource 1 held by entity 1
//! g.set_request(0, Some(1)); // entity 0 waits for resource 1
//! g.set_request(1, Some(0)); // entity 1 waits for resource 0
//!
//! let cycle = g.find_cycle().unwrap();
//! assert!(cycle.contains(&0) && cycle.contains(&1));
//! ```

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

/// An entity identifier in the resource contention model.
pub type EntityId = u64;
/// A resource identifier in the resource contention model.
pub type ResourceId = u64;

/// Holds-and-waits bookkeeping: which entities hold which resources, and
/// which resource each waiting entity is blocked on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaitForGraph {
    /// resource → set of entities currently holding ≥ 1 unit of it.
    holders: BTreeMap<ResourceId, BTreeSet<EntityId>>,
    /// entity → resource → number of outstanding queued requests for it.
    /// An entity may have several queued requests at once (on different
    /// resources, or the same one), so each edge is counted.
    waiting_for: BTreeMap<EntityId, BTreeMap<ResourceId, usize>>,
}

impl WaitForGraph {
    /// Record that `entity` now holds a unit of `resource`.
    pub fn add_holder(&mut self, resource: ResourceId, entity: EntityId) {
        self.holders.entry(resource).or_default().insert(entity);
    }

    /// Record that `entity` no longer holds any unit of `resource`.
    /// Returns true if it was a holder.
    pub fn remove_holder(&mut self, resource: ResourceId, entity: EntityId) -> bool {
        self.holders
            .get_mut(&resource)
            .is_some_and(|set| set.remove(&entity))
    }

    /// Replace all of `entity`'s waits with a single wait on `resource`
    /// (`None` = running, waiting on nothing).
    pub fn set_request(&mut self, entity: EntityId, resource: Option<ResourceId>) {
        self.waiting_for.remove(&entity);
        if let Some(resource) = resource {
            self.add_request(entity, resource);
        }
    }

    /// Record one more queued request by `entity` for `resource`.
    pub fn add_request(&mut self, entity: EntityId, resource: ResourceId) {
        *self
            .waiting_for
            .entry(entity)
            .or_default()
            .entry(resource)
            .or_insert(0) += 1;
    }

    /// Withdraw one queued request by `entity` for `resource`. Returns true
    /// if one was outstanding.
    pub fn remove_request(&mut self, entity: EntityId, resource: ResourceId) -> bool {
        let Some(resources) = self.waiting_for.get_mut(&entity) else {
            return false;
        };
        let Some(count) = resources.get_mut(&resource) else {
            return false;
        };
        *count -= 1;
        if *count == 0 {
            resources.remove(&resource);
        }
        if resources.is_empty() {
            self.waiting_for.remove(&entity);
        }
        true
    }

    /// The lowest-numbered resource `entity` is waiting for, if any.
    pub fn waiting_for(&self, entity: EntityId) -> Option<ResourceId> {
        self.waiting_for
            .get(&entity)
            .and_then(|resources| resources.keys().next().copied())
    }

    /// The wait-for adjacency: entity → entities it is blocked on.
    pub fn adjacency(&self) -> BTreeMap<EntityId, BTreeSet<EntityId>> {
        let mut adj: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
        for (&entity, resources) in &self.waiting_for {
            for &resource in resources.keys() {
                let blocked_on = self.holders.get(&resource).cloned().unwrap_or_default();
                // A holder is never blocked on itself.
                blocked_on
                    .into_iter()
                    .filter(|&holder| holder != entity)
                    .for_each(|holder| {
                        adj.entry(entity).or_default().insert(holder);
                    });
            }
        }
        adj
    }

    /// Find a cycle in the wait-for graph, returning its entities in cyclic
    /// order, or `None` if the graph is acyclic (no deadlock).
    ///
    /// Iterative DFS with white/gray/black coloring; deterministic (BTreeMap
    /// iteration order).
    pub fn find_cycle(&self) -> Option<Vec<EntityId>> {
        find_cycle_in_digraph(&self.adjacency())
    }
}

/// Generic deterministic cycle detection over a directed graph given as an
/// adjacency map. Returns one cycle as a node path (first node repeated at
/// the end is *not* included), or `None`.
pub fn find_cycle_in_digraph(adjacency: &BTreeMap<u64, BTreeSet<u64>>) -> Option<Vec<u64>> {
    // 0 = white (unvisited), 1 = gray (on stack), 2 = black (done).
    let mut color: BTreeMap<u64, u8> = BTreeMap::new();
    for &node in adjacency.keys() {
        color.entry(node).or_insert(0);
    }

    for &root in adjacency.keys() {
        if color.get(&root).copied().unwrap_or(2) != 0 {
            continue;
        }
        // Explicit stack of (node, iterator over successors).
        let mut stack: Vec<(u64, alloc::vec::IntoIter<u64>)> = Vec::new();
        color.insert(root, 1);
        let successors: alloc::vec::Vec<u64> = adjacency
            .get(&root)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default();
        stack.push((root, successors.into_iter()));

        while let Some(&node) = stack.last().map(|(n, _)| n) {
            let Some((_node, iter)) = stack.last_mut() else {
                break;
            };
            match iter.next() {
                Some(next) => {
                    match color.get(&next).copied().unwrap_or(0) {
                        0 => {
                            color.insert(next, 1);
                            let succ: alloc::vec::Vec<u64> = adjacency
                                .get(&next)
                                .map(|s| s.iter().copied().collect())
                                .unwrap_or_default();
                            stack.push((next, succ.into_iter()));
                        }
                        1 => {
                            // Back edge to a gray node: reconstruct the cycle.
                            let pos = stack.iter().position(|(n, _)| *n == next)?;
                            let cycle: Vec<u64> = stack[pos..].iter().map(|(n, _)| *n).collect();
                            return Some(cycle);
                        }
                        _ => {} // black: already fully explored
                    }
                }
                None => {
                    color.insert(node, 2);
                    stack.pop();
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_entity_cross_seize_is_a_cycle() {
        let mut g = WaitForGraph::default();
        // E0 holds R0 and wants R1; E1 holds R1 and wants R0.
        g.add_holder(0, 0);
        g.add_holder(1, 1);
        g.set_request(0, Some(1));
        g.set_request(1, Some(0));
        let cycle = g.find_cycle().expect("deadlock expected");
        assert!(cycle.contains(&0) && cycle.contains(&1));
    }

    #[test]
    fn acyclic_waits_are_not_deadlocked() {
        let mut g = WaitForGraph::default();
        // Chain: E0 waits for R1 (held by E1); E1 waits for R2 (held by E2,
        // which waits for nothing).
        g.add_holder(1, 1);
        g.add_holder(2, 2);
        g.set_request(0, Some(1));
        g.set_request(1, Some(2));
        g.set_request(2, None);
        assert!(g.find_cycle().is_none());
    }

    #[test]
    fn releasing_a_resource_breaks_the_cycle() {
        let mut g = WaitForGraph::default();
        g.add_holder(0, 0);
        g.add_holder(1, 1);
        g.set_request(0, Some(1));
        g.set_request(1, Some(0));
        assert!(g.find_cycle().is_some());
        g.remove_holder(1, 1);
        assert!(g.find_cycle().is_none());
    }

    #[test]
    fn self_wait_is_ignored() {
        let mut g = WaitForGraph::default();
        g.add_holder(0, 0); // E0 holds the resource it waits for
        g.set_request(0, Some(0));
        assert!(g.find_cycle().is_none());
    }

    #[test]
    fn three_way_cycle() {
        let mut g = WaitForGraph::default();
        g.add_holder(0, 1);
        g.add_holder(1, 2);
        g.add_holder(2, 0);
        g.set_request(0, Some(1));
        g.set_request(1, Some(2));
        g.set_request(2, Some(0));
        let cycle = g.find_cycle().expect("3-cycle expected");
        assert_eq!(cycle.len(), 3);
    }
}
