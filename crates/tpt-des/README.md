# tpt-des

The umbrella crate for the workspace. It re-exports the member crates behind feature flags, so one dependency covers what you need.

## Install

```toml
[dependencies]
tpt-des = { version = "0.1", features = ["queue", "std"] }
```

## Example

```rust
use tpt_des::queue::{erlang_c, mmc};

// Ten agents, 6 erlangs of traffic: the probability that a call waits.
let p_wait = erlang_c(6.0, 10).unwrap();
let analytic = mmc(6.0, 1.0, 10).unwrap();
assert!((p_wait - analytic.erlang_c).abs() < 1e-12);
```

## Feature flags

| Feature | Enables | Notes |
|---|---|---|
| `std` (default) | `std` in each enabled member | Required by every feature except `core`. |
| `core` | `tpt-des-core` | The only no_std-capable member. |
| `queue` | `tpt-des-queue` | Needs `std`. |
| `resource` | `tpt-des-resource` | |
| `process` | `tpt-des-process` | Needs `std`. |
| `petri` | `tpt-des-petri` (and `markov`) | |
| `markov` | `tpt-des-markov` | |
| `stats` | `tpt-des-stats` | |
| `verify` | `tpt-des-verify` and the members it tests | Implies `std`. |

Examples live in `examples/` and declare the features they need. Run one with, for example, `cargo run -p tpt-des --example mm1_vs_erlang --features "std queue"`.

## Minimal no_std use

```toml
tpt-des = { version = "0.1", default-features = false, features = ["core"] }
```

Only the `core` feature works without `std`.

---

Licensed under MIT OR Apache-2.0. See [the workspace README](../../README.md).
