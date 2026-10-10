//! Property-based verification of the event calendar.
//!
//! These proptests statistically verify the stochastic/behavioral laws the
//! Kani harnesses in `src/proofs.rs` prove for small symbolic schedules:
//! pops come out in `(time, priority, sequence)` order, cancelled events
//! never fire, and the live-count bookkeeping is exact.

use proptest::prelude::*;
use tpt_des_core::{EntityId, EventCalendar, Ticks, Time};
use tpt_des_verify::strategies::{cancellation_mask, schedule_plan};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn pops_are_sorted_by_time_priority_seq(
        plan in schedule_plan(48),
        mask in cancellation_mask(48),
    ) {
        let mut cal: EventCalendar<Ticks, usize> = EventCalendar::new();
        let ids: Vec<_> = plan
            .iter()
            .map(|&(t, _)| cal.schedule_at::<0>(Time::from_raw(t as f64), EntityId(0), t as usize))
            .collect();
        let mut cancelled = 0usize;
        for (i, &cancel) in mask.iter().enumerate() {
            if cancel && i < ids.len() && cal.cancel(ids[i]) {
                cancelled += 1;
            }
        }

        let mut last = (0.0f64, 0u8, 0u64);
        let mut fired = 0usize;
        while let Some(ev) = cal.pop() {
            let key = (ev.time, ev.priority, ev.id.raw());
            prop_assert!(key >= last, "pop order violated: {:?} after {:?}", key, last);
            last = key;
            fired += 1;
        }
        prop_assert_eq!(fired + cancelled, plan.len());
        prop_assert!(cal.is_empty());
    }

    #[test]
    fn live_len_tracks_schedule_and_cancel(
        plan in schedule_plan(32),
        mask in cancellation_mask(32),
    ) {
        let mut cal: EventCalendar<Ticks, usize> = EventCalendar::new();
        let n = plan.len();
        let ids: Vec<_> = plan
            .iter()
            .map(|&(t, _)| cal.schedule_at::<0>(Time::from_raw(t as f64), EntityId(0), t as usize))
            .collect();
        prop_assert_eq!(cal.live_len(), n);
        let mut cancelled = 0usize;
        for (i, &cancel) in mask.iter().enumerate() {
            if cancel && i < ids.len() && cal.cancel(ids[i]) {
                cancelled += 1;
            }
            prop_assert_eq!(cal.live_len(), n - cancelled);
        }
    }
}

/// Priority ordering (compile-time `const` generic) at equal times.
#[test]
fn lower_priority_number_fires_first() {
    let mut cal: EventCalendar<Ticks, char> = EventCalendar::new();
    cal.schedule_at::<3>(Time::from_raw(1.0), EntityId(0), 'a');
    cal.schedule_at::<1>(Time::from_raw(1.0), EntityId(0), 'b');
    cal.schedule_at::<2>(Time::from_raw(1.0), EntityId(0), 'c');
    let order: Vec<char> = core::iter::from_fn(|| cal.pop().map(|e| e.payload)).collect();
    assert_eq!(order, ['b', 'c', 'a']);
}

/// Deterministic replay: identical seed + identical schedule → identical trace.
#[test]
fn simulation_replay_is_bit_exact() {
    use tpt_des_core::{Ctx, Entity, Simulation};

    struct Sampler {
        samples: Vec<f64>,
    }

    impl Entity<Ticks, ()> for Sampler {
        fn on_event(&mut self, (): (), ctx: &mut Ctx<Ticks, ()>) {
            self.samples.push(ctx.uniform());
            if self.samples.len() < 32 {
                ctx.schedule::<0>(Time::from_raw(1.0), EntityId(0), ());
            }
        }
    }

    fn run() -> Vec<f64> {
        let mut entity = Sampler {
            samples: Vec::new(),
        };
        let mut sim = Simulation::new(0xDEADBEEF);
        let _id = sim.add_entity(&mut entity);
        sim.start_at::<0>(Time::from_raw(0.0), EntityId(0), ());
        sim.run(64);
        entity.samples
    }

    let first = run();
    let second = run();
    assert_eq!(first, second);
    assert_eq!(first.len(), 32);
}
