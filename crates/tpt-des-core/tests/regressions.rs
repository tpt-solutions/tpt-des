//! Regression tests for the Phase 6 core fixes.

use tpt_des_core::{Ctx, Entity, EntityId, Seconds, SimRng, Simulation, Time};
use tpt_math_prob_core::Rng as _;

#[derive(Default)]
struct Counter {
    hits: u64,
}

impl Entity<Seconds, u32> for Counter {
    fn on_event(&mut self, _event: u32, _ctx: &mut Ctx<'_, Seconds, u32>) {
        self.hits += 1;
    }
}

/// `run_until` used to leave the clock at the last event before the horizon.
/// It now reads the horizon, so an idle window still advances time.
#[test]
fn run_until_advances_clock_to_horizon() {
    let mut counter = Counter::default();
    let mut sim: Simulation<'_, Seconds, u32> = Simulation::new(1);
    sim.add_entity(&mut counter);
    sim.run_until(Time::<Seconds>::from_raw(5.0));
    assert_eq!(sim.now().raw(), 5.0);
}

/// Later events are still pending after `run_until` stops at the horizon,
/// and the clock never moves past them.
#[test]
fn run_until_does_not_pass_pending_events() {
    let mut counter = Counter::default();
    let mut sim: Simulation<'_, Seconds, u32> = Simulation::new(2);
    let me = sim.add_entity(&mut counter);
    sim.start_at::<0>(Time::<Seconds>::from_raw(1.0), me, 1);
    sim.start_at::<0>(Time::<Seconds>::from_raw(9.0), me, 2);
    sim.run_until(Time::<Seconds>::from_raw(5.0));
    assert_eq!(sim.now().raw(), 5.0);
    assert_eq!(sim.pending_events(), 1);
    sim.run_until(Time::<Seconds>::from_raw(10.0));
    assert_eq!(sim.now().raw(), 10.0);
    assert_eq!(sim.pending_events(), 0);
}

/// Events addressed to an unregistered id used to vanish without a trace.
/// They are counted now.
#[test]
fn events_to_unknown_entities_are_counted() {
    let mut counter = Counter::default();
    let mut sim: Simulation<'_, Seconds, u32> = Simulation::new(3);
    let me = sim.add_entity(&mut counter);
    sim.start_at::<0>(Time::<Seconds>::from_raw(1.0), me, 1);
    sim.start_at::<0>(Time::<Seconds>::from_raw(2.0), EntityId(7), 2);
    sim.run_until(Time::<Seconds>::from_raw(3.0));
    let stats = sim.stats();
    assert_eq!(stats.dropped_events, 1);
    assert_eq!(stats.events_total, 2);
}

/// `core_mut` hands out the raw generator, bypassing the counter, so the
/// determinism fingerprint did not change. Each access now counts.
#[test]
fn core_mut_access_is_counted_as_a_draw() {
    let mut rng = SimRng::new(4);
    assert_eq!(rng.draws(), 0);
    let _ = rng.core_mut().next_u64();
    assert_eq!(rng.draws(), 1);
}

#[test]
#[should_panic(expected = "simulation time must be finite")]
fn non_finite_time_is_rejected_in_all_profiles() {
    let _ = Time::<Seconds>::from_raw(f64::NAN);
}
