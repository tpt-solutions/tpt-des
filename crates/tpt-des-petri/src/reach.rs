//! Reachability graph generation by breadth-first search.
//!
//! [`reachability`] explores every marking reachable from the initial one.
//! Petri net reachability is explosive in general, so the search is bounded
//! by [`DEFAULT_MAX_STATES`] and reports [`ReachabilityError::TooManyStates`]
//! rather than exhausting memory.

use crate::net::{Marking, PetriNet};
use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

/// Default cap on the number of distinct markings explored.
pub const DEFAULT_MAX_STATES: usize = 100_000;

/// Reachability search failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReachabilityError {
    /// The net exceeded the state bound (likely unbounded or huge).
    TooManyStates,
    /// The initial marking does not match the net's place count.
    InvalidInitialMarking,
    /// Firing a transition would push a place past `u32::MAX` tokens.
    TokenOverflow,
}

impl core::fmt::Display for ReachabilityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReachabilityError::TooManyStates => {
                write!(f, "reachability exceeded the state bound")
            }
            ReachabilityError::InvalidInitialMarking => {
                write!(f, "initial marking does not match the net's place count")
            }
            ReachabilityError::TokenOverflow => {
                write!(f, "firing a transition overflows a place's token count")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ReachabilityError {}

/// The reachability graph: states, labeled edges, and the initial index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachabilityGraph {
    /// Every reachable marking, indexed.
    pub states: Vec<Marking>,
    /// Edges from each state: `(transition, successor index)`.
    pub edges: Vec<Vec<(usize, usize)>>,
    /// Index of the initial marking (always 0).
    pub initial: usize,
}

impl ReachabilityGraph {
    /// Number of reachable states.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// True if the graph has no states (invalid construction).
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// State indices with no outgoing edges (deadlocks of the net).
    pub fn deadlock_states(&self) -> Vec<usize> {
        self.edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_empty())
            .map(|(i, _)| i)
            .collect()
    }
}

/// Explore all markings reachable from `initial`, breadth-first.
///
/// Deterministic: transitions are tried in index order and states are
/// numbered in discovery order.
pub fn reachability(
    net: &PetriNet,
    initial: &Marking,
    max_states: usize,
) -> Result<ReachabilityGraph, ReachabilityError> {
    if initial.0.len() != net.places() {
        return Err(ReachabilityError::InvalidInitialMarking);
    }
    let mut states: Vec<Marking> = Vec::new();
    let mut index: BTreeMap<Marking, usize> = BTreeMap::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut edges: Vec<Vec<(usize, usize)>> = Vec::new();

    index.insert(initial.clone(), 0);
    states.push(initial.clone());
    edges.push(Vec::new());
    queue.push_back(0);

    while let Some(current) = queue.pop_front() {
        let mut out = Vec::new();
        for t in 0..net.transitions() {
            if !net.is_enabled(t, &states[current]) {
                continue;
            }
            // Enabling was just checked, so skip `fire`'s second check.
            let successor = net
                .fire_enabled(t, &states[current])
                .ok_or(ReachabilityError::TokenOverflow)?;
            let next_idx = match index.get(&successor) {
                Some(&i) => i,
                None => {
                    if states.len() >= max_states {
                        return Err(ReachabilityError::TooManyStates);
                    }
                    let i = states.len();
                    states.push(successor.clone());
                    index.insert(successor, i);
                    edges.push(Vec::new());
                    queue.push_back(i);
                    i
                }
            };
            out.push((t, next_idx));
        }
        edges[current] = out;
    }

    Ok(ReachabilityGraph {
        states,
        edges,
        initial: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Three-place cycle A →t1→ B →t2→ C →t3→ A with 2 tokens: bounded,
    /// with C(2 + 3 − 1, 2) = 6 reachable markings.
    fn three_cycle() -> PetriNet {
        let mut net = PetriNet::new();
        let a = net.add_place("A");
        let b = net.add_place("B");
        let c = net.add_place("C");
        let t1 = net.add_transition("t1");
        let t2 = net.add_transition("t2");
        let t3 = net.add_transition("t3");
        net.add_arc_pt(a, t1, 1);
        net.add_arc_tp(t1, b, 1);
        net.add_arc_pt(b, t2, 1);
        net.add_arc_tp(t2, c, 1);
        net.add_arc_pt(c, t3, 1);
        net.add_arc_tp(t3, a, 1);
        net
    }

    #[test]
    fn bounded_cycle_reaches_all_states() {
        let net = three_cycle();
        let graph = reachability(&net, &Marking(vec![2, 0, 0]), 100).unwrap();
        // Distributing 2 tokens over 3 places: C(4, 2) = 6 states.
        assert_eq!(graph.len(), 6);
        // The cycle never deadlocks.
        assert!(graph.deadlock_states().is_empty());
        // Total tokens conserved in every state.
        assert!(graph.states.iter().all(|m| m.total() == 2));
    }

    #[test]
    fn invalid_initial_marking_rejected() {
        let net = three_cycle();
        assert_eq!(
            reachability(&net, &Marking(vec![1, 0]), 10),
            Err(ReachabilityError::InvalidInitialMarking)
        );
    }

    #[test]
    fn state_bound_is_enforced() {
        // An unbounded net: one place, one transition with no pre-arc.
        let mut net = PetriNet::new();
        let p = net.add_place("p");
        let grow = net.add_transition("grow");
        net.add_arc_tp(grow, p, 1);
        assert_eq!(
            reachability(&net, &Marking(vec![0]), 10),
            Err(ReachabilityError::TooManyStates)
        );
    }

    /// Firing `t` would push `q` past `u32::MAX`. That used to panic inside
    /// the search; it is now a reported error.
    #[test]
    fn token_overflow_is_an_error_not_a_panic() {
        let mut net = PetriNet::new();
        let p = net.add_place("p");
        let q = net.add_place("q");
        let t = net.add_transition("t");
        net.add_arc_pt(p, t, 1);
        net.add_arc_tp(t, q, 1);
        assert_eq!(
            reachability(&net, &Marking(vec![1, u32::MAX]), 10),
            Err(ReachabilityError::TokenOverflow)
        );
    }
}
