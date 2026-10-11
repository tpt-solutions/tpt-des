# tpt-des-stats

Output analysis for simulations: confidence intervals, batch means, replication and deletion, warm-up detection, and time-weighted statistics.

A simulation's raw output is correlated and starts from an unrepresentative state. This crate covers the steps that turn that output into an estimate with an honest interval: find where the warm-up ends, group the data so the batches are roughly independent, and report a confidence interval around the result.

## Features

- **Confidence intervals.** Student-t intervals for means (`ci::mean_of_samples_ci`), a known-sigma interval (`mean_known_sigma_ci`), Wilson score intervals for proportions (`proportion_wilson`), and order-statistic intervals for quantiles (`quantile_ci_sorted`).
- **Batch methods.** `batch::batch_means` splits one long run into batches. `batch::replication_deletion` discards a warm-up from each of several independent replications. `batch::regenerative` gives a ratio estimator for regenerative cycles.
- **Warm-up detection.** `warmup::welch` averages replications, `warmup::welch_warmup` finds where the smoothed series settles, and `warmup::mser` and `mser_stride` apply the MSER rule. `moving_average` smooths a series first.
- **Time-weighted statistics.** `TimeWeightedAvg` gives the exact average of a step function, such as a queue length or a utilisation.
- **Running statistics.** `RunningStats` computes mean and variance in one pass with Welford's method. `autocorrelation` gives the lag-k correlation of a series.
- **Distributions.** Student-t, normal, and incomplete-beta routines, implemented in this crate, with no external numerics dependency.

Estimators return `Result<_, StatsError>`. A single observation gives no interval, and the crate reports that as an error rather than returning a zero-width interval.

## Quick start

A 95% interval over five replication means:

```rust
use tpt_des_stats::ci::mean_of_samples_ci;

let replication_means = [3.1, 3.4, 2.9, 3.3, 3.2];
let (lo, hi) = mean_of_samples_ci(&replication_means, 0.95).unwrap();
assert!(lo < 3.2 && 3.2 < hi);
```

Exact time-average of a queue length that changes at irregular times:

```rust
use tpt_des_core::Time;
use tpt_des_stats::timeseries::TimeWeightedAvg;

let mut queue = TimeWeightedAvg::new();
queue.push(Time::from_raw(0.0), 0.0).unwrap(); // empty at t = 0
queue.push(Time::from_raw(2.0), 3.0).unwrap(); // three jobs from t = 2
queue.push(Time::from_raw(6.0), 1.0).unwrap(); // one job from t = 6
// Over the window [0, 6]: (0*2 + 3*4) / 6 = 2.
assert!((queue.average() - 2.0).abs() < 1e-12);
```

## Module map

| Module | Main items |
|---|---|
| `ci` | `mean_of_samples_ci`, `mean_known_sigma_ci`, `proportion_wilson`, `quantile_ci_sorted`, `student_t_critical` |
| `batch` | `batch_means`, `replication_deletion`, `regenerative`, and their result structs |
| `warmup` | `welch`, `welch_warmup`, `moving_average`, `mser`, `mser_stride`, `msir` |
| `timeseries` | `TimeWeightedAvg`, `RunningStats`, `autocorrelation` |
| `dist` | `student_t_cdf`, `student_t_ppf`, `normal_cdf`, `normal_ppf`, `beta_inc`, `ln_gamma` |
| `error` | `StatsError` |

## Installation

```toml
[dependencies]
tpt-des-stats = "0.1"
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Enables `alloc`. |
| `alloc` | no | Heap collections without `std`. |

## Notes and limits

- Batch means assume the batches are roughly independent. Choose a batch size large enough that the lag-1 autocorrelation of batch means is small. `autocorrelation` helps check this.
- Warm-up detection is a heuristic. Check it against a plot of the series, and against a longer run.
- `TimeWeightedAvg::push` needs times that never decrease. Out-of-order pushes are an error.
- `StatsError` is `#[non_exhaustive]`. Matches need a wildcard arm.

## Testing

```sh
cargo test -p tpt-des-stats
```

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
