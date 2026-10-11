# Changelog

All notable changes to the `tpt-des` workspace are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions adhere to [Semantic Versioning](https://semver.org).

## [Unreleased]

Correctness fixes from the 2026-10-11 review (todo.md Phase 6). Each fix has
a regression test. **Several changes are breaking** and are listed first.

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
