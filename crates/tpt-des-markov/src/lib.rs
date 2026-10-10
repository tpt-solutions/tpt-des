//! # tpt-des-markov
//!
//! Discrete- and continuous-time Markov chain analysis, built on
//! `tpt-math-prob-markov` (simulation, normalization) and
//! `tpt-math-linalg-dense` (linear solves, matrix products):
//!
//! * [`dtmc::Dtmc`] — validated row-stochastic transition matrices,
//!   distribution evolution, and steady-state probabilities by power
//!   iteration on the in-house [`DMatrix`](tpt_math_linalg_dense::DMatrix).
//! * [`ctmc::Ctmc`] — validated generator matrices `Q`, the stationary
//!   distribution via the augmented solve `πQ = 0, Σπ = 1`, and transient
//!   probabilities via a from-scratch scaling-and-squaring matrix
//!   exponential.
//! * [`absorb`] — absorbing chains: canonical form, fundamental matrix
//!   `N = (I−Q)⁻¹`, expected time to absorption, absorption probabilities.
//! * [`fpt`] — mean first-passage times and mean recurrence times.
//!
//! Every constructor enforces its mathematical preconditions
//! (row-stochasticity, `Q` row sums of zero, …) and returns errors instead
//! of silently normalizing garbage.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations)]

extern crate alloc;

pub mod absorb;
pub mod ctmc;
pub mod dtmc;
pub mod error;
pub mod fpt;

pub use ctmc::Ctmc;
pub use dtmc::Dtmc;
pub use error::MarkovError;
