# tpt-des

[![CI](https://github.com/tpt-solutions/tpt-des/actions/workflows/ci.yml/badge.svg)](https://github.com/tpt-solutions/tpt-des/actions/workflows/ci.yml)
![MSRV](https://img.shields.io/badge/MSRV-1.84-blue)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

A pure-Rust **discrete event simulation (DES)** and stochastic systems
library: event scheduling, queuing theory, Petri nets, Markov
chains, resource contention, process-oriented simulation, and rigorous
output statistics — **no C/C++ FFI and no external solvers**.

Part of the TPT ecosystem; numeric substrate is
[`tpt-math`](https://crates.io/crates/tpt-math) (in-house, MIT OR Apache-2.0).

## Design principles

- **Zero external solver dependencies.** Event calendars, priority queues,
  stochastic samplers, matrix exponentials, and confidence-interval numerics
  are implemented from scratch in pure Rust.
- **Strict licensing.** MIT OR Apache-2.0 throughout; `cargo-deny` blocks
  Apache-2.0-only crates and unknown sources.
- **Type-state & temporal safety.** Phantom-typed time units (`Ticks`,
  `Seconds`, `Hours`), compile-time event priorities (`const` generics),
  and `const`-capacity resource pools.
- **Formal verification, in two layers.** Kani bounded model-checking
  harnesses cover event-calendar ordering, entity conservation, Markov
  probability mass, Petri token conservation, and deadlock freedom for acyclic
  wait graphs (run with `just kani`; non-blocking in CI). `proptest` checks
  stochastic laws statistically.
- **`no_std`-friendly core.** `tpt-des-core` builds with
  `--no-default-features` for `thumbv6m-none-eabi` and
  `wasm32-unknown-unknown`. The other crates require `std`.

## Crates

| Crate | Purpose |
|-------|---------|
| [`tpt-des-core`](crates/tpt-des-core) | unit-safe clock, O(log n) event calendar with lazy cancellation, entity traits, seeded deterministic replay |
| [`tpt-des-queue`](crates/tpt-des-queue) | M/M/1, M/M/c, M/G/1 (Pollaczek–Khinchine), Erlang B/C, Jackson networks, MVA, DES queues (FIFO/LIFO/priority/PS) |
| [`tpt-des-resource`](crates/tpt-des-resource) | capacity pools, stores, fluid buffers, seize-delay-release with preemption & timeouts, deadlock detection |
| [`tpt-des-process`](crates/tpt-des-process) | `async`/`await` process-oriented simulation: delay, resource waits, condition waits, SimPy-style interruption, hybrid discrete–continuous hook |
| [`tpt-des-petri`](crates/tpt-des-petri) | place/transition nets, stochastic Petri nets (Gillespie), reachability graphs, P/T-invariants, CTMC conversion |
| [`tpt-des-markov`](crates/tpt-des-markov) | DTMC/CTMC validation, stationary solvers, matrix exponential, absorbing chains, first-passage times |
| [`tpt-des-stats`](crates/tpt-des-stats) | Welch & MSER warm-up detection, batch means, replication/deletion, regenerative methods, Student-t/Wilson/order-statistic CIs |
| [`tpt-des-verify`](crates/tpt-des-verify) | `proptest` strategies and Kani harnesses for the whole workspace |
| [`tpt-des`](crates/tpt-des) | feature-gated umbrella re-exporting everything |

## Quick start

```rust
use tpt_des::kernel::Time;
use tpt_des::queue::{run_queue, mm1, Discipline, Exponential, QueueConfig};

// Simulate an M/M/1 queue (λ = 0.75, μ = 1) and compare to theory.
let metrics = run_queue(
    QueueConfig {
        arrival: Box::new(Exponential::new(0.75).unwrap()),
        service: Box::new(Exponential::new(1.0).unwrap()),
        discipline: Discipline::Fifo,
        capacity: None,
        priority: None,
    },
    Time::from_raw(100_000.0), // horizon
    Time::from_raw(10_000.0),  // warm-up
    2024,      // seed: deterministic replay
);
let exact = mm1(0.75, 1.0).unwrap();
assert!((metrics.mean_sojourn().raw() - exact.w).abs() / exact.w < 0.15);
```

## Worked examples

```sh
cargo run -p tpt-des --example mm1_vs_erlang        # DES vs Erlang formulas
cargo run -p tpt-des --example spn_reachability     # stochastic Petri net → CTMC
cargo run -p tpt-des --example markov_absorption    # absorbing chains, first passage
cargo run -p tpt-des --example deadlock_detection   # seize-delay-release deadlock
```

## Verification & CI

```sh
just check    # fmt + clippy -D warnings + tests + cargo-deny
just kani     # bounded model checks (requires cargo-kani)
just no-std   # bare-metal build of the core crate
cargo bench   # criterion: calendar hot path + matrix exponential
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy, the full test suite, a
`thumbv6m-none-eabi` build of `tpt-des-core`, `cargo-deny`, and Kani
(non-blocking).

## License

Dual-licensed under `MIT OR Apache-2.0` (see `LICENSE-MIT` and
`LICENSE-APACHE`). Copyright TPT Solutions.
