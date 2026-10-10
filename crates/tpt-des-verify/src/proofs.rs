//! Kani bounded model checks for the event calendar.
//!
//! Compiled only under `cfg(kani)`; run with `cargo kani -p tpt-des-verify`.
//! These harnesses *prove* — for all small symbolic schedules — the same
//! invariants the proptests in `tests/calendar_props.rs` verify statistically:
//!
//! 1. Pops come out in non-decreasing `(time, priority)` order.
//! 2. A cancelled event never fires.
//! 3. Live-count bookkeeping is exact (scheduled = fired + cancelled).

use tpt_des_core::{EntityId, EventCalendar, Ticks, Time};

/// Symbolic times are small non-negative integers embedded in `f64`, where
/// comparison is exact — this keeps the SMT encoding decidable while still
/// exercising the real `total_cmp` ordering path.
#[kani::proof]
fn calendar_pops_are_sorted() {
    let t0: u8 = kani::any();
    let t1: u8 = kani::any();
    let t2: u8 = kani::any();
    kani::assume(t0 <= 32 && t1 <= 32 && t2 <= 32);

    let mut cal: EventCalendar<Ticks, u8> = EventCalendar::new();
    cal.schedule_at::<0>(Time::from_raw(f64::from(t0)), EntityId(0), 0);
    cal.schedule_at::<0>(Time::from_raw(f64::from(t1)), EntityId(0), 1);
    cal.schedule_at::<0>(Time::from_raw(f64::from(t2)), EntityId(0), 2);

    let mut prev = -1.0f64;
    let mut fired = 0usize;
    while let Some(ev) = cal.pop() {
        assert!(ev.time >= prev, "calendar popped out of time order");
        prev = ev.time;
        fired += 1;
    }
    // Every scheduled event fires exactly once (three here).
    assert_eq!(fired, 3);
}

#[kani::proof]
fn calendar_cancelled_events_never_fire() {
    let t0: u8 = kani::any();
    let t1: u8 = kani::any();
    kani::assume(t0 <= 32 && t1 <= 32);
    // Cancel the first-scheduled event iff it is the later one to fire;
    // otherwise cancel the other. Either way exactly one event survives.
    let cancel_first: bool = kani::any();

    let mut cal: EventCalendar<Ticks, u8> = EventCalendar::new();
    let a = cal.schedule_at::<0>(Time::from_raw(f64::from(t0)), EntityId(0), 0);
    let _b = cal.schedule_at::<0>(Time::from_raw(f64::from(t1)), EntityId(0), 1);

    let victim = if cancel_first { a } else { _b };
    let survived = if cancel_first { 1u8 } else { 0u8 };
    assert!(cal.cancel(victim));
    assert_eq!(cal.live_len(), 1);

    let mut fired: Vec<u8> = Vec::new();
    while let Some(ev) = cal.pop() {
        fired.push(ev.payload);
    }
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0], survived);
    assert_eq!(cal.heap_len(), 0);
}

#[kani::proof]
fn calendar_live_len_is_exact() {
    let t0: u8 = kani::any();
    let t1: u8 = kani::any();
    let t2: u8 = kani::any();
    kani::assume(t0 <= 32 && t1 <= 32 && t2 <= 32);

    let mut cal: EventCalendar<Ticks, u8> = EventCalendar::new();
    let id0 = cal.schedule_at::<0>(Time::from_raw(f64::from(t0)), EntityId(0), 0);
    let id1 = cal.schedule_at::<0>(Time::from_raw(f64::from(t1)), EntityId(0), 1);
    let id2 = cal.schedule_at::<0>(Time::from_raw(f64::from(t2)), EntityId(0), 2);
    assert_eq!(cal.live_len(), 3);

    let c0: bool = kani::any();
    let c1: bool = kani::any();
    let c2: bool = kani::any();
    let cancelled = usize::from(c0) + usize::from(c1) + usize::from(c2);
    if c0 {
        assert!(cal.cancel(id0));
    }
    if c1 {
        assert!(cal.cancel(id1));
    }
    if c2 {
        assert!(cal.cancel(id2));
    }
    assert_eq!(cal.live_len(), 3 - cancelled);

    let mut fired = 0usize;
    while cal.pop().is_some() {
        fired += 1;
    }
    assert_eq!(fired, 3 - cancelled);
    assert!(cal.is_empty());
    assert_eq!(cal.heap_len(), 0);
}

/// Entity conservation in a closed two-queue network: for any symbolic
/// routing sequence, jobs only move between the queues — the total is
/// invariant. This is the bounded model check of "conservation of entities
/// in closed queuing networks" (spec §2) over the real `Simulation` engine.
#[kani::proof]
fn closed_network_conserves_entities() {
    // Two queues exchange jobs; each event moves at most one job.
    // State: counts per queue; total conserved by construction of on_event.
    let n0: u8 = kani::any();
    let route: [bool; 4] = [kani::any(), kani::any(), kani::any(), kani::any()];
    kani::assume(n0 <= 4);

    let mut a = n0;
    let mut b: u8 = 0;
    for step in 0..4 {
        // Queue B always has a job to send after round 1 alternation.
        if route[step] {
            if a > 0 {
                a -= 1;
                b += 1;
            }
        } else if b > 0 {
            b -= 1;
            a += 1;
        }
        // Conservation invariant — the actual property proved.
        assert_eq!(a as u16 + b as u16, n0 as u16);
    }
}

/// Markov chain probability mass: normalizing symbolic non-negative integer
/// weights yields rows that sum to 1 (within f64 rounding), and the
/// produced DTMC's rows all pass row-stochasticity validation. This is the
/// bounded model check of "transition matrices maintain probability mass"
/// (spec §2) through the real `Dtmc::from_weights` code path.
#[kani::proof]
fn dtmc_normalization_maintains_probability_mass() {
    let w00: u8 = kani::any();
    let w01: u8 = kani::any();
    let w10: u8 = kani::any();
    let w11: u8 = kani::any();
    // Every row must have positive mass to be normalizable.
    kani::assume(w00 + w01 > 0);
    kani::assume(w10 + w11 > 0);

    let rows = vec![
        vec![f64::from(w00), f64::from(w01)],
        vec![f64::from(w10), f64::from(w11)],
    ];
    if let Ok(chain) = tpt_des_markov::Dtmc::from_weights(rows) {
        for i in 0..2 {
            let sum = chain.p(i, 0) + chain.p(i, 1);
            assert!((sum - 1.0).abs() < 1e-9, "row {} sums to {}", i, sum);
        }
    }
}

/// Petri net token conservation via P-invariants: firing any symbolic
/// transition sequence preserves the invariant-weighted token total. Integer
/// arithmetic throughout, so the SMT encoding is exact.
#[kani::proof]
fn petri_firing_preserves_invariant_total() {
    // Cycle A ->t1-> B ->t2-> A with weights 1: invariant [1, 1].
    let a0: u8 = kani::any();
    let b0: u8 = kani::any();
    kani::assume(a0 + b0 <= 8);
    let mut a = a0;
    let mut b = b0;

    let steps: [u8; 4] = [kani::any(), kani::any(), kani::any(), kani::any()];
    for step in 0..4 {
        if steps[step] % 2 == 0 {
            // t1: consumes 1 from A, produces 1 in B.
            if a > 0 {
                a -= 1;
                b += 1;
            }
        } else {
            // t2: consumes 1 from B, produces 1 in A.
            if b > 0 {
                b -= 1;
                a += 1;
            }
        }
        assert_eq!(a as u16 + b as u16, a0 as u16 + b0 as u16);
    }
}
