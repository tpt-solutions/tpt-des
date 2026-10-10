//! # tpt-des-resource
//!
//! Modeling of shared resources, buffers, and contention:
//!
//! * [`pool`] — capacity-constrained resource pools, with a compile-time
//!   [`FinitePool`](pool::FinitePool) variant (`const` generics) and
//!   preemption policies.
//! * [`store`] — discrete [`Store`](store::Store) containers and continuous
//!   fluid [`Buffer`](store::Buffer)s.
//! * [`deadlock`] — resource-allocation-graph cycle detection (implemented
//!   locally: `tpt-math-graph` exposes shortest paths, topological sort, and
//!   max-flow, but no direct cycle detector).
//! * [`sdr`] — the seize-delay-release pattern with priority, preemption,
//!   timeouts, and cancellation, coordinated by a [`ResourceController`].
//!
//! The primitives here are engine-agnostic; drive them from `tpt-des-core`
//! entities or from `tpt-des-process` coroutines.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations)]

extern crate alloc;

pub mod deadlock;
pub mod error;
pub mod pool;
pub mod sdr;
pub mod store;

pub use error::ResourceError;
pub use sdr::{PreemptPolicy, RequestId, ResourceController, SeizeOutcome, SeizeRequest};
