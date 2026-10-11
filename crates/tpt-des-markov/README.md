# tpt-des-markov

Discrete- and continuous-time Markov chains: validated transition and generator matrices, steady state, absorbing-chain analysis, and first-passage times.

## What's here

- `Dtmc` (row-stochastic, validated; `from_rows`, `from_weights`) and `Ctmc` (generator matrices; `transient` via a matrix exponential).
- Steady state: `Dtmc::stationary` (direct solve, so periodic chains work) and `Ctmc::stationary`.
- `absorb::analyze`: expected time to absorption and absorption probabilities.
- `fpt::mean_first_passage` and Kac's `mean_recurrence_times`.

## Example

```rust
use tpt_des_markov::Dtmc;

// Weather: sunny and rainy days.
let chain = Dtmc::from_rows(vec![vec![0.9, 0.1], vec![0.5, 0.5]]).unwrap();
let pi = chain.stationary(1e-12, 0).unwrap(); // max_iter is unused by the direct solve
assert!((pi[0] - 5.0 / 6.0).abs() < 1e-9);
```

## Install

```toml
[dependencies]
tpt-des-markov = "0.1"
```

## Feature flags

- `std` (default): enables `alloc` and `tpt-math-linalg-dense/std`.
- `alloc`: heap collections, without `std`.

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
