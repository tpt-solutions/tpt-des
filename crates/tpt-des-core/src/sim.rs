//! The simulation loop: clock, entities, calendar, and RNG in one place.
//!
//! [`Simulation`] owns the event loop and drives registered [`Entity`]s:
//!
//! ```
//! use tpt_des_core::{Entity, EntityId, Simulation, Time, Ticks};
//!
//! #[derive(Debug)]
//! struct Beacon {
//!     period: f64,
//!     ticks: u64,
//! }
//!
//! impl Entity<Ticks, ()> for Beacon {
//!     fn on_create(&mut self, ctx: &mut tpt_des_core::Ctx<Ticks, ()>) {
//!         let me = EntityId(0); // single-entity model
//!         ctx.schedule::<0>(Time::from_raw(self.period), me, ());
//!     }
//!     fn on_event(&mut self, (): (), ctx: &mut tpt_des_core::Ctx<Ticks, ()>) {
//!         self.ticks += 1;
//!         let me = EntityId(0);
//!         ctx.schedule::<0>(Time::from_raw(self.period), me, ());
//!     }
//! }
//!
//! let mut beacon = Beacon { period: 2.0, ticks: 0 };
//! let mut sim = Simulation::new(0);
//! let _id = sim.add_entity(&mut beacon);
//! sim.run_until(Time::from_raw(10.0));
//! assert_eq!(beacon.ticks, 5); // fired at t = 2, 4, 6, 8, 10
//! ```

use crate::calendar::EventCalendar;
use crate::entity::{Ctx, Entity, EntityId};
use crate::rng::SimRng;
use crate::time::Time;
use alloc::vec::Vec;
use core::fmt;

/// The simulation clock: the current time, only ever advanced.
pub struct SimClock<Unit> {
    now: Time<Unit>,
}

impl<Unit> fmt::Debug for SimClock<Unit> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimClock").field("now", &self.now).finish()
    }
}

impl<Unit> SimClock<Unit> {
    /// A clock stopped at the origin.
    pub fn new() -> Self {
        Self { now: Time::ZERO }
    }

    /// The current simulation time.
    pub fn now(&self) -> Time<Unit> {
        self.now
    }

    /// Advance the clock to `t`.
    ///
    /// # Panics
    ///
    /// In all build profiles, if `t` is earlier than the current time:
    /// simulation time must never move backwards.
    pub fn advance_to(&mut self, t: Time<Unit>) {
        assert!(
            t >= self.now,
            "simulation time moved backwards: {:?} < {:?}",
            t.raw(),
            self.now.raw()
        );
        self.now = t;
    }
}

impl<Unit> Default for SimClock<Unit> {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics accumulated by a simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunStats {
    /// Cumulative events dispatched since the simulation was created.
    pub events_total: u64,
    /// Events whose target [`EntityId`] was never registered. They are
    /// dropped rather than dispatched; a non-zero count almost always means a
    /// modelling bug (a stale or foreign id), so check it after a run.
    pub dropped_events: u64,
}

/// A discrete event simulation over entities of one payload type `E`.
///
/// Entities are registered by `&mut` reference and keep their owner, so any
/// metrics they accumulate remain readable after the run. The simulation is
/// deterministic: the same seed, the same entity registration order, and the
/// same schedule calls reproduce the same event trace.
pub struct Simulation<'ent, Unit, E> {
    clock: SimClock<Unit>,
    calendar: EventCalendar<Unit, E>,
    rng: SimRng,
    entities: Vec<&'ent mut dyn Entity<Unit, E>>,
    initialized: bool,
    finalized: bool,
    stats: RunStats,
}

impl<'ent, Unit, E> fmt::Debug for Simulation<'ent, Unit, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Simulation")
            .field("now", &self.clock.now())
            .field("pending_events", &self.calendar.live_len())
            .field("entities", &self.entities.len())
            .field("rng", &self.rng)
            .field("stats", &self.stats)
            .finish()
    }
}

impl<'ent, Unit, E> Simulation<'ent, Unit, E> {
    /// Create a simulation with the given RNG seed.
    pub fn new(seed: u64) -> Self {
        Self {
            clock: SimClock::new(),
            calendar: EventCalendar::new(),
            rng: SimRng::new(seed),
            entities: Vec::new(),
            initialized: false,
            finalized: false,
            stats: RunStats::default(),
        }
    }

    /// Register an entity; returns its id (registration order).
    ///
    /// Entities must be registered before the first event is dispatched.
    pub fn add_entity(&mut self, entity: &'ent mut dyn Entity<Unit, E>) -> EntityId {
        assert!(
            !self.initialized,
            "entities must be added before the simulation starts"
        );
        self.entities.push(entity);
        EntityId(self.entities.len() - 1)
    }

    /// Schedule the first event of the simulation (before running).
    pub fn start_at<const PRIORITY: u8>(
        &mut self,
        at: Time<Unit>,
        target: EntityId,
        payload: E,
    ) -> crate::calendar::EventId {
        assert!(!self.initialized, "cannot start a running simulation");
        self.calendar.schedule_at::<PRIORITY>(at, target, payload)
    }

    /// Current simulation time.
    pub fn now(&self) -> Time<Unit> {
        self.clock.now()
    }

    /// The simulation RNG.
    pub fn rng_mut(&mut self) -> &mut SimRng {
        &mut self.rng
    }

    /// Number of events currently scheduled (live, not cancelled).
    pub fn pending_events(&self) -> usize {
        self.calendar.live_len()
    }

    /// Cumulative run statistics.
    pub fn stats(&self) -> RunStats {
        self.stats
    }

    /// Call [`on_create`](Entity::on_create) on every entity, in
    /// registration order. Idempotent; invoked automatically before the
    /// first dispatch.
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        let Simulation {
            clock,
            calendar,
            rng,
            entities,
            ..
        } = self;
        let now = clock.now();
        let mut ctx = Ctx {
            now,
            clock,
            calendar,
            rng,
        };
        for entity in entities.iter_mut() {
            entity.on_create(&mut ctx);
        }
    }

    /// Call [`on_terminate`](Entity::on_terminate) on every entity, in
    /// registration order. Idempotent.
    pub fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;
        self.init();
        let Simulation {
            clock,
            calendar,
            rng,
            entities,
            ..
        } = self;
        let now = clock.now();
        let mut ctx = Ctx {
            now,
            clock,
            calendar,
            rng,
        };
        for entity in entities.iter_mut() {
            entity.on_terminate(&mut ctx);
        }
    }

    /// Dispatch the next event, if any. Returns `false` when the calendar is
    /// exhausted.
    pub fn step(&mut self) -> bool {
        self.init();
        match self.calendar.pop() {
            Some(popped) => {
                self.dispatch(popped.time, popped.target, popped.payload);
                true
            }
            None => false,
        }
    }

    /// Run until `horizon` (events scheduled exactly at the horizon do
    /// fire). Returns the number of events processed by this call.
    ///
    /// On return the clock reads `horizon` (when it is finite and later than
    /// the last event), so [`now`](Self::now) reports the end of the window
    /// rather than the time of the last event before it.
    pub fn run_until(&mut self, horizon: Time<Unit>) -> u64 {
        self.init();
        let before = self.stats.events_total;

        while let Some(popped) = self.calendar.pop_until(horizon) {
            self.dispatch(popped.time, popped.target, popped.payload);
        }
        if horizon.raw().is_finite() && horizon > self.clock.now() {
            self.clock.advance_to(horizon);
        }
        self.stats.events_total - before
    }

    /// Run until the calendar is empty or `max_events` events have fired.
    /// Returns the number of events processed by this call.
    pub fn run(&mut self, max_events: u64) -> u64 {
        let mut processed = 0;
        while processed < max_events && self.step() {
            processed += 1;
        }
        processed
    }

    fn dispatch(&mut self, time: Time<Unit>, target: EntityId, payload: E) {
        self.clock.advance_to(time);
        self.stats.events_total += 1;
        let Simulation {
            clock,
            calendar,
            rng,
            entities,
            ..
        } = self;
        let now = clock.now();
        let mut ctx = Ctx {
            now,
            clock,
            calendar,
            rng,
        };
        match entities.get_mut(target.raw()) {
            Some(entity) => entity.on_event(payload, &mut ctx),
            None => self.stats.dropped_events += 1,
        }
    }
}
