# tpt-des-verify

Verification tools for the workspace. This crate is for tests and proofs, not for simulation code.

## What's here

- **proptest strategies** (`strategies`): stable queue parameters, row-stochastic and sub-stochastic matrices, acyclic edge lists, markings, schedule plans, and cancellation masks.
- **Property tests** in `tests/`: calendar ordering, cancellation, and live-count bookkeeping, with the priority varied per event.
- **Kani harnesses** (`src/proofs.rs`, compiled only under `cfg(kani)`): bounded model checks of the calendar, closed-network entity conservation, Markov normalization, Petri token conservation, and deadlock freedom for acyclic wait graphs.

## Running

```sh
cargo test -p tpt-des-verify          # property tests
cargo kani -p tpt-des-verify          # bounded model checks (needs cargo-kani)
```

## Install

```toml
[dev-dependencies]
tpt-des-verify = "0.1"
```

## Feature flags

None. Proptest and the Kani harnesses are always built for this crate.

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
