//! Entity lifecycle traits and the per-event context handed to entities.

use crate::calendar::EventCalendar;
use crate::rng::SimRng;
use crate::sim::SimClock;
use crate::time::Time;
use tpt_math_prob_core::Rng as _;

/// Identifies an entity within a [`Simulation`](crate::Simulation).
///
/// Ids are assigned in registration order starting at zero, so they are
/// stable across deterministic replays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub usize);

impl EntityId {
    /// The raw numeric id.
    pub fn raw(self) -> usize {
        self.0
    }
}

/// The context handed to an entity for each lifecycle hook.
///
/// It bundles the current time plus mutable access to the clock, the event
/// calendar, and the simulation RNG — everything an entity needs to react to
/// an event — without exposing the other entities.
pub struct Ctx<'a, Unit, E> {
    /// Time at which the current event fires.
    pub now: Time<Unit>,
    /// The simulation clock (`now` mirrored, advanced before dispatch).
    pub clock: &'a mut SimClock<Unit>,
    /// Schedule new events or cancel pending ones.
    pub calendar: &'a mut EventCalendar<Unit, E>,
    /// The seeded simulation RNG.
    pub rng: &'a mut SimRng,
}

impl<'a, Unit, E> core::fmt::Debug for Ctx<'a, Unit, E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ctx").field("now", &self.now).finish()
    }
}

impl<'a, Unit, E> Ctx<'a, Unit, E> {
    /// Schedule `payload` for `target` at absolute time `at`.
    pub fn schedule_at<const PRIORITY: u8>(
        &mut self,
        at: Time<Unit>,
        target: EntityId,
        payload: E,
    ) -> crate::calendar::EventId {
        self.calendar.schedule_at::<PRIORITY>(at, target, payload)
    }

    /// Schedule `payload` for `target` after a relative `delay` from now.
    pub fn schedule<const PRIORITY: u8>(
        &mut self,
        delay: Time<Unit>,
        target: EntityId,
        payload: E,
    ) -> crate::calendar::EventId {
        self.calendar
            .schedule_at::<PRIORITY>(self.now + delay, target, payload)
    }

    /// Cancel a pending event (no-op if it already fired).
    pub fn cancel(&mut self, id: crate::calendar::EventId) -> bool {
        self.calendar.cancel(id)
    }

    /// Next uniform `f64` in `[0, 1)` from the simulation RNG.
    pub fn uniform(&mut self) -> f64 {
        self.rng.next_f64()
    }
}

/// A simulated entity with lifecycle hooks.
///
/// Entities are registered with a [`Simulation`](crate::Simulation) by
/// `&mut` reference, stay owned by the caller (so metrics collected in entity
/// fields remain readable after the run), and receive events addressed to
/// them.
///
/// * [`on_create`](Entity::on_create) runs once, in registration order,
///   before the first event is dispatched.
/// * [`on_event`](Entity::on_event) runs for every event addressed to the
///   entity.
/// * [`on_terminate`](Entity::on_terminate) runs once, in registration
///   order, when [`finalize`](crate::Simulation::finalize) is called.
pub trait Entity<Unit, E> {
    /// Called once at simulation start (before any event dispatch).
    fn on_create(&mut self, _ctx: &mut Ctx<'_, Unit, E>) {}

    /// Called for each event addressed to this entity.
    fn on_event(&mut self, event: E, ctx: &mut Ctx<'_, Unit, E>);

    /// Called once at simulation end, in registration order.
    fn on_terminate(&mut self, _ctx: &mut Ctx<'_, Unit, E>) {}
}
