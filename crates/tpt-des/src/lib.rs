//! # tpt-des
//!
//! A pure-Rust discrete event simulation (DES) and stochastic
//! systems library: event scheduling, queuing theory, Petri nets, Markov
//! chains, resource contention, process-oriented simulation, and rigorous
//! output statistics — no C/C++ FFI, no external solvers.
//!
//! Feature-gated modules (all on by default except `verify`):
//!
//! | Feature   | Crate               | Contents |
//! |-----------|---------------------|----------|
//! | `core`    | [`tpt_des_core`]    | unit-safe clock, event calendar, entities, deterministic RNG |
//! | `queue`   | [`tpt_des_queue`]   | M/M/1, M/M/c, M/G/1, Erlang B/C, Jackson networks, DES queues |
//! | `resource`| [`tpt_des_resource`]| pools, stores, fluid buffers, seize-delay-release, deadlock detection |
//! | `process` | [`tpt_des_process`] | async/await coroutines, interruption, hybrid hook |
//! | `petri`   | [`tpt_des_petri`]   | P/T nets, stochastic nets (Gillespie), reachability, invariants |
//! | `markov`  | [`tpt_des_markov`]  | DTMC/CTMC, matrix exponential, absorbing chains, first passage |
//! | `stats`   | [`tpt_des_stats`]   | Welch/MSER warm-up, batch means, confidence intervals |
//! | `verify`  | [`tpt_des_verify`]  | proptest strategies + Kani bounded model checks |
//!
//! # Examples
//!
//! Simulate an M/M/1 queue and compare against the analytical formula:
//!
//! ```
//! # #[cfg(all(feature = "queue", feature = "std"))]
//! # fn main() {
//! use tpt_des::queue::{run_queue, mm1, Discipline, Exponential, QueueConfig};
//!
//! let (lambda, mu) = (0.5, 1.0);
//! let metrics = run_queue(
//!     QueueConfig {
//!         arrival: Box::new(Exponential::new(lambda).unwrap()),
//!         service: Box::new(Exponential::new(mu).unwrap()),
//!         discipline: Discipline::Fifo,
//!         capacity: None,
//!         priority: None,
//!     },
//!     20_000.0,
//!     2_000.0,
//!     42,
//! );
//! let exact = mm1(lambda, mu).unwrap();
//! assert!((metrics.mean_sojourn() - exact.w).abs() / exact.w < 0.25);
//! # }
//! # #[cfg(not(all(feature = "queue", feature = "std")))]
//! # fn main() {}
//! ```
//!
//! Markov chain steady state:
//!
//! ```
//! # #[cfg(all(feature = "markov", feature = "std"))]
//! # fn main() {
//! use tpt_des::markov::Dtmc;
//!
//! let chain = Dtmc::from_rows(vec![vec![0.9, 0.1], vec![0.5, 0.5]]).unwrap();
//! let pi = chain.stationary(1e-12, 10_000).unwrap();
//! assert!((pi[0] - 5.0 / 6.0).abs() < 1e-9);
//! # }
//! # #[cfg(not(all(feature = "markov", feature = "std")))]
//! # fn main() {}
//! ```

#![cfg_attr(not(feature = "core"), deny(unknown_lints))]
#![warn(missing_docs, missing_debug_implementations)]

#[cfg(feature = "core")]
pub use tpt_des_core as kernel;
#[cfg(feature = "markov")]
pub use tpt_des_markov as markov;
#[cfg(feature = "petri")]
pub use tpt_des_petri as petri;
#[cfg(feature = "process")]
pub use tpt_des_process as process;
#[cfg(feature = "queue")]
pub use tpt_des_queue as queue;
#[cfg(feature = "resource")]
pub use tpt_des_resource as resource;
#[cfg(feature = "stats")]
pub use tpt_des_stats as stats;
#[cfg(feature = "verify")]
pub use tpt_des_verify as verify;
