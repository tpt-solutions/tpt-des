//! # tpt-des-core
//!
//! The foundation layer of the `tpt-des` discrete event simulation library:
//! the unit-safe simulation clock, the from-scratch event calendar, the
//! [`Entity`] lifecycle trait, and the deterministic simulation loop.
//!
//! # Design principles
//!
//! * **Unit-safe time** ([`time`]). Phantom-typed [`Time<Unit>`] distinguishes
//!   simulation ticks, seconds, and hours at compile time, so a lead time in
//!   hours can never be scheduled as a tick.
//! * **O(log n) event calendar** ([`calendar`]). A binary heap built from
//!   scratch with lazy cancellation: scheduling and cancelling are both
//!   `O(log n)`, and events pop in `(time, priority, insertion order)` order.
//! * **Deterministic replay** ([`rng`]). Every simulation is driven by a
//!   seeded [`SimRng`] (a `SplitMix64` from `tpt-math-prob-core`); re-running
//!   with the same seed reproduces the same event trace.
//! * **`no_std` + `alloc`.** The crate is `no_std` compatible and only needs
//!   an allocator; enable the `alloc` feature instead of `std` for embedded
//!   targets.
//!
//! # Examples
//!
//! A two-entity ping-pong simulation. Entities are registered by `&mut`
//! reference in a fixed order (their [`EntityId`] is that order), so entities
//! can address each other by index and keep all metrics after the run:
//!
//! ```
//! use tpt_des_core::{Entity, EntityId, Simulation, Time, Ticks};
//!
//! #[derive(Debug)]
//! enum Msg {
//!     Ping(u64),
//!     Pong(u64),
//! }
//!
//! struct Player {
//!     peer: EntityId,
//!     received: u64,
//! }
//!
//! impl Entity<Ticks, Msg> for Player {
//!     fn on_event(&mut self, event: Msg, ctx: &mut tpt_des_core::Ctx<Ticks, Msg>) {
//!         self.received += 1;
//!         let n = match event {
//!             Msg::Ping(n) => n,
//!             Msg::Pong(n) => n,
//!         };
//!         if n < 4 {
//!             let reply = if matches!(event, Msg::Ping(_)) {
//!                 Msg::Pong(n + 1)
//!             } else {
//!                 Msg::Ping(n + 1)
//!             };
//!             ctx.schedule_at::<0>(ctx.now + Time::from_raw(1.0), self.peer, reply);
//!         }
//!     }
//! }
//!
//! let mut a = Player { peer: EntityId(1), received: 0 }; // second entity
//! let mut b = Player { peer: EntityId(0), received: 0 }; // first entity
//!
//! let mut sim = Simulation::new(42);
//! let _a_id = sim.add_entity(&mut a); // EntityId(0)
//! let _b_id = sim.add_entity(&mut b); // EntityId(1)
//! sim.start_at::<0>(Time::from_raw(0.0), EntityId(0), Msg::Ping(0));
//! sim.run_until(Time::from_raw(10.0));
//! assert_eq!(a.received, 3); // Pings at t = 0, 2, 4
//! assert_eq!(b.received, 2); // Pongs at t = 1, 3
//! ```
//!
//! Note that entities are added by `&mut` reference and stay owned by the
//! caller, which keeps borrowing simple; see [`Simulation::add_entity`].

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations)]

extern crate alloc;

pub mod calendar;
pub mod entity;
pub mod rng;
pub mod sim;
pub mod time;

pub use calendar::{EventCalendar, EventId};
pub use entity::{Ctx, Entity, EntityId};
pub use rng::SimRng;
pub use sim::{RunStats, Simulation};
pub use time::{Hours, Seconds, Ticks, Time};

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}
