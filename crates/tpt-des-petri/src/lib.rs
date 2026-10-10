//! # tpt-des-petri
//!
//! Petri nets for concurrent and distributed systems:
//!
//! * [`net`] — place/transition nets with weighted arcs and token
//!   markings; [`fire`](net::PetriNet::fire) semantics.
//! * [`reach`] — reachability graph generation by BFS with a state bound.
//! * [`invariant`] — P-invariant / T-invariant analysis via null-space
//!   elimination on the incidence matrix, plus conservation checks.
//! * [`spn`] — stochastic Petri nets with exponentially distributed firing
//!   times, simulated by the Gillespie direct method (competing
//!   exponentials drawn through `tpt-math-prob-dist`, transitions chosen by
//!   `tpt-math-prob-sampler`), and conversion of a bounded SPN into a
//!   [`Ctmc`](tpt_des_markov::Ctmc) for steady-state analysis.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations)]

extern crate alloc;

pub mod invariant;
pub mod net;
pub mod reach;
pub mod spn;

pub use net::{Marking, PetriNet};
pub use reach::{reachability, ReachabilityError, ReachabilityGraph};
pub use spn::StochasticPetriNet;
