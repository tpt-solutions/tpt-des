# tpt-des — Phased Build Tracker

Companion to `spec.txt`. License: dual MIT OR Apache-2.0. Author/org: TPT Solutions.

Status: **v0.1.0 complete** (Phases 0–5 done; publish/tag intentionally left
to the maintainer — see the Phase 5 notes at the bottom).

## Phase 0 — Foundation & Repo Setup

- [x] Workspace scaffold: root `Cargo.toml` with `[workspace]`, `resolver = "2"`, `[workspace.package]` (`version`, `edition = "2021"`, `authors = ["TPT Solutions"]`, `license = "MIT OR Apache-2.0"`, `repository`)
- [x] Create `crates/` layout per spec.txt section 3: `tpt-des-core`, `tpt-des-queue`, `tpt-des-resource`, `tpt-des-process`, `tpt-des-petri`, `tpt-des-markov`, `tpt-des-stats`, `tpt-des-verify`, `tpt-des` (umbrella)
- [x] Add `LICENSE-MIT` and `LICENSE-APACHE` (TPT Solutions boilerplate, mirror `tpt-math`/`tpt-control`); set `license.workspace = true` in every member crate
- [x] Add `deny.toml` modeled on `tpt-math`/`tpt-control` (permissive-license allowlist; deny unknown registry/git; deny Apache-2.0-only crates such as `nalgebra`, `clarabel`, `faer`, `statrs` per ecosystem precedent)
- [x] CI: `.github/workflows/ci.yml` (build + test + clippy + fmt + cargo-deny, plus a no-std job and a non-blocking Kani job)
- [x] `rustfmt.toml`, `rust-toolchain.toml` (pin version), `justfile` task runner (mirror `tpt-math`/`tpt-control` conventions)
- [x] Add `tpt-math` as a dependency; confirm which sub-crates are usable today: `tpt-math-prob-markov` (`MarkovChain`), `tpt-math-linalg-dense` (`DMatrix`/`DVector`), `tpt-math-prob-sampler`/`tpt-math-prob-dist` (samplers/distributions), `tpt-math-graph` (`dijkstra`/`astar`/`max_flow`)
  - **Resolution:** the `tpt-math` crates are published on crates.io
    (`0.1.x`), and the sibling `tpt-systems-optimisation` consumes them as
    **registry version dependencies** — `tpt-des` follows that convention
    (`[workspace.dependencies]` pins `0.1.0`, `default-features = false`
    where the member supports it) instead of fragile sibling path deps.
    Note: `tpt-math-prob-markov` publishes with an empty feature set (always
    alloc), and `tpt-math-linalg-dense` requires its `std` feature for the
    generic `norm()` methods — both reflected in the member feature maps.
- [x] `tpt-des-core`: unit-safe simulation clock with phantom-typed time units (ticks, seconds, hours)
- [x] `tpt-des-core`: from-scratch binary-heap event calendar with O(log n) scheduling and cancellation (lazy tombstones; `O(1)` live-count bookkeeping; bounded `pop_until`/`next_time` peeking)
- [x] `tpt-des-core`: `Entity` trait with lifecycle hooks (`on_create`, `on_event`, `on_terminate`)
- [x] `tpt-des-core`: deterministic replay via seeded RNG for reproducible simulations (`SimRng` wraps `tpt-math-prob-core::SplitMix64`; bit-exact replay tested)
- [x] `tpt-des-core`: `no_std` + `alloc` compatibility (CI builds `--no-default-features` for `thumbv6m-none-eabi`)
- [x] `tpt-des-verify`: scaffold crate, set up `proptest` harness for basic event calendar operations
- [x] `tpt-des-verify`: first Kani harness proving the event calendar maintains heap ordering (three harnesses: pop order, cancellation soundness, live-count exactness — `cargo kani -p tpt-des-verify`)
- [x] Sanity check: `cargo build` succeeds across the full workspace

## Phase 1 — Queuing & Resources

- [x] `tpt-des-queue`: closed-form M/M/1, M/M/c, M/G/1 (Pollaczek-Khinchine), Erlang B/C loss formulas (Erlang B via the overflow-proof downward recursion; unit tests against textbook values and the direct factorial formula)
- [x] `tpt-des-queue`: network of queues — Jackson networks, open/closed queuing networks with routing matrices (traffic equations `(I−Pᵀ)λ = γ` via `DMatrix::solve`; exact single-chain MVA)
- [x] `tpt-des-queue`: discrete-event simulation of queues with arbitrary inter-arrival/service distributions, via `tpt-math-prob-sampler`'s `InverseCdfSampler`/`sample_categorical` and `tpt-math-prob-dist`'s `rand_distr` wrappers (arbitrary distributions additionally supported through `FromClosure`)
- [x] `tpt-des-queue`: FIFO, LIFO, priority, and processor-sharing disciplines (PS implemented exactly: remaining-work tracking, rescheduled completion events, FIFO-fair re-granting)
- [x] `tpt-des-resource`: resource pools with capacity constraints and preemption policies (runtime `ResourcePool`, `const`-generic `FinitePool<CAP>`, `PreemptPolicy::{None, Priority, Fifo}` with eviction reporting)
- [x] `tpt-des-resource`: Stores (discrete containers) and buffers (continuous fluid models) (`Store`, `Buffer` with pour/drain rate durations)
- [x] `tpt-des-resource`: seize-delay-release patterns with timeout and cancellation support (`ResourceController`: `seize`/`release`/`cancel`/`grant_ready`; timeouts = caller-scheduled timer + `cancel`, documented and exercised in tests)
- [x] **Checkpoint:** deadlock detection needs resource-allocation-graph cycle detection — confirm whether `tpt-math-graph` already exposes it, or whether a small upstream addition there is needed before implementing deadlock detection locally in `tpt-des-resource`
  - **Resolution:** `tpt-math-graph` does **not** expose cycle detection
    (only `toposort`, shortest paths, and max-flow), so the small detector
    lives locally: `tpt_des_resource::deadlock` provides iterative
    white/gray/black DFS over the wait-for graph with cycle reconstruction
    (`WaitForGraph::find_cycle`, generic `find_cycle_in_digraph`). No
    upstream change was needed.
- [x] `tpt-des-resource`: deadlock detection via resource allocation graph cycle detection (holdings/waiters auto-synced into the wait-for graph by the `ResourceController`)
- [x] Integration test: M/M/1 DES simulation matches Erlang analytical formula via `proptest` (`tests/mm1_vs_analytic.rs`: 32-case sweeps over the stable regime for Wq/W/L/ρ, M/M/1/K blocking vs Erlang B, PS insensitivity, discipline conservation law, deterministic replay equality)

## Phase 2 — Process-Oriented & Advanced Models

- [x] `tpt-des-process`: generator-based process definitions using Rust's async/await or custom yield primitives (`async` bodies spawned onto a lock-during-poll-free executor; safe `std::task::Wake` wakers — the crate is std-only because the workspace forbids `unsafe`)
- [x] `tpt-des-process`: blocking operations — `wait_for(resource)`, `delay(duration)`, `wait_until(condition)` (`Sim::acquire`/`release`, `Sim::delay`, `Sim::wait_until` + `notify_all`; condition registry, no polling loops)
- [x] `tpt-des-process`: process interruption, preemption, and resumption (SimPy-style: the pending operation resolves `Err(Interrupt)`; the coroutine recovers and continues or aborts; self-interruption supported)
- [x] **Checkpoint:** confirm `tpt-control`'s current API surface before wiring the hybrid discrete-continuous integration hook in `tpt-des-process` (spec.txt section 5)
  - **Resolution:** `tpt-control` **does not exist** in the workspace (no
    sibling repo), so the hook is defined locally: the `hybrid` module's
    `ContinuousModel` trait + `HybridBridge` (fixed-step integration,
    guard-crossing interrupts) form the seam; when `tpt-control` lands it
    implements the trait with no bridge changes. Documented in the crate
    docs.
- [x] `tpt-des-process`: integration hook for hybrid discrete-continuous simulation with `tpt-control` (see above; `ProcessSystem::set_time` keeps the discrete clock synchronized with continuous time)
- [x] `tpt-des-petri`: place/transition Petri nets with marking representation (weighted arcs, `is_enabled`/`fire`, incidence matrix)
- [x] `tpt-des-petri`: stochastic Petri nets (SPN) with exponentially distributed firing times (via `tpt-math-prob-dist`) (Gillespie direct method: competing exponentials, `tpt-math-prob-sampler::sample_categorical` for transition choice, occupancy measurement)
- [x] `tpt-des-petri`: reachability graph generation and invariant analysis (P-invariants, T-invariants) (BFS with state bound + deadlock marking detection; null-space elimination over the incidence matrix; conservation checks)
- [x] `tpt-des-petri`: conversion to continuous-time Markov chains for steady-state analysis (`StochasticPetriNet::to_ctmc` → `tpt-des-markov::Ctmc`; verified against the truncated-geometric M/M/1/K stationary distribution)
- [x] `tpt-des-markov`: transition matrix construction and validation (row-stochastic enforcement), built on `tpt-math-prob-markov`'s `MarkovChain`
- [x] `tpt-des-markov`: steady-state probability computation via power iteration or eigendecomposition, using `tpt-math-linalg-dense`'s `DMatrix` (power iteration with L1 renormalization; CTMC stationarity via the augmented `(Qᵀ|1)πᵀ = (0|1)` solve)
- [x] `tpt-des-markov`: absorbing Markov chains — expected time to absorption, absorption probabilities (canonical form, fundamental matrix `N = (I−Q)⁻¹` via `DMatrix::inverse`; gambler's-ruin exactness tested)
- [x] `tpt-des-markov`: first-passage time and mean recurrence time calculations (per-target absorbing solves with reachability pre-filtering; Kac's `1/π_i` recurrence times)
- [x] `tpt-des-verify`: Kani proofs for entity conservation in closed queuing networks (`closed_network_conserves_entities`)
- [x] `tpt-des-verify`: Kani proofs that Markov chain transition matrices maintain probability mass (rows sum to 1.0) (`dtmc_normalization_maintains_probability_mass`, through the real `Dtmc::from_weights` path; plus a bonus Petri token-conservation harness)

## Phase 3 — Statistics

- [x] `tpt-des-stats`: warm-up period detection (Welch's method, MSIR) (Welch cross-replication averaging + moving-average smoothing + sustained-band detection; MSER and batched MSER-5, exposed as `mser`/`mser_stride`/`msir` — "MSIR" in the spec is the same rule family)
- [x] `tpt-des-stats`: output analysis — batch means, replication/deletion, and regenerative methods (non-overlapping batch means with Student-t intervals; per-replication deletion; delta-method regenerative ratio estimator)
- [x] `tpt-des-stats`: confidence interval computation for means, quantiles, and proportions (Student-t intervals via a from-scratch incomplete beta (DLMF 8.17.5 continued fraction, Lentz) and Lanczos log-gamma; Wilson score proportions; order-statistic quantile CIs with exact binomial coverage)
- [x] `tpt-des-stats`: time-series analysis of queue lengths and resource utilization (`TimeWeightedAvg` exact integrals, Welford `RunningStats`, lag-k autocorrelation)
- [x] `tpt-des-verify`: `proptest` strategies for generating valid queuing networks, Petri net topologies, and transition matrices (`tpt_des_verify::strategies`: stable queue params, row-stochastic matrices, sub-stochastic routing (spectral radius < 1), acyclic edge lists, markings, schedule plans, cancellation masks)
- [x] End-to-end test: multi-class queuing network — simulate, then verify `tpt-des-stats` reproduces known analytical confidence bounds (`tpt-des-queue/tests/e2e_stats.rs`: replication/deletion and batch-means CIs cover the Erlang/Cromer W and ρ; Wilson interval covers Erlang B blocking; warm-up smoke test)

## Phase 4 — Umbrella Crate, Docs & Examples

- [x] `tpt-des` umbrella crate: feature-gated re-exports of all 8 sub-crates (default = std + everything except `verify`; no_std subset = `default-features = false, features = ["core"]`)
- [x] Top-level `README.md`; per-crate README/API docs; doctests for public API surface (every crate has a README; core/queue/markov/process/resource/petri/stats all carry runnable doc examples)
- [x] Worked example: M/M/1 queue simulation validated against Erlang formulas (`examples/mm1_vs_erlang.rs` — 0.01% error at ρ = 0.75)
- [x] Worked example: stochastic Petri net reachability analysis (`examples/spn_reachability.rs` — reachability, conservation, exact CTMC steady state vs Gillespie occupancy)
- [x] Worked example: Markov chain steady-state / absorption analysis (`examples/markov_absorption.rs` — gambler's ruin i(N−i)/i/N exactness, Kac recurrence times)
- [x] Worked example: resource contention with deadlock detection (`examples/deadlock_detection.rs` — cross-seize cycle, timeout cancellation, drain)
- [x] Benchmarks (`criterion`, matching `tpt-math`/`tpt-dsp` convention) for event-calendar scheduling and matrix-exponential hot paths (`tpt-des-core/benches/calendar_bench.rs`, `tpt-des-markov/benches/expm_bench.rs`)

## Phase 5 — Release v1.0

- [x] Full `cargo-deny` audit pass across all sub-crates (`just deny` — licenses/advisories/bans/sources clean on the resolved graph)
- [x] `CHANGELOG.md`; align versions across all workspace members (all members at `0.1.0` via `[workspace.package]`)
- [x] Confirm dual LICENSE files and `authors = ["TPT Solutions"]` consistent across every crate (both inherited from `[workspace.package]`; LICENSE-MIT/LICENSE-APACHE copied verbatim from `tpt-math`)
- [ ] Decide publish target: crates.io vs. internal/path-dependency-only — **maintainer decision; not touched in the 2026 batch (you asked for no publish)** (the ecosystem precedent is crates.io: `tpt-math` is published; the umbrella's dependency graph is publish-ready, but publishing is outward-facing and left for sign-off)
- [ ] Tag and publish v1.0.0 — **maintainer decision; not done (you asked for no publish)** (blocked on the publish-target choice; version currently reads `0.1.0` matching the ecosystem convention for first releases)

---

# Post-0.1.0 review follow-ups

Source: code review of 2026-10-11 (read-only audit; findings not yet reproduced
by running code unless marked **verified**). Fix order: Phase 6 → 7 → 8 → 9 → 10.
Every Phase 6 fix lands with a failing-first regression test (release mode where relevant).

## Phase 6 — Correctness fixes

- [x] `tpt-des-resource` `sdr.rs:241` **(verified)**: `debug_assert!(self.try_take(..))` skips the pool take in release builds, so preemption double-grants units — *fixed; regression test added*
  - **Done:** take moved out of `debug_assert!`; `preemption_take_is_not_skipped_in_release` fails on the old code under `--release`.
- [x] `tpt-des-core` `calendar.rs` `next_time` **(verified)**: tombstones dropped without removing ids from `cancelled`, so `live_len()` underflows/panics; rewrite `next_time`/`pop_until` with `peek` — *fixed; regression tests added*
  - **Done:** `next_time` discards tombstones and their ids; `pop_until` peeks before popping. Tests `next_time_after_cancel_keeps_live_len_consistent` and `pop_until_respects_horizon_without_losing_events`.
- [x] `tpt-des-process`: contended `Sim::acquire` never completes (`ops.rs:561-609` vs `system.rs:103-131`); add contention test
  - **Done:** `release` wakes the FIFO head without reserving units for it; the head takes them when polled. `crates/tpt-des-process/tests/regressions.rs`: 11 of 12 fail on the old code (the 12th guards a path the API can't reach, see below).
- [x] `tpt-des-process`: `notify_all` panics (RefCell double borrow) when a predicate reads `Sim` state (`system.rs:157-171`)
  - **Done:** the registry is taken out while predicates run, so they can read `Sim` state. Test `notify_all_predicate_can_read_sim_state`.
- [x] `tpt-des-process`: `WaitUntil` should evaluate its predicate at registration; drop empty conditions on interrupt; reject NaN/negative `Delay`; replace user-reachable `expect` at `system.rs:208` with a clear error; validate hybrid step (`hybrid.rs:776`); `release` must not create units beyond capacity; drop finished futures from `procs`
  - **Done:** predicate checked at registration; interrupts remove only the interrupted process and drop emptied conditions; `delay` panics on NaN/negative; a clear message replaces the bare `expect`; `release` clamps at capacity; `acquire` beyond capacity panics; finished futures are dropped; hybrid `new`/`step` validate inputs; self-interrupt works (`is_alive` covers the process being polled).
- [x] `tpt-des-markov`: `matrix_exponential` scales by max-abs entry instead of an induced norm (`ctmc.rs:351`) — use infinity norm
  - **Done:** scaling uses the infinity norm. Test `expm_scales_by_infinity_norm`.
- [x] `tpt-des-markov`: `Dtmc::stationary` fails on periodic/reducible chains (use GTH or a linear solve); wrong error variants (`ctmc.rs:332`, `fpt.rs:653`); exact `== 1.0` absorbing test (`absorb.rs:500`); validate NaN tolerance
  - **Done:** direct solve with the normalization row (periodic chains converge; reducible chains return `Singular`); `tol` bounds the residual; absorbing test uses `ROW_TOLERANCE`; NaN tolerances rejected.
- [x] `tpt-des-queue`: `Discipline::Priority` behaves as FIFO (priority hard-coded to 0, `sim.rs:410,419`); fix the test that hides it
  - **Done:** `QueueConfig::priority` draws a class per arrival; per-class waits in `QueueMetrics::waits_by_priority`. Test `priority_discipline_serves_urgent_class_first`. The single-class test now covers only Fifo/Lifo.
- [x] `tpt-des-queue`: validate warm-up/horizon (NaN, warm-up >= horizon); reject negative/NaN service samples in release; integrate the final span to the horizon (biased `time_avg_number`/`utilization`)
  - **Done:** `run_queue` validates horizon and warm-up; samples checked in all profiles; the population integral runs to the exact cutoff and to the horizon (`finalize`). Test `time_average_is_exact_over_warmup_window_and_closed_at_horizon` (exact 7.25/9.75).
- [x] `tpt-des-queue`: `mmc` overflows for c > ~170 (use Erlang-B recursion); `erlang_c(0, c)` inconsistency; `n as i32` in `mm1_pn`; `l < 0.0` spurious `NoTrafficSolution` (`network.rs:408`)
  - **Done:** Erlang C in log space; `erlang_c(0, c) = 0`; `mm1_pn` uses `powf`; Jackson rates clamped with a scale-relative tolerance. Tests in `crates/tpt-des-queue/tests/regressions.rs`.
- [x] `tpt-des-resource`: `held_fraction` returns the inverse (`pool.rs:54`); reject `amount > capacity`; `WaitForGraph` keeps one `waiting_for` per entity; `Buffer::pour(NaN)`; `Store` level overflow; clarify/rename `PreemptPolicy::Fifo`; new seizes bypass the wait queue (starvation)
  - **Done:** `held_fraction` fixed; `seize` rejects `amount > capacity`; `WaitForGraph` counts requests per (entity, resource); queued waiters are no longer bypassed; `PreemptPolicy::Fifo` renamed `PreemptivePriority`; `Buffer` refuses NaN; `Store::put` cannot overflow.
- [x] `tpt-des-petri`: duplicate pre-arcs underflow in `fire` (`net.rs:139`); unchecked `u32` add; `simulate(horizon = INF)` never ends; `to_ctmc` unbounded dense allocation; add `Display`/`Error` for `SpnError`/`ReachabilityError`; `place_invariants` is not Farkas (negative entries despite docs)
  - **Done:** enabling sums weights per place; `fire` uses `checked_add`; `simulate` rejects non-finite horizons (`InvalidHorizon`); `to_ctmc` bounded by `MAX_CTMC_STATES`; `Display`/`Error` for `SpnError` and `ReachabilityError`; `place_invariants` is Farkas (minimal, non-negative).
- [x] `tpt-des-stats`: `TimeWeightedAvg::window()` wrong when first push is not at t=0; out-of-order push corrupts state
  - **Done:** window measured from the first push; `push` returns `Result` (`OutOfOrder`, `NonFinite`); rejected pushes leave state unchanged.
- [x] `tpt-des-stats`: panicking inputs (`ln_gamma`, confidence outside [0,1], `trials == 0`, `successes > trials`) → return `Result`; `normal_cdf` tail accuracy (~1.5e-7); `n = 1` CI falsely `(mean, mean)`; `batch_means` returns 0.0 on short series
  - **Done:** CI estimators return `Result<_, StatsError>`; `batch_means` rejects too-short series; a single observation is `InsufficientData`; `ln_gamma` and the quantile functions return NaN outside their domain; `normal_cdf` uses erfc with a continued fraction in the tails.
- [x] `tpt-des-core`: promote `Time::from_raw`/`advance_to` `debug_assert`s to real checks; warn on events to unknown `EntityId`; `SimRng::core_mut()` bypasses `draws`; `run_until` does not advance clock to horizon
  - **Done:** `from_raw` and `advance_to` check in all profiles; events to unknown entities are counted in `RunStats::dropped_events` (no_std cannot print a warning); `core_mut` counts a draw; `run_until` advances the clock to a finite horizon.
- [x] `tpt-des-verify`: add missing `tpt-des-markov` dependency (Kani build fails); replace local-`u8` pseudo-proofs with harnesses on real `Simulation`/`PetriNet`; add the spec'd "acyclic resource graph ⇒ no deadlock" proof; fix `u8` overflow in assumes
  - **Done, not formally verified:** dependency added (with `std`); the two local-`u8` proofs now use `Simulation`/`PetriNet`; acyclic deadlock-freedom proofs added; `u8` overflow in the assumes fixed. Kani is not installed in the authoring environment, so the harness bodies were only compiled and run on random inputs through a temporary shim, not proved. Run `just kani` to verify.
- [x] Missing tests: contended acquire, `notify_all` reading state, interrupt of `Acquire`/`WaitUntil`, non-zero queue priorities, large-c `mmc`, duplicate Petri arcs, stats edge cases, calendar proptest that actually varies priority (`calendar_props.rs`), tighten the 25–30% simulation tolerances
  - **Done (2026 batch):** the M/M/1 checks average 5 independent replications, with a 10% band on the means (utilization 5%). The PS and FIFO/LIFO checks use the same replication helper. Test time rose from about 10 s to 45–75 s in debug, because each proptest case now runs five simulations. Reduce `ProptestConfig::with_cases` if that is too slow for CI.
  - **Partial:** every test in the list above is added (the calendar proptest now varies priority). Still open: tightening the 25–30% tolerances in `crates/tpt-des-queue/tests/mm1_vs_analytic.rs`. Doing it properly needs replication-based confidence intervals and longer runs, which would roughly double that file's ~50 s debug runtime, so it is left for a decision.

## Phase 7 — CI & build hygiene

- [x] CI: add `cargo test --workspace --release` job (catches `debug_assert`-only bugs)
  - **Done:** release run added as a second step in the `test` matrix. The release suite passes locally.
- [x] CI: run Kani for real (drop `--only-codegen`; keep non-blocking until stable)
  - **Done:** flag dropped from the `kani` job, still `continue-on-error`. Not run here: Kani is not installed in the authoring environment.
- [x] CI: MSRV (1.84) job, `cargo doc -D warnings`, wasm32 build, per-crate `--no-default-features` + umbrella feature-matrix builds, Windows/macOS test jobs
  - **Done (2026 batch):** the `feature-matrix` job builds 11 umbrella feature sets on their own. Running it locally found a real bug: `process_bank_teller` used `tpt_des::queue` without declaring the feature. Fixed. The non-core crates stay std-only and are documented as such, so their `--no-default-features` builds are not expected to pass.
  - **Added:** MSRV job (`1.84`), `doc` job with `RUSTDOCFLAGS=-D warnings` (passes locally with `--all-features`), wasm32 and thumbv6m builds of `tpt-des-core`, umbrella `--features core` build, and a Windows/macOS matrix for tests. Not run here: the MSRV toolchain and the matrix runners.
  - **Still open:** per-crate `--no-default-features` builds for the non-core crates (they need std, see the next item), and a fuller umbrella feature matrix.
- [x] Resolve `no_std`/`alloc` claims for non-core crates (petri/stats/markov use `vec!`/`Vec` without alloc imports; queue has no `no_std` attribute) — add CI coverage or document as std-only
  - **Done (documented as std-only):** only `tpt-des-core` is no_std-capable. `markov` and `petri` fail `--no-default-features` because `tpt-math-linalg-dense` needs `alloc`/`std`. The umbrella manifest and the CI `no_std` job state this. `tpt-des-core` builds for `thumbv6m-none-eabi` and `wasm32-unknown-unknown`.
- [x] Umbrella `tpt-des`: `std` feature must use `dep?/std` so it does not enable every sub-crate; include `process`; fix `pub use tpt_des_core as core` shadowing and feature-reduced doctests
  - **Done (2026 batch):** the core re-export is now `tpt_des::kernel`, a breaking rename listed in the changelog. The Cargo feature is still `core`. `std` also enables `process/std`.
  - **Done:** `std` uses `dep?/std`; doctests are gated on their features; examples are gated with `required-features`. `--features core` builds and tests pass.
  - **Still open:** the `core` re-export shadows the `core` crate name for users who glob-import the umbrella. Renaming it is a breaking change, so it is left for a decision.
- [x] Commit `Cargo.lock`; `criterion`/`proptest` with `default-features = false`; consider thin LTO; add `[profile.test]` tuning
  - **Checked (2026 batch):** `Cargo.lock` is tracked in git (`git ls-files`). `criterion` and `proptest` keep their defaults on purpose; see the earlier note.
  - **Done:** `Cargo.lock` removed from `.gitignore` (it is tracked with the next commit). `[profile.test] opt-level = 1` added: the queue's `mm1_vs_analytic` suite went from 65 s to 5.5 s with overflow checks still on.
  - **Not changed, on purpose:** `lto = "fat"` in release is a maintainer choice and was left alone; `proptest` already runs with its default `std` feature, so disabling defaults gains nothing.
- [~] Add `CONTRIBUTING.md`, `SECURITY.md`, CI/MSRV badges; tighten `deny.toml` (`wildcards`, explicit bans for `nalgebra`/`clarabel`/`faer`/`statrs`)
  - **Partial:** the CI badge is not in the README yet. It needs a run on GitHub to confirm the badge URL, so it is not added blind.
  - **Done:** `CONTRIBUTING.md` and `SECURITY.md` added. `deny.toml` denies wildcard versions and the four Apache-2.0-only crates; `cargo deny check` passes (bans, licenses, sources, advisories).
  - **Still open:** CI and MSRV badges in the README (wait for the CI jobs to run once).

## Phase 8 — Performance (benchmark before/after with criterion)

- [x] Calendar: replace the two `BTreeSet<u64>` (pending/cancelled) with per-entry flag or slab + generation ids (est. 2–4×)
  - **Done (2026 batch):** slab with generation-tagged `EventId`s; `cancel` is O(1). Criterion, two calendar benchmarks: 2.0–2.6× faster. The public API is unchanged. Stale-handle and slot-reuse tests added.
  - **Open:** needs a slab with generation ids, which changes the public `EventId` type. Not started; it deserves its own change with a criterion before/after run.
- [x] Queue: `VecDeque` for FIFO (kill `Vec::remove(0)`), `BinaryHeap` for Priority, precompute `Exponential` distribution, indexed PS
  - **Done (2026 batch):** priority uses a heap; PS uses a virtual-time heap. `Exponential` is not precomputed on purpose. `Exp::new` is one check and a reciprocal, so caching would save nothing measurable and would cost `Copy` and `PartialEq`. A PS regression test also fixed the utilization bug found along the way.
  - **Done:** FIFO/LIFO use `VecDeque` (`pop_front`/`pop_back`).
  - **Open:** Priority still scans the waiting line on each dispatch (O(n)); the heap is not built. `Exponential` is not precomputed; PS is not indexed.
- [x] Resource: per-resource wait queues instead of O(waiters²) `grant_ready`; O(1) `waiting_count`
  - **Done (2026 batch):** per-resource queues keyed by (priority, id), one pass in `grant_ready`, index-based `cancel`. `waiting_count` is a map length. Regression test covers order and cancel.
  - **Open:** not started. `waiting_count` is still a scan.
- [x] Petri: avoid double marking clones and per-call `Vec` in `reachability`; O(1) `occupy` lookup
  - **Done (2026 batch):** the search checks enabling once and fires without re-checking, and no `Vec` is built per state. One clone per new state remains, since the state is stored and indexed. Overflow now returns `TokenOverflow` rather than panicking (a change to the error enums, in the changelog).
  - **Partial:** occupancy is a `BTreeMap` (O(log k) per firing, down from a linear scan); the output is still a `Vec`, now sorted by marking. `reachability` allocations not reviewed.
- [x] Markov: no per-iteration clone in `stationary`; `Dtmc::rows()` copy; single reverse BFS in `can_reach` (O(n³) → O(n+m)); Padé/Horner `expm`
  - **Done (2026 batch):** `expm` uses Horner's rule with 16 terms (about 1.3–1.45× faster). `as_dmatrix` no longer copies twice. `Dtmc::rows()` still returns an owned `Vec` by API design. Padé was not done, because Horner with 16 terms reaches about 1e-25 after scaling. The reverse BFS was already done in an earlier pass.
  - **Done:** `stationary` is now a direct solve (no iteration, so no per-iteration clone). Reachability is one reverse BFS, O(n²) on the dense matrix (the item's O(n+m) assumed sparse storage). Test: the existing `mean_first_passage` suite.
  - **Open:** `Dtmc::rows()` still copies; `expm` is still the 24-term Taylor series, not Padé/Horner.
- [x] Stats: O(n) `mser_stride` (suffix sums), O(n) `moving_average`, O(n) `quantile_ci_sorted`, memoize common t critical values
  - **Done (2026 batch):** the t quantile's bisection stops at a 1e-12 bracket, about 45 steps instead of 200. A memo table was not added: it needs shared mutable state in a crate that is `no_std`-capable, and the early exit gets most of the gain.
  - **Done:** MSER uses backward Welford accumulation; `moving_average` uses prefix sums; `quantile_ci_sorted` computes the binomial CDF once. Measured in release on this machine (n = 200k unless noted): `moving_average` 4.4 ms → 1.2 ms; MSER stride 1 at n = 20k 133 ms → 0.15 ms; MSER stride 5 at n = 200k 533 ms → 0.39 ms. Equivalence tests: `moving_average_prefix_sums_match_direct_windows`, `mser_backward_welford_matches_direct_reference`.
  - **Open:** t critical values are not memoized.

## Phase 9 — Adoption: examples, templates, docs

- [x] Replace the 9 stub crate READMEs with real ones (install, 10-line example, feature flags)
  - **Done:** all nine READMEs have install, example, and feature sections. Their Rust blocks compile and run as doctests, checked by temporary hooks that were then removed.
- [x] Examples: `process_bank_teller` (async processes), `stats_replications` (CI + warm-up), `hybrid_bridge`, `call_center_erlang` (sizing), `inventory_store`, `jackson_network`
  - **Done:** all six are in `crates/tpt-des/examples/` with `required-features`, and each one checks its own result. Writing `process_bank_teller` exposed an executor timing bug, now fixed (see CHANGELOG, Fixed). `stats_replications` uses 50 replications: with 20, a 95% interval missed the analytic value in one fixed-seed run, which is expected for 5% of runs.
- [x] `templates/` (cargo-generate style): `single-queue-sim`, `process-sim`, `petri-model`
  - **Done:** three templates, each generated into a scratch directory with a path dependency and run. **Open:** the templates depend on `tpt-des = "0.1"`, which resolves only once the crate is published (see Phase 5); until then, substitute a path.
- [x] `docs/` guide: getting started, choosing a crate, validating a model, FAQ, spec-vs-implementation status table
  - **Done:** `docs/` has the five pages plus an index. The status table maps every spec item to Done, Partial, or Not implemented.
- [x] Reconcile claims: README "AI-native"/"differentiable" vs reality; `spec.txt` phases 0–3 vs this tracker's 4–5; spec "MIT" vs shipped MIT OR Apache-2.0; CHANGELOG "Phases 0–5"
  - **Done (2026 batch):** "AI-native" is gone from the crate descriptions and umbrella docs, as well as the README. `spec.txt`'s license line now says MIT OR Apache-2.0, and its differentiable and AI-native lines are marked cut for 0.1.x. The original design wording is kept, so the history stays visible.
  - **Done:** README wording now matches the code ("AI-native" removed; Kani and no_std claims stated precisely). `docs/status.md` records the license difference and the phase mapping.
- [~] Spec gaps to implement or explicitly cut: differentiable/gradient hooks (spec §5), `tpt-systems-optimisation` integration and "optimize server count" end-to-end test, multi-class routing-matrix network test
  - **Decided (2026 batch):** differentiable hooks are **cut** for 0.1.x (recorded in `spec.txt` and `docs/status.md`). The `tpt-systems-optimisation` integration and its server-count test are **deferred**, as instructed. The multi-class routing-matrix network is **built** (`MultiClassClosed`), with unit-level tests. The server-count test waits on the deferred integration.
  - **Open, needs a decision:** none of these is started. Each is either a feature to build or a scope cut. `docs/status.md` lists them as Not implemented. Building them is large; the multi-class closed network is the one that unblocks the server-count test.

## Phase 10 — Proposed new features (confirm each before building)

- [~] Auto-sizing / what-if API: find minimum servers meeting an SLA (analytic bound + simulation confirmation)
  - **Partial:** `planning::size_for_mean_wait` gives the fewest servers for a mean-wait target, analytically. Simulation confirmation is `validate_mmc` on the chosen count, called by the user, not automatic. The target is a mean-wait only, not a general SLA such as a percentile.
- [~] Automatic model-validation report: simulate vs analytic (M/M/c, Jackson) with CIs, usable as a `cargo test` helper
  - **Partial:** `planning::validate_mmc` covers M/M/c, with `assert_passes` for tests. It reports standard errors, not confidence intervals, and the Jackson network is not covered.
- [~] Run-until-precision driver: automatic MSER warm-up + replications until CI half-width < target
  - **Partial:** `planning::mean_wait_to_precision` adds replications until the Student-t half-width of the mean wait meets the target. The warm-up is the caller's; MSER detection is not wired in.
- [x] Deterministic replay + trace export (event log, seed fingerprint → JSON/CSV/Chrome-trace); rayon-parallel replications behind `std`
  - **Done (pre-release batch):** CSV, JSON, and Chrome-trace export; `trace_fingerprint` for replay checks. Rayon is an optional `parallel` feature (adds a new dependency), used by `validate_mmc`. `mean_wait_to_precision` stays serial, because its stopping rule depends on replication order. Results match the serial run exactly.
- [x] Typed time end-to-end: `Time<Unit>` in queue/process/petri/SPN APIs instead of raw `f64`
  - **Done (pre-release batch):** times and durations are `Time<Seconds>` in queue, process, petri, resource, and stats. Rates and analytic formulas stay `f64`. Breaking, and listed in the changelog. The calendar benchmark shows no measurable cost.
- [x] Optional `serde` for configs/results; WASM playground; `tpt-des` CLI running a TOML-described queue network
  - **Done (pre-release batch):** `serde` derives on result types (configs are not serializable: they hold boxed samplers). The CLI is `crates/tpt-des-cli` (unpublished): `run`, `validate`, `size`, and `network` on TOML files. The WASM bindings are `crates/tpt-des-wasm`. The bindings were tested in Node, and the playground page was not run in a browser.

---

# Pre-release checklist (before the first crates.io publish)

Not published, per instruction. The publish itself is still a maintainer decision.

- [x] `#[non_exhaustive]` on public error enums and result/report structs.
- [x] README badges added (CI, MSRV 1.84, license). The CI badge URL is unverified until GitHub runs CI.
- [x] `cargo publish --dry-run -p tpt-des-core` passes (with `--allow-dirty`).
- [ ] `cargo publish --dry-run` for the dependent crates. Blocked: they depend on
      `tpt-des-core`, which is not on crates.io yet. Publish core first, then the rest in
      dependency order (core, stats, markov, queue, resource, petri, process, verify, tpt-des).
      `tpt-des-markov` packages cleanly with `cargo package --no-verify`.
- [ ] GitHub CI run: Kani, MSRV 1.84, Windows and macOS, the feature matrix, the wasm build.
      None of these ran locally.
- [ ] Decide publish target and tag v1.0.0 (Phase 5 above).
- [x] `tpt-des-cli` and `tpt-des-wasm` are `publish = false` (decision: unpublished for 0.1.0).
