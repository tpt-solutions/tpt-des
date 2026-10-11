//! # tpt-des-stats
//!
//! Rigorous statistical analysis of simulation outputs:
//!
//! * [`dist`] — the numerics under everything: Lanczos log-gamma,
//!   regularized incomplete beta, Student-t and normal CDF/quantiles —
//!   from scratch, no external solvers.
//! * [`warmup`] — warm-up (initial transient) detection: Welch's
//!   cross-replication averaging with moving-average smoothing, and MSER
//!   (the spec's "MSIR"), the marginal-standard-error rule.
//! * [`batch`] — output analysis: batch means, replication/deletion, and
//!   regenerative (cycle) estimation.
//! * [`ci`] — confidence intervals for means (Student-t), proportions
//!   (Wilson), and quantiles (order statistics).
//! * [`timeseries`] — running (Welford) statistics, time-weighted averages
//!   for queue lengths and utilization, autocorrelation.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations)]

extern crate alloc;

pub mod batch;
pub mod ci;
pub mod dist;
pub mod error;
pub mod timeseries;
pub mod warmup;

pub use error::StatsError;
