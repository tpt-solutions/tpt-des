# tpt-des

The umbrella crate for the `tpt-des` workspace. It re-exports every member crate behind a feature flag, so one dependency covers the parts you use.

A pure-Rust discrete-event simulation library: event scheduling, queueing theory, Petri nets, Markov chains, resource contention, process-oriented simulation, output statistics, and verification harnesses. There are no C or C++ dependencies and no external solvers.

## Install

```toml
[dependencies]
tpt-des = { version = "0.1", features = ["queue", "std"] }
```

The defaults (`std` plus every module except `verify`) are enabled if you write `tpt-des = "0.1"`.

## Quick start

Erlang C gives the probability that a call waits, for ten agents and 6 erlangs of traffic. Check it against the M/M/c formula:

```rust
use tpt_des::queue::{erlang_c, mmc};

let p_wait = erlang_c(6.0, 10).unwrap();
let analytic = mmc(6.0, 1.0, 10).unwrap();
assert!((p_wait - analytic.erlang_c).abs() < 1e-12);
```

## Modules

Each enabled feature exposes one module:

| Module | Feature | Crate | Covers |
|---|---|---|---|
| `tpt_des::kernel` | `core` | [`tpt-des-core`](../tpt-des-core/README.md) | Typed time, the event calendar, entities, seeded RNG |
| `tpt_des::queue` | `queue` | [`tpt-des-queue`](../tpt-des-queue/README.md) | M/M/1, M/M/c, M/G/1, Erlang B and C, networks, DES queues |
| `tpt_des::resource` | `resource` | [`tpt-des-resource`](../tpt-des-resource/README.md) | Pools, stores, buffers, seize–delay–release, deadlock detection |
| `tpt_des::process` | `process` | [`tpt-des-process`](../tpt-des-process/README.md) | `async` process-oriented simulation, hybrid bridge |
| `tpt_des::petri` | `petri` | [`tpt-des-petri`](../tpt-des-petri/README.md) | Petri nets, stochastic simulation, reachability, invariants |
| `tpt_des::markov` | `markov` | [`tpt-des-markov`](../tpt-des-markov/README.md) | DTMC and CTMC analysis, absorption, first passage |
| `tpt_des::stats` | `stats` | [`tpt-des-stats`](../tpt-des-stats/README.md) | Confidence intervals, batch means, warm-up detection |
| `tpt_des::verify` | `verify` | [`tpt-des-verify`](../tpt-des-verify/README.md) | proptest strategies and Kani harnesses |

## Feature flags

| Feature | Default | Enables |
|---|---|---|
| `std` | yes | `std` in each enabled member. Required by every feature except `core`. |
| `core` | yes | `tpt-des-core`, the only no_std-capable member. |
| `queue` | yes | `tpt-des-queue`. Implies `core`. |
| `resource` | yes | `tpt-des-resource`. Implies `core`. |
| `process` | yes | `tpt-des-process`. Implies `core`. |
| `petri` | yes | `tpt-des-petri`. Implies `core` and `markov`. |
| `markov` | yes | `tpt-des-markov`. |
| `stats` | yes | `tpt-des-stats`. |
| `verify` | no | `tpt-des-verify` and the members it tests. Implies `std`. |
| `parallel` | no | Rayon-backed replications in `tpt-des-queue`. Implies `queue` and `std`. |

`std` does not pull in every optional crate. Each `std` entry is conditional on that member being enabled.

## Minimal no_std use

```toml
[dependencies]
tpt-des = { version = "0.1", default-features = false, features = ["core"] }
```

Only the `core` feature works without `std`. The queue, resource, Petri, Markov, and statistics numerics depend on `ln`, `exp`, and `sqrt` from `std` or `alloc`, so enable `std` alongside them.

## Examples

The examples live in [`examples/`](examples/). Each one declares the features it needs:

```sh
cargo run -p tpt-des --example mm1_vs_erlang --features "std queue"
cargo run -p tpt-des --example spn_reachability --features "std petri"
cargo run -p tpt-des --example process_bank_teller --features "std process queue"
```

| Example | Shows |
|---|---|
| `mm1_vs_erlang` | Simulation checked against the closed-form M/M/1 and Erlang results |
| `process_bank_teller` | `async` processes, checked against Erlang C |
| `stats_replications` | A confidence interval over independent replications |
| `hybrid_bridge` | A continuous model interrupting a process at a threshold |
| `call_center_erlang` | Erlang B and C for sizing a call centre |
| `inventory_store` | A `Store` with a lead time |
| `jackson_network` | Traffic equations and mean value analysis |
| `markov_absorption` | Absorbing chains and first-passage times |
| `spn_reachability` | A stochastic Petri net converted to a CTMC |
| `deadlock_detection` | Detecting a deadlock in seize–delay–release |

## Notes

- The `core` feature is exposed as the `kernel` module. The Cargo feature keeps the name `core`.
- Each member crate has its own changelog. The workspace changelog is in [CHANGELOG.md](../../CHANGELOG.md).

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for the umbrella's release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
