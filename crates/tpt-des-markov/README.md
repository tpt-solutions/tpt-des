# tpt-des-markov

Discrete-time and continuous-time Markov chains: validated transition and generator matrices, steady-state solvers, transient distributions, absorbing-chain analysis, and first-passage times.

Use it to check a simulated Markov model against its exact answer, to solve a small chain directly, or to get the expected time to absorption (for example, to a failed state) without simulating.

## Features

- **Discrete-time chains.** `Dtmc` holds a row-stochastic matrix. It is built from rows (`from_rows`, which requires each row to sum to one) or from non-negative weights (`from_weights`, which normalises each row).
- **Continuous-time chains.** `Ctmc` holds a generator matrix. `transient` gives the distribution at time `t`, using a matrix exponential.
- **Steady state.** `Dtmc::stationary` and `Ctmc::stationary` solve the balance equations directly. They work for periodic chains, where power iteration does not converge.
- **Absorbing chains.** `absorb::analyze` gives the expected time to absorption and the probability of ending in each absorbing state.
- **First passage.** `fpt::mean_first_passage` gives the expected time to reach a target state from every state. `fpt::mean_recurrence_times` applies Kac's formula to a stationary distribution.
- **Matrix exponential.** `matrix_exponential` is exposed directly, with 16 Taylor terms evaluated by Horner's rule.

## Quick start

A weather chain, solved for its long-run distribution:

```rust
use tpt_des_markov::Dtmc;

// Sunny and rainy days.
let chain = Dtmc::from_rows(vec![vec![0.9, 0.1], vec![0.5, 0.5]]).unwrap();
let pi = chain.stationary(1e-12, 0).unwrap(); // max_iter is unused by the direct solve
assert!((pi[0] - 5.0 / 6.0).abs() < 1e-9);
```

The expected number of steps to reach state 0 from each state:

```rust
use tpt_des_markov::{fpt::mean_first_passage, Dtmc};

let chain = Dtmc::from_rows(vec![vec![0.5, 0.5], vec![0.25, 0.75]]).unwrap();
let steps = mean_first_passage(&chain, 0).unwrap();
assert_eq!(steps[0], 0.0);
assert!(steps[1] > 0.0);
```

## Module map

| Module | Main items |
|---|---|
| `dtmc` | `Dtmc`, `validate_row_stochastic`, `ROW_TOLERANCE` |
| `ctmc` | `Ctmc`, `matrix_exponential` |
| `absorb` | `analyze`, `AbsorbingAnalysis`, `is_absorbing` |
| `fpt` | `mean_first_passage`, `mean_recurrence_times` |
| `error` | `MarkovError` |

## Installation

```toml
[dependencies]
tpt-des-markov = "0.1"
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Enables `alloc` and `tpt-math-linalg-dense/std`. |
| `alloc` | no | Heap collections without `std`. |

## Notes and limits

- `Dtmc::stationary` solves the chain directly. Its `tol` argument bounds the residual, and `max_iter` is ignored.
- A reducible chain has no unique stationary distribution, so `stationary` returns `MarkovError::Singular`.
- `Ctmc::new` takes a tolerance for checking the generator's row sums.
- `MarkovError` is `#[non_exhaustive]`. Matches need a wildcard arm.

## Testing

```sh
cargo test -p tpt-des-markov
cargo bench -p tpt-des-markov   # criterion: matrix exponential
```

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
