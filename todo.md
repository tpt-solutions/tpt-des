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
- [ ] Decide publish target: crates.io vs. internal/path-dependency-only — **maintainer decision** (the ecosystem precedent is crates.io: `tpt-math` is published; the umbrella's dependency graph is publish-ready, but publishing is outward-facing and left for sign-off)
- [ ] Tag and publish v1.0.0 — **maintainer decision** (blocked on the publish-target choice; version currently reads `0.1.0` matching the ecosystem convention for first releases)
