//! Regression tests for the Phase 6 Petri-net fixes.

use tpt_des_core::Time;
use tpt_des_petri::invariant::place_invariants;
use tpt_des_petri::spn::{SpnError, MAX_CTMC_STATES};
use tpt_des_petri::{Marking, PetriNet, StochasticPetriNet};

/// Two parallel arcs from one place with one token: the net must not be
/// enabled. Before the fix each arc was checked against the same marking,
/// so the transition was enabled and `fire` underflowed (`1 − 1 − 1`).
#[test]
fn duplicate_pre_arcs_are_enabled_together() {
    let mut net = PetriNet::new();
    let p = net.add_place("p");
    let t = net.add_transition("t");
    net.add_arc_pt(p, t, 1);
    net.add_arc_pt(p, t, 1);
    let m = Marking(vec![1]);
    assert!(!net.is_enabled(t, &m));
    assert_eq!(net.fire(t, &m), None);

    let m2 = Marking(vec![2]);
    assert!(net.is_enabled(t, &m2));
    assert_eq!(net.fire(t, &m2), Some(Marking(vec![0])));
}

/// Output arcs that would push a place past `u32::MAX` are refused instead
/// of wrapping (or panicking in debug builds).
#[test]
fn firing_that_overflows_a_place_is_refused() {
    let mut net = PetriNet::new();
    let src = net.add_place("src");
    let full = net.add_place("full");
    let t = net.add_transition("t");
    net.add_arc_pt(src, t, 1);
    net.add_arc_tp(t, full, 1);
    let m = Marking(vec![1, u32::MAX]);
    assert_eq!(net.fire(t, &m), None);
}

/// A transition with no arcs is always enabled; firing it must not panic.
#[test]
fn zero_arc_transition_fires_unchanged() {
    let mut net = PetriNet::new();
    net.add_place("p");
    let t = net.add_transition("noop");
    assert_eq!(net.fire(t, &Marking(vec![3])), Some(Marking(vec![3])));
}

/// Infinite horizons never end on a live net: they must be rejected.
#[test]
#[should_panic(expected = "simulation time must be finite")]
fn simulate_rejects_non_finite_horizon() {
    let mut net = PetriNet::new();
    let p = net.add_place("p");
    let t = net.add_transition("t");
    net.add_arc_pt(p, t, 1);
    net.add_arc_tp(t, p, 1);
    let mut spn = StochasticPetriNet::new(net);
    spn.set_rate(0, 1.0).unwrap();
    let m = Marking(vec![1]);
    assert_eq!(
        spn.simulate(&m, Time::from_raw(-1.0), 1).err(),
        Some(SpnError::InvalidHorizon)
    );
    assert_eq!(
        spn.simulate(&m, Time::from_raw(f64::NAN), 1).err(),
        Some(SpnError::InvalidHorizon)
    );
}

/// A chain of 4101 reachable markings exceeds the dense generator bound, so
/// `to_ctmc` must refuse before allocating n² entries.
#[test]
fn to_ctmc_bounds_dense_allocation() {
    let mut net = PetriNet::new();
    let a = net.add_place("a");
    let b = net.add_place("b");
    let fwd = net.add_transition("fwd");
    let back = net.add_transition("back");
    net.add_arc_pt(a, fwd, 1);
    net.add_arc_tp(fwd, b, 1);
    net.add_arc_pt(b, back, 1);
    net.add_arc_tp(back, a, 1);
    let mut spn = StochasticPetriNet::new(net);
    spn.set_rate(fwd, 1.0).unwrap();
    spn.set_rate(back, 1.0).unwrap();
    let initial = Marking(vec![MAX_CTMC_STATES as u32, 0]);
    assert!(matches!(
        spn.to_ctmc(&initial, 100_000),
        Err(SpnError::TooManyStates)
    ));
}

#[test]
fn spn_errors_have_messages() {
    for e in [
        SpnError::InvalidRate,
        SpnError::InvalidInitialMarking,
        SpnError::TooManyStates,
        SpnError::CtmcConversion,
        SpnError::NoEnabledTransition,
        SpnError::InvalidHorizon,
    ] {
        assert!(!e.to_string().is_empty());
    }
}

/// Producing into both places from nothing conserves no non-negative
/// weighting. The old null-space basis returned a signed vector such as
/// `[−1, 1]` here; Farkas correctly reports no non-negative P-invariant.
#[test]
fn place_invariants_are_non_negative() {
    let mut net = PetriNet::new();
    let p0 = net.add_place("p0");
    let p1 = net.add_place("p1");
    let t = net.add_transition("source");
    net.add_arc_tp(t, p0, 1);
    net.add_arc_tp(t, p1, 1);
    assert!(place_invariants(&net).is_empty());
}

#[test]
fn place_invariant_of_a_move_is_the_token_total() {
    let mut net = PetriNet::new();
    let p0 = net.add_place("p0");
    let p1 = net.add_place("p1");
    let t = net.add_transition("move");
    net.add_arc_pt(p0, t, 1);
    net.add_arc_tp(t, p1, 1);
    assert_eq!(place_invariants(&net), vec![vec![1.0, 1.0]]);
}

/// `p0 + p1 → 2·p2`: firing removes one token from each input and adds two
/// to `p2`, so `2·p0 + p2` and `2·p1 + p2` are conserved (minimal support,
/// non-negative).
#[test]
fn place_invariants_of_a_merge_are_the_minimal_non_negative_basis() {
    let mut net = PetriNet::new();
    let p0 = net.add_place("p0");
    let p1 = net.add_place("p1");
    let p2 = net.add_place("p2");
    let t = net.add_transition("merge");
    net.add_arc_pt(p0, t, 1);
    net.add_arc_pt(p1, t, 1);
    net.add_arc_tp(t, p2, 2);
    assert_eq!(
        place_invariants(&net),
        vec![vec![0.0, 2.0, 1.0], vec![2.0, 0.0, 1.0]]
    );
}
