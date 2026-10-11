# Changelog

All notable changes to the `tpt-des` workspace are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions adhere to [Semantic Versioning](https://semver.org).

## [Unreleased]

Correctness fixes from the 2026-10-11 review (todo.md Phase 6). Each fix has
a regression test. **Several changes are breaking** and are listed first.

### Documentation

- Every crate has its own `README.md` (usage, module map, feature flags, limits)
  and `CHANGELOG.md`, which is split from this file.
- The README examples of the library crates are compiled and tested as doctests.
- `CONTRIBUTING.md` says contributions are taken as issues only.

### Pre-release batch (typed time, new crates, export formats)

Breaking:
- **Typed time.** Times and durations in public APIs are `Time<Seconds>`. Rates
  and dimensionless values stay `f64`, documented as "per time unit".
  - `tpt-des-core`: `EventCalendar::pop_until` and `next_time` take or return
    `Time<Unit>`. `Popped` is now `Popped<Unit, E>` with a `Time<Unit>` `time`.
    `Time` gains `AddAssign`, `Div<f64>`, `Sum`, `Display`, and (feature `serde`)
    `Serialize`.
  - `tpt-des-queue`: `run_queue`, `run_queue_multi`, `run_queue_traced`,
    `SimQueue::with_warmup`, `with_servers`, and `finalize` take `Time<Seconds>`.
    `RateSampler::draw` and `FromClosure` return `Time<Seconds>`.
    `QueueMetrics::{total_wait, total_sojourn, last_t, reset_at}`,
    `ClassWait::total_wait`, `JobRecord` times, and `mean_wait`, `mean_sojourn`,
    `mean_wait_for_priority`, `JobRecord::wait` and `sojourn` return or take
    `Time<Seconds>`. `size_for_mean_wait`, `validate_mmc`, and
    `mean_wait_to_precision` take typed horizons, warm-ups, and targets.
  - `tpt-des-process`: `Sim::now`, `Sim::set_time`, `Sim::delay`,
    `ProcessSystem::now`, `set_time`, and `run_until` are typed. Bridge types
    use `Time<Seconds>` too. `ContinuousModel::integrate(dt)` takes
    `Time<Seconds>`, so implementers must change.
  - `tpt-des-petri`: `StochasticPetriNet::simulate(horizon)`, `SpnRun::horizon`,
    and `SpnRun::occupancy` durations are `Time<Seconds>`.
  - `tpt-des-resource`: `Buffer::pour_duration` and `drain_duration` return
    `Option<Time<Seconds>>`, and return `None` when the result overflows.
  - `tpt-des-stats`: `TimeWeightedAvg::push(t, v)` and `window()` use
    `Time<Seconds>`. `tpt-des-core` is now a normal dependency of this crate.
- A non-finite time cannot be built as a `Time`, so it panics at construction.
  Earlier, NaN horizons returned errors (or panicked with a different message).
  `SpnError::InvalidHorizon` now covers negative horizons only.
- **`#[non_exhaustive]`** on the error enums (`QueueError`, `ReachabilityError`,
  `SpnError`, `MarkovError`, `StatsError`, `ResourceError`) and on the result
  and report structs in `tpt-des-queue` (`QueueMetrics`, `ClassWait`,
  `JobRecord`, `Mm1Result`, `MmcResult`, `Mg1Result`, `MvaResult`,
  `MultiClassResult`, `Sizing`, `Estimate`, `ValidationRow`,
  `ValidationReport`, `PrecisionRun`). Matches on these need a wildcard arm.
- `tpt_des::core` was renamed to `tpt_des::kernel` in the earlier batch; the
  umbrella's `parallel` feature is new.
- The WebAssembly seed argument is `u32`, so JavaScript passes a plain number.

Added:
- `tpt-des-queue`: `parallel` feature (rayon). `validate_mmc` runs its
  replications on a pool. Results are identical to the serial run, because each
  replication keeps its seed and the order is preserved. `mean_wait_to_precision`
  stays serial, because its stopping rule depends on the order of replications.
- `tpt-des-queue::trace`: `trace_to_json`, `trace_to_chrome` (Chrome Trace Event
  format, with one lane per concurrent service), and `trace_fingerprint` (a
  64-bit replay check over the seed and the records).
- `crates/tpt-des-cli` (unpublished): binary `tpt-des` with `run`, `validate`,
  `size`, and `network` commands, driven by TOML files. Unknown keys are errors.
- `crates/tpt-des-wasm` (unpublished): WebAssembly bindings (`run_queue_json`,
  `validate_mmc_json`, `size_json`) and a static playground in
  `playground/`. The bindings were exercised in Node, and the page was not run
  in a browser.
- CI: `cargo build` for the WebAssembly crate, and the feature matrix includes
  `std queue parallel`.
- README badges for CI, MSRV, and license.

### Review follow-ups, Phases 7 to 10 (this batch)

Added:
- `tpt-des-queue`: multi-server simulation (`run_queue_multi`, plus
  `run_queue_traced` for per-job records and `trace_to_csv`).
- `tpt-des-queue`: exact multi-class closed-network MVA (`MultiClassClosed`,
  `ClassSpec`, `MultiClassResult`). Visit ratios come from each class's routing
  matrix.
- `tpt-des-queue::planning`: `size_for_mean_wait` (fewest servers for a
  mean-wait target), `validate_mmc` (simulation against M/M/c, with an
  assertable `ValidationReport`), and `mean_wait_to_precision` (replications
  until a confidence half-width target is met).
- `tpt-des-queue` `serde` feature: `Serialize` on result types (not configs,
  which hold boxed samplers).
- CI: `feature-matrix` job builds each umbrella feature combination on its own.

Changed:
- Priority dispatch uses a binary heap (O(log n) per dispatch, was a scan of the
  waiting line). Order is unchanged: priority, then arrival.
- Processor sharing keeps jobs in a heap keyed on virtual finish time, so each
  event is O(log n) (was O(n)). Completion times are unchanged.
- Resource waiters are kept per resource in grant order. Granting is one pass,
  and `cancel` is O(log n) through an index. Grants are unchanged.
- The event calendar uses a slab with generation-tagged `EventId`s. Schedule and
  cancel are O(1) (cancel was O(log n)), and stale handles are rejected. The
  two calendar benchmarks run about 2.0 to 2.6 times faster.
- `expm` uses 16 Taylor terms by Horner's rule (was 24 by repeated products).
  On the `expm` bench it is about 1.3 to 1.45 times faster.
- Student-t quantile bisection stops when its bracket is narrow (about 45
  iterations, was 200). Results agree with the table values in the tests.
- Replication-based M/M/1 tests: means over five replications, 10% tolerance
  (was single runs at 25 to 30%).
- `Dtmc::as_dmatrix` reads the chain directly, without the intermediate copy.

Breaking:
- `tpt_des::core` is now `tpt_des::kernel`. The Cargo feature is still `core`.
- `tpt-des-queue`: `QueueError::StateSpaceTooLarge` added.
- `tpt-des-petri`: `ReachabilityError::TokenOverflow` and `SpnError::TokenOverflow`
  added. Overflow used to panic inside the search and now returns an error.
- `tpt-des-queue`: `QueueMetrics` gains a public `servers` field.

Fixed:
- Processor-sharing `utilization` returned the mean number in system (it summed
  the job count into busy time). It is now the busy fraction, so it is near ρ.
  Regression test `processor_sharing_utilization_is_busy_fraction_not_population`
  fails on the old code.
- `process_bank_teller` imports `tpt_des::queue` but did not declare the feature.
  It now does, and CI passes it.
- The umbrella `std` feature also enables `tpt-des-process/std`.

### Added
- **Examples** in `crates/tpt-des/examples/`: `process_bank_teller` (checked
  against Erlang C), `stats_replications` (confidence interval over
  replications), `hybrid_bridge`, `call_center_erlang` (Erlang B and C sizing),
  `inventory_store` (a `Store` with a lead time), and `jackson_network`
  (traffic equations and MVA).
- **Templates** in `templates/` (cargo-generate layout): `single-queue-sim`,
  `process-sim`, and `petri-model`. They build against a path dependency. Until
  `tpt-des` is published, replace the `version = "0.1"` dependency with a path.
- **Docs** in `docs/`: getting started, choosing a crate, validating a model,
  FAQ, and a spec-versus-implementation status table. Crate READMEs are
  written, and their code blocks compile as doctests.
- `CONTRIBUTING.md` and `SECURITY.md`.

### Changed
- `moving_average` and MSER are linear in the series length (prefix sums and
  backward Welford accumulation). The quantile-interval search computes the
  binomial CDF once. On a 200k series, `moving_average` went from 4.4 ms to
  1.2 ms; MSER on 20k points from 133 ms to 0.15 ms (release build).
- The Markov first-passage reachability check is one reverse BFS, not one
  forward search per state.
- `[profile.test] opt-level = 1`: the queue's `mm1_vs_analytic` suite runs in
  about 6 s instead of 65 s in debug builds, with overflow checks still on.
- CI runs the test suite in debug and release on Linux, Windows, and macOS,
  and adds jobs for rustdoc warnings, MSRV 1.84, and wasm32.

### Breaking
- **tpt-des-stats**: estimators now return `Result<_, StatsError>` instead of
  panicking or returning degenerate values: `ci::mean_of_samples_ci`,
  `ci::mean_known_sigma_ci`, `ci::proportion_wilson`,
  `ci::quantile_ci_sorted`, `batch::batch_means`, `batch::replication_deletion`,
  `batch::regenerative`, and `timeseries::TimeWeightedAvg::push`. A single
  observation no longer yields a zero-width interval.
- **tpt-des-resource**: `PreemptPolicy::Fifo` is renamed to
  `PreemptPolicy::PreemptivePriority`; the old name described queueing order,
  not preemption. `seize` rejects requests larger than the pool's capacity
  with `ResourceError::InvalidParameter`.
- **tpt-des-queue**: `QueueConfig` has a new field `priority:
  Option<PriorityFn>`; set it to `None` for the previous behaviour. `run_queue`
  panics unless `0 <= warmup < horizon` and `horizon` is finite.
- **tpt-des-markov**: `Dtmc::stationary` is a direct linear solve, not power
  iteration; `tol` is now a residual bound and `max_iter` is ignored. Reducible
  chains return `MarkovError::Singular`. New `MarkovError` variants:
  `Singular`, `InvalidTolerance`.
- **tpt-des-petri**: `SpnError` gains `InvalidHorizon`, and
  `StochasticPetriNet::to_ctmc` refuses more than `MAX_CTMC_STATES` (4096)
  states. `place_invariants` returns non-negative minimal invariants (Farkas)
  instead of an arbitrary null-space basis.
- **tpt-des-core**: `Time::from_raw` and `SimClock::advance_to` check their
  inputs in all build profiles. `Simulation::run_until` advances the clock to a
  finite horizon. `SimRng::core_mut` counts as one draw. `RunStats` gains
  `dropped_events`.
- **tpt-des-process**: `Sim::delay` panics on a negative or NaN duration, and
  `Sim::acquire` panics when the request exceeds capacity. `HybridBridge::new`
  and `step` validate their inputs.

### Fixed
- **tpt-des-process**: the executor advanced the clock to the next timer before
  running processes that were already runnable at the current time. A process
  spawned at t = 0.5 therefore started at t = 1.0, and waits measured from its
  arrival came out too short (the new `process_bank_teller` example reported
  zero waits). It now runs runnable processes first and fires one timer at a
  time. Test `spawned_process_runs_before_clock_advances`.
- **tpt-des-process**: a contended `acquire` never completed after `release`
  (the woken waiter was dequeued before it could take its units). `release`
  clamps at capacity. `notify_all` no longer panics when a predicate reads
  simulation state. Finished processes are dropped. Self-interruption works.
- **tpt-des-resource**: `PreemptivePriority` could double-grant units in
  release builds (the take lived inside `debug_assert!`). An entity with two
  queued requests lost a wait-for edge, so deadlocks could go undetected. A
  new request could jump ahead of queued waiters (starvation). `held_fraction`
  returned the inverse. `Store::put` could overflow near `i64::MAX`. `Buffer`
  accepted NaN.
- **tpt-des-markov**: `matrix_exponential` scaled by the largest entry, not the
  induced norm. A NaN tolerance disabled row validation. Absorbing-state
  detection used exact float equality.
- **tpt-des-queue**: `Discipline::Priority` was FIFO because every job had
  priority 0. The warm-up cutoff discarded population time between events.
  `time_avg_number` and `utilization` now integrate to the horizon. `mmc`
  overflowed for `c` above about 170 (Erlang C now in log space).
  `erlang_c(0, c)` returned an error; it returns 0. `mm1_pn` wrapped for
  `n > i32::MAX`. Jackson networks rejected zero-rate nodes whose rates were a
  rounding residue below zero.
- **tpt-des-petri**: parallel input arcs from one place were checked one by
  one, so `fire` could underflow. Output overflow now refuses the firing. An
  infinite simulation horizon never ended.
- **tpt-des-stats**: `TimeWeightedAvg::window` measured from t = 0 rather than
  from the first push. Out-of-order pushes corrupted the average. The normal
  CDF tail was accurate only to about 1.5e-7 absolute.
- **tpt-des-verify**: two Kani harnesses checked local `u8` models rather than
  the real code; they now use `Simulation` and `PetriNet`. A deadlock-freedom
  proof for acyclic wait graphs was added. The calendar proptest now varies
  event priority.

## [0.1.0] — 2026-10-07

Initial release of the full phased build (spec.txt Phases 0–5).

### Added
- **tpt-des-core**: phantom-typed `Time<Unit>` (ticks/seconds/hours),
  from-scratch binary-heap event calendar with `O(log n)` scheduling,
  lazy tombstone cancellation, and compile-time `const` priorities;
  entity lifecycle traits (`on_create`/`on_event`/`on_terminate`);
  seeded `SplitMix64` simulation RNG with bit-exact deterministic replay;
  `no_std` + `alloc` compatibility (`thumbv6m-none-eabi` verified).
- **tpt-des-queue**: closed-form M/M/1, M/M/c, M/G/1 (Pollaczek–Khinchine),
  Erlang B (stable recursion) and Erlang C; open Jackson networks solved
  with `tpt-math-linalg-dense`; single-chain closed networks via exact
  mean value analysis; DES single-station queues with arbitrary
  inter-arrival/service distributions and FIFO/LIFO/priority/
  processor-sharing disciplines (exact work-conserving PS).
- **tpt-des-resource**: runtime and `const`-generic capacity pools,
  discrete stores, continuous fluid buffers with rate operations,
  seize-delay-release controller with FIFO/priority queueing, priority
  preemption, request cancellation (timeou
