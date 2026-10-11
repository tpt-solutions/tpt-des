# tpt-des-verify

Verification tools for the `tpt-des` workspace: proptest strategies, property tests, and Kani bounded model checks. This crate is for tests and proofs. Simulation code should not depend on it.

## Features

- **Strategies.** `strategies` provides proptest generators for the values the workspace's properties need: stable queue parameters, row-stochastic and sub-stochastic matrices, acyclic edge lists, Petri markings, event schedules with priorities, and cancellation masks. `finite_time` generates finite, non-negative raw `f64` times.
- **Property tests.** The `tests/` directory checks the event calendar for ordering, cancellation, and live-count bookkeeping, with the priority varied per event.
- **Kani harnesses.** `src/proofs.rs` is compiled only under `cfg(kani)`. It holds bounded model checks of the event calendar, closed-network entity conservation, Markov probability mass, Petri token conservation, and deadlock freedom for acyclic wait graphs. The harnesses run on the real `Simulation`, `PetriNet`, and resource code, not on local models of them.

## Running

```sh
cargo test -p tpt-des-verify            # property tests
cargo kani -p tpt-des-verify            # bounded model checks (needs cargo-kani)
```

Kani is not part of the default build. Install it with `cargo install --locked kani-verifier` and then run `cargo kani setup`. The workspace's `just kani` recipe runs the same check. In CI it is non-blocking.

## Using the strategies

Add the crate as a dev-dependency and use the strategies in your own proptest suites:

```rust
use proptest::prelude::*;
use tpt_des_verify::strategies::stable_queue_params;

proptest! {
    #[test]
    fn utilisation_is_below_one((lambda, mu) in stable_queue_params()) {
        prop_assert!(lambda < mu);
    }
}
```

## Installation

```toml
[dev-dependencies]
tpt-des-verify = "0.1"
```

## Feature flags

None. Proptest and the Kani harnesses are always built for this crate.

## Notes and limits

- Property tests check the statistical and structural laws with random inputs. They do not prove anything for all inputs.
- Kani checks are bounded. They prove the property for schedules and nets up to the bound in each harness, not for all sizes.

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
