//! # tpt-des-verify
//!
//! Verification harnesses for the `tpt-des` workspace, per the spec's
//! "native formal verification" principle:
//!
//! * [`strategies`] — reusable `proptest` strategies for generating valid
//!   queuing parameters, row-stochastic transition matrices, queuing-network
//!   routing matrices, Petri net topologies, and schedule plans.
//! * Calendar property tests — statistical verification that pops come out in
//!   `(time, priority, sequence)` order, cancelled events never fire, and the
//!   live-count bookkeeping is exact (see the `tests` in this crate).
//! * Kani bounded model checks (module [`proofs`], compiled only under
//!   `cfg(kani)`) — machine-checked proofs of the same invariants for small
//!   symbolic schedules. Run with `cargo kani -p tpt-des-verify`.
//!
//! This crate is std-only (proptest needs std); it is a development
//! dependency by intent — downstream simulators normally depend on
//! `tpt-des-core` etc. and pull strategies from here only in their tests.

#![warn(missing_docs, missing_debug_implementations)]

pub mod strategies;

#[cfg(kani)]
mod proofs;
