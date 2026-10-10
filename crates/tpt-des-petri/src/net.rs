//! Place/transition Petri nets with marking semantics.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// A token distribution over the places of a net.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Marking(pub Vec<u32>);

impl Marking {
    /// A marking with `tokens` in every place set to 0 except `initial`.
    pub fn new(place_count: usize) -> Self {
        Self(vec![0; place_count])
    }

    /// Total tokens (useful for conservation checks).
    pub fn total(&self) -> u64 {
        self.0.iter().map(|&t| t as u64).sum()
    }

    /// Tokens in place `p`.
    pub fn at(&self, p: usize) -> u32 {
        self.0.get(p).copied().unwrap_or(0)
    }
}

impl fmt::Display for Marking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}]",
            self.0
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

/// One transition with its weighted input (pre) and output (post) arcs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transition {
    /// `(place, weight)` input arcs.
    pub pre: Vec<(usize, u32)>,
    /// `(place, weight)` output arcs.
    pub post: Vec<(usize, u32)>,
    /// Optional label.
    pub name: String,
}

/// A place/transition Petri net.
///
/// Places are numbered `0..places()`; transitions are built incrementally
/// with [`add_transition`](PetriNet::add_transition) and arc setters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PetriNet {
    place_count: usize,
    place_names: Vec<String>,
    transitions: Vec<Transition>,
}

impl PetriNet {
    /// An empty net.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a place; returns its index.
    pub fn add_place(&mut self, name: &str) -> usize {
        self.place_names.push(name.to_string());
        self.place_count += 1;
        self.place_count - 1
    }

    /// Add a transition with no arcs yet; returns its index.
    pub fn add_transition(&mut self, name: &str) -> usize {
        self.transitions.push(Transition {
            pre: Vec::new(),
            post: Vec::new(),
            name: name.to_string(),
        });
        self.transitions.len() - 1
    }

    /// Add a weighted arc `place → transition` (consumes tokens).
    pub fn add_arc_pt(&mut self, place: usize, transition: usize, weight: u32) {
        assert!(place < self.place_count, "place index out of range");
        self.transitions[transition]
            .pre
            .push((place, weight.max(1)));
    }

    /// Add a weighted arc `transition → place` (produces tokens).
    pub fn add_arc_tp(&mut self, transition: usize, place: usize, weight: u32) {
        assert!(place < self.place_count, "place index out of range");
        self.transitions[transition]
            .post
            .push((place, weight.max(1)));
    }

    /// Number of places.
    pub fn places(&self) -> usize {
        self.place_count
    }

    /// Number of transitions.
    pub fn transitions(&self) -> usize {
        self.transitions.len()
    }

    /// Transition metadata.
    pub fn transition(&self, t: usize) -> &Transition {
        &self.transitions[t]
    }

    /// True if `t` can fire at `marking` (every pre-arc has enough tokens).
    pub fn is_enabled(&self, t: usize, marking: &Marking) -> bool {
        let transition = &self.transitions[t];
        transition.pre.iter().all(|&(p, w)| marking.at(p) >= w)
    }

    /// All transitions enabled at `marking`, in index order.
    pub fn enabled(&self, marking: &Marking) -> Vec<usize> {
        (0..self.transitions.len())
            .filter(|&t| self.is_enabled(t, marking))
            .collect()
    }

    /// Fire `t` at `marking`, returning the successor marking, or `None` if
    /// `t` is not enabled.
    pub fn fire(&self, t: usize, marking: &Marking) -> Option<Marking> {
        if !self.is_enabled(t, marking) {
            return None;
        }
        let transition = &self.transitions[t];
        let mut next = marking.0.clone();
        for &(p, w) in &transition.pre {
            next[p] -= w;
        }
        for &(p, w) in &transition.post {
            next[p] += w;
        }
        Some(Marking(next))
    }

    /// The incidence matrix `C` (places × transitions): `C[p][t] =
    /// post_weight − pre_weight`. Firing transition `t` changes the marking
    /// by the column `C[·][t]`.
    pub fn incidence(&self) -> Vec<Vec<i64>> {
        let mut c = vec![vec![0i64; self.transitions.len()]; self.place_count];
        for (t, transition) in self.transitions.iter().enumerate() {
            for &(p, w) in &transition.pre {
                c[p][t] -= w as i64;
            }
            for &(p, w) in &transition.post {
                c[p][t] += w as i64;
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two-machine flow line: source → M1 buffer → M1 → M2 buffer → sink.
    fn flow_line() -> PetriNet {
        let mut net = PetriNet::new();
        let idle1 = net.add_place("idle1");
        let busy1 = net.add_place("busy1");
        let buf = net.add_place("buffer");
        let start = net.add_transition("start");
        let finish = net.add_transition("finish");
        net.add_arc_pt(idle1, start, 1);
        net.add_arc_tp(start, busy1, 1);
        net.add_arc_pt(busy1, finish, 1);
        net.add_arc_tp(finish, buf, 1);
        net.add_arc_tp(finish, idle1, 1);
        net
    }

    #[test]
    fn enabled_and_fire_semantics() {
        let net = flow_line();
        let m0 = Marking(vec![1, 0, 0]);
        assert!(net.is_enabled(0, &m0)); // start
        assert!(!net.is_enabled(1, &m0)); // finish (busy1 empty)
        let m1 = net.fire(0, &m0).unwrap();
        assert_eq!(m1, Marking(vec![0, 1, 0]));
        let m2 = net.fire(1, &m1).unwrap();
        assert_eq!(m2, Marking(vec![1, 0, 1]));
        // Firing a disabled transition fails.
        assert!(net.fire(1, &m0).is_none());
    }

    #[test]
    fn weighted_arcs() {
        let mut net = PetriNet::new();
        let p0 = net.add_place("p0");
        let p1 = net.add_place("p1");
        let batch = net.add_transition("batch");
        net.add_arc_pt(p0, batch, 2);
        net.add_arc_tp(batch, p1, 3);
        let m0 = Marking(vec![2, 0]);
        assert!(net.is_enabled(0, &m0));
        assert!(!net.is_enabled(0, &Marking(vec![1, 0])));
        assert_eq!(net.fire(0, &m0).unwrap(), Marking(vec![0, 3]));
    }

    #[test]
    fn incidence_matrix_columns() {
        let net = flow_line();
        let c = net.incidence();
        // start: idle1 −1, busy1 +1, buffer 0
        assert_eq!((c[0][0], c[1][0], c[2][0]), (-1, 1, 0));
        // finish: idle1 +1, busy1 −1, buffer +1
        assert_eq!((c[0][1], c[1][1], c[2][1]), (1, -1, 1));
    }

    #[test]
    fn enabled_lists_all_ready_transitions() {
        let net = flow_line();
        let m = Marking(vec![1, 1, 1]);
        assert_eq!(net.enabled(&m), vec![0, 1]);
    }

    #[test]
    fn marking_formatting() {
        assert_eq!(format!("{}", Marking(vec![1, 0, 3])), "[1 0 3]");
    }
}
