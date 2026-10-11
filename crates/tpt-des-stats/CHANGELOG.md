# Changelog

All notable changes to `tpt-des-stats` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Workspace-wide notes are in the [workspace changelog](../../CHANGELOG.md).

## [Unreleased]

### Breaking

- Estimators return `Result<_, StatsError>` instead of panicking or returning
  degenerate values: `ci::mean_of_samples_ci`, `ci::mean_known_sigma_ci`,
  `ci::proportion_wilson`, `ci::quantile_ci_sorted`, `batch::batch_means`,
  `batch::replication_deletion`, `batch::regenerative`, and
  `TimeWeightedAvg::push`. A single observation no longer yields a zero-width
  interval.
- Typed time. `TimeWeightedAvg::push(t, v)` and `TimeWeightedAvg::window()` use
  `Time<Seconds>`.
- `tpt-des-core` is now a normal dependency of this crate.
- `StatsError` is `#[non_exhaustive]`. Matches need a wildcard arm.

### Changed

- `moving_average` and MSER run in linear time in the series length (prefix sums
  and backward Welford accumulation). On a 20k-point series, MSER went from 133 ms
  to 0.15 ms in a release build.
- The quantile-interval search computes the binomial CDF once.
- Student-t quantile bisection stops when its bracket is narrow (about 45
  iterations, was 200). Results agree with the table values in the tests.

### Fixed

- `TimeWeightedAvg::window` measured from t = 0, not from the first push.
- Out-of-order pushes corrupted the time-weighted average.
- The normal CDF tail was accurate only to about 1.5e-7 absolute.

## [0.1.0] — 2026-10-07

### Added

- Warm-up detection (Welch, moving average, MSER, MSER-5).
- Batch means, replication/deletion, and regenerative estimators.
- Student-t, Wilson, and order-statistic confidence intervals.
- Time-weighted and running statistics, and autocorrelation.
