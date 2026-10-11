# Changelog

All notable changes to `tpt-des-queue` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Workspace-wide notes are in the [workspace changelog](../../CHANGELOG.md).

## [Unreleased]

### Breaking

- Typed time. `run_queue`, `run_queue_multi`, `run_queue_traced`,
  `SimQueue::with_warmup`, `with_servers`, and `finalize` take `Time<Seconds>`.
  `RateSampler::draw` and `FromClosure` return `Time<Seconds>`.
- `QueueMetrics::{total_wait, total_sojourn, last_t, reset_at}`,
  `ClassWait::total_wait`, and `JobRecord` times are typed. `mean_wait`,
  `mean_sojourn`, `mean_wait_for_priority`, `JobRecord::wait`, and
  `JobRecord::sojourn` return or take `Time<Seconds>`.
- `size_for_mean_wait`, `validate_mmc`, and `mean_wait_to_precision` take typed
  horizons, warm-ups, and targets.
- `QueueConfig` has a new field, `priority: Option<PriorityFn>`. Set it to `None`
  for the previous behaviour.
- `run_queue` panics unless `0 <= warmup < horizon` and `horizon` is finite.
- `QueueError`, `QueueMetrics`, `ClassWait`, `JobRecord`, `Mm1Result`,
  `MmcResult`, `Mg1Result`, `MvaResult`, `MultiClassResult`, `Sizing`,
  `Estimate`, `ValidationRow`, `ValidationReport`, and `PrecisionRun` are
  `#[non_exhaustive]`. Matches on them need a wildcard arm.
- `QueueError::StateSpaceTooLarge` is added.
- `QueueMetrics` gains a public `servers` field.

### Added

- `parallel` feature (rayon). `validate_mmc` runs its replications on a pool.
  Results are identical to the serial run, because each replication keeps its
  seed and the order is preserved. `mean_wait_to_precision` stays serial.
- `run_queue_multi` simulates several parallel servers sharing one waiting line.
  `run_queue_traced` also returns a `JobRecord` for each completed job.
- `trace` module: `trace_to_json`, `trace_to_chrome` (Chrome Trace Event format,
  with one lane per concurrent service), and `trace_fingerprint` (a 64-bit replay
  check over the seed and the records). `trace_to_csv` writes the records as CSV.
- Exact multi-class closed-network MVA: `MultiClassClosed`, `ClassSpec`, and
  `MultiClassResult`. Visit ratios come from each class's routing matrix.
- `planning` module: `size_for_mean_wait` (fewest servers for a mean-wait
  target), `validate_mmc` (simulation against M/M/c, with an assertable
  `ValidationReport`), and `mean_wait_to_precision` (replications until a
  confidence half-width target is met).
- `serde` feature: `Serialize` on result types. Configs are not serialized,
  because they hold boxed samplers.

### Changed

- Priority dispatch uses a binary heap, so each dispatch is O(log n) (it was a
  scan of the waiting line). The order is unchanged: priority, then arrival.
- Processor sharing keeps jobs in a heap keyed on virtual finish time, so each
  event is O(log n) (was O(n)). Completion times are unchanged.

### Fixed

- `Discipline::Priority` behaved as FIFO, because every job had priority 0.
- The warm-up cutoff discarded population time between events. `time_avg_number`
  and `utilization` now integrate to the horizon.
- Processor-sharing `utilization` returned the mean number in system. It is now
  the busy fraction, so it is near ρ.
- `mmc` overflowed for `c` above about 170. Erlang C now works in log space.
- `erlang_c(0, c)` returned an error. It returns 0.
- `mm1_pn` wrapped for `n > i32::MAX`.
- Jackson networks rejected zero-rate nodes whose rates were a rounding residue
  below zero.

## [0.1.0] — 2026-10-07

### Added

- Closed-form M/M/1, M/M/c, and M/G/1 (Pollaczek–Khinchine) results.
- Erlang B (stable recursion) and Erlang C.
- Open Jackson networks, solved with `tpt-math-linalg-dense`.
- Single-chain closed networks, solved by exact mean value analysis.
- Discrete-event single-station queues with arbitrary inter-arrival and service
  distributions, and FIFO, LIFO, priority, and processor-sharing disciplines
  (exact, work-conserving processor sharing).
