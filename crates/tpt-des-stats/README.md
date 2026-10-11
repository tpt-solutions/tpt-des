# tpt-des-stats

Output analysis: confidence intervals, batch means, replication/deletion, warm-up detection, and time-weighted statistics.

## What's here

- Intervals: Student-t for means (`ci::mean_of_samples_ci`), Wilson score for proportions, and order-statistic intervals for quantiles.
- Batch means, replication/deletion, and regenerative ratio estimators (`batch`).
- Warm-up: Welch averaging, moving averages, MSER and MSER-5 (`warmup`).
- `TimeWeightedAvg` for exact step-function averages (queue lengths, utilization), `RunningStats` (Welford), and autocorrelation.

Estimators return `Result<_, StatsError>`. A single observation has no interval, for example, and that is reported as an error rather than a zero-width interval.

## Example

```rust
use tpt_des_stats::ci::mean_of_samples_ci;

let replication_means = [3.1, 3.4, 2.9, 3.3, 3.2];
let (lo, hi) = mean_of_samples_ci(&replication_means, 0.95).unwrap();
assert!(lo < 3.2 && 3.2 < hi);
```

## Install

```toml
[dependencies]
tpt-des-stats = "0.1"
```

## Feature flags

- `std` (default): enables `alloc`.
- `alloc`: heap collections, without `std`.

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
