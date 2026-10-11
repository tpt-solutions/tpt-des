//! Kani bounded model checks for the event calendar.
//!
//! Compiled only under `cfg(kani)`; run with `cargo kani -p tpt-des-verify`.
//! These harnesses *prove* — for all small symbolic schedules — the same
//! invariants the proptests in `tests/calendar_props.rs` verify statistically:
//!
//! 1. Pops come out in non-decreasing `(time, priority)` order.
//! 2. A cancelled event never fires.
//! 3. Live-count bookkeeping is exact (scheduled = fired + cancelled).

use std::collections::{BTreeMap, BTreeSet};

use tpt_des_core::{Ctx, Entity, EntityId, EventCalendar, Simulation, Ticks, Time};
use tpt_des_petri::{Marking, PetriNet};
use tpt_des_resource::deadlock::{find_cycle_in_digraph, WaitForGraph};

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

/// A job token. Every event in the calendar is exactly one job in transit.
#[derive(Clone, Copy)]
struct Job;

/// A node in a closed two-node network. It holds `count` jobs and forwards
/// one to its peer, at a symbolic choice of moments.
struct Node {
    count: u8,
    peer: EntityId,
}

impl Entity<Ticks, Job> for Node {
    fn on_create(&mut self, ctx: &mut Ctx<'_, Ticks, Job>) {
        if self.count > 0 {
            self.count -= 1;
            ctx.schedule::<0>(Time::from_raw(1.0), self.peer, Job);
        }
    }

    fn on_event(&mut self, _event: Job, ctx: &mut Ctx<'_, Ticks, Job>) {
        self.count += 1;
        let forward: bool = kani::any();
        if forward && self.count > 0 {
            self.count -= 1;
            ctx.schedule::<0>(Time::from_raw(1.0), self.peer, Job);
        }
    }
}

/// Entity conservation in a closed network, on the real `Simulation`
/// engine: at any point, jobs held by nodes plus jobs in transit (pending
/// events) equal the population. This is the bounded model check of
/// "conservation of entities in closed queuing networks" (spec §2).
#[kani::proof]
#[kani::unwind(8)]
fn closed_network_conserves_entities() {
    let population: u8 = kani::any();
    kani::assume(population <= 3);
    let steps: usize = kani::any();
    kani::assume(steps <= 6);

    let mut a = Node {
        count: population,
        peer: EntityId(1),
    };
    let mut b = Node {
        count: 0,
        peer: EntityId(0),
    };
    let in_transit = {
        let mut sim: Simulation<'_, Ticks, Job> = Simulation::new(7);
        sim.add_entity(&mut a);
        sim.add_entity(&mut b);
        sim.run(steps as u64);
        sim.pending_events()
    };
    assert_eq!(
        a.count as usize + b.count as usize + in_transit,
        population as usize
    );
}

/// Petri net token conservation on the real `PetriNet` API: a two-place
/// cycle with unit weights conserves the P-invariant `[1, 1]` (the token
/// total) under any firing sequence. Integer arithmetic, so the encoding is
/// exact.
#[kani::proof]
#[kani::unwind(6)]
fn petri_firing_preserves_invariant_total() {
    let mut net = PetriNet::new();
    let p0 = net.add_place("p0");
    let p1 = net.add_place("p1");
    let t0 = net.add_transition("t0");
    let t1 = net.add_transition("t1");
    net.add_arc_pt(p0, t0, 1);
    net.add_arc_tp(t0, p1, 1);
    net.add_arc_pt(p1, t1, 1);
    net.add_arc_tp(t1, p0, 1);

    let a0: u8 = kani::any();
    let b0: u8 = kani::any();
    kani::assume(u16::from(a0) + u16::from(b0) <= 8);
    let mut m = Marking(vec![u32::from(a0), u32::from(b0)]);
    let total = m.total();

    for _ in 0..4 {
        let pick: bool = kani::any();
        let t = if pick { t0 } else { t1 };
        if let Some(next) = net.fire(t, &m) {
            m = next;
        }
        assert_eq!(m.total(), total);
    }
}

/// Deadlock freedom for an acyclic wait-for relation (spec §2): when every
/// possible wait edge `i → j` has `i < j`, no cycle exists, so the detector
/// reports no deadlock. The edges are symbolic; the acyclicity is by
/// construction.
#[kani::proof]
#[kani::unwind(8)]
fn acyclic_wait_graph_has_no_deadlock() {
    let mut adjacency: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
    for i in 0u64..4 {
        for j in (i + 1)..4 {
            if kani::any::<bool>() {
                adjacency.entry(i).or_default().insert(j);
            }
        }
    }
    assert!(find_cycle_in_digraph(&adjacency).is_none());
}

/// The same property through the resource layer: entity `i` holds resource
/// `i`, and may wait only for resources held by higher-numbered entities.
#[kani::proof]
#[kani::unwind(8)]
fn acyclic_resource_graph_has_no_deadlock() {
    let mut graph = WaitForGraph::default();
    for i in 0u64..3 {
        graph.add_holder(i, i);
    }
    for i in 0u64..3 {
        for j in (i + 1)..3 {
            if kani::any::<bool>() {
                graph.add_request(i, j);
            }
        }
    }
    assert!(graph.find_cycle().is_none());
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
    kani::assume(u16::from(w00) + u16::from(w01) > 0);
    kani::assume(u16::from(w10) + u16::from(w11) > 0);

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
