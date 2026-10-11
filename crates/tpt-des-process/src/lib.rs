//! # tpt-des-process
//!
//! Process-oriented (coroutine-based) simulation on top of the
//! `tpt-des-core` engine, using Rust `async`/`await` as the yield
//! primitive:
//!
//! ```
//! use tpt_des_process::ProcessSystem;
//!
//! let mut sys = ProcessSystem::new(42);
//! sys.spawn(|sim| async move {
//!     for _ in 0..3 {
//!         sim.delay(tpt_des_core::Time::from_raw(2.0)).await?;
//!     }
//!     Ok(())
//! });
//! sys.run();
//! assert_eq!(sys.now().raw(), 6.0);
//! ```
//!
//! # Blocking operations
//!
//! * [`Sim::delay`] — suspend until a duration elapses.
//! * [`Sim::acquire`] / [`Sim::release`] — wait for units of a named
//!   resource (all-or-nothing, FIFO granting).
//! * [`Sim::wait_until`] — suspend until a predicate becomes true
//!   (re-checked on [`Sim::notify_all`], never by polling).
//!
//! # Interruption (SimPy-style)
//!
//! [`Sim::interrupt`] makes the *pending* operation of a process resolve
//! with `Err(Interrupt)` — mirroring SimPy's `InterruptedException`: the
//! coroutine body decides whether to recover (and continue awaiting further
//! operations, i.e. resume) or to abort by returning the error. A process
//! may interrupt itself.
//!
//! # Hybrid discrete–continuous hook
//!
//! The [`hybrid`] module defines the seam where a continuous solver plugs
//! in: a [`ContinuousModel`](hybrid::ContinuousModel) integrates between
//! discrete events and interrupts a process when a guard crosses a
//! threshold. (`tpt-control` is the intended upstream solver; that repo
//! does not exist yet, so the trait seam lives here — see the Phase 2
//! checkpoint note in `todo.md`.)
//!
//! This crate is **std-only** by design: process wakers use
//! `std::task::Wake` (safe code), and the workspace forbids `unsafe`.

#![warn(missing_docs, missing_debug_implementations)]

extern crate alloc;

pub mod hybrid;
pub mod ops;
pub mod system;

pub use ops::{Acquire, Delay, Interrupt, WaitUntil};
pub use system::{ProcessId, ProcessOutput, ProcessSystem, Sim};

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}
