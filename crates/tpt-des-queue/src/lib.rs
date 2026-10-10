//! # tpt-des-queue
//!
//! Analytical and simulation-based queuing theory.
//!
//! * Closed-form solutions ([`analytic`]): M/M/1, M/M/c, M/G/1
//!   (Pollaczek–Khinchine), and Erlang B/C loss formulas.
//! * Networks of queues ([`network`]): open Jackson networks with routing
//!   matrices, and single-chain closed networks solved by mean value analysis.
//! * Discrete-event simulation ([`sim`]): single-station queues with
//!   arbitrary inter-arrival and service distributions under FIFO, LIFO,
//!   priority, and processor-sharing disciplines.
//!
//! The simulation and the analysis meet in the integration tests: a seeded
//! M/M/1 run reproduces the analytical waiting time within a statistical
//! tolerance, checked by `proptest` across the stable regime
//! (`tpt-des-verify` supplies the parameter strategies).

#![warn(missing_docs, missing_debug_implementations)]

pub mod analytic;
pub mod error;
pub mod network;
pub mod sim;

pub use analytic::{erlang_b, erlang_c, mg1, mm1, mm1_pn, mmc, Mg1Result, Mm1Result, MmcResult};
pub use error::QueueError;
pub use network::{ClosedNetwork, JacksonNetwork};
pub use sim::{
    run_queue, Discipline, Exponential, QueueConfig, QueueEvent, QueueMetrics, RateSampler,
    SimQueue,
};
