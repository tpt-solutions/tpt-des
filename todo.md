# tpt-des — Phased Build Tracker

Companion to `spec.txt`. License: dual MIT OR Apache-2.0. Author/org: TPT Solutions.

## Phase 0 — Foundation & Repo Setup

- [ ] Workspace scaffold: root `Cargo.toml` with `[workspace]`, `resolver = "2"`, `[workspace.package]` (`version`, `edition = "2021"`, `authors = ["TPT Solutions"]`, `license = "MIT OR Apache-2.0"`, `repository`)
- [ ] Create `crates/` layout per spec.txt section 3: `tpt-des-core`, `tpt-des-queue`, `tpt-des-resource`, `tpt-des-process`, `tpt-des-petri`, `tpt-des-markov`, `tpt-des-stats`, `tpt-des-verify`, `tpt-des` (umbrella)
- [ ] Add `LICENSE-MIT` and `LICENSE-APACHE` (TPT Solutions boilerplate, mirror `tpt-math`/`tpt-control`); set `license.workspace = true` in every member crate
- [ ] Add `deny.toml` modeled on `tpt-math`/`tpt-control` (permissive-license allowlist; deny unknown registry/git; deny Apache-2.0-only crates such as `nalgebra`, `clarabel`, `faer`, `statrs` per ecosystem precedent)
- [ ] CI: `.github/workflows/ci.yml` (build + test + clippy + fmt + cargo-deny)
- [ ] `rustfmt.toml`, `rust-toolchain.toml` (pin version), `justfile` task runner (mirror `tpt-math`/`tpt-control` conventions)
- [ ] Add `tpt-math` as a path dependency; confirm which sub-crates are usable today: `tpt-math-prob-markov` (`MarkovChain`), `tpt-math-linalg-dense` (`DMatrix`/`DVector`), `tpt-math-prob-sampler`/`tpt-math-prob-dist` (samplers/distributions), `tpt-math-graph` (`dijkstra`/`astar`/`max_flow`)
- [ ] `tpt-des-core`: unit-safe simulation clock with phantom-typed time units (ticks, seconds, hours)
- [ ] `tpt-des-core`: from-scratch binary-heap event calendar with O(log n) scheduling and cancellation
- [ ] `tpt-des-core`: `Entity` trait with lifecycle hooks (`on_create`, `on_event`, `on_terminate`)
- [ ] `tpt-des-core`: deterministic replay via seeded RNG for reproducible simulations
- [ ] `tpt-des-core`: `no_std` + `alloc` compatibility
- [ ] `tpt-des-verify`: scaffold crate, set up `proptest` harness for basic event calendar operations
- [ ] `tpt-des-verify`: first Kani harness proving the event calendar maintains heap ordering
- [ ] Sanity check: `cargo build` succeeds across the full workspace

## Phase 1 — Queuing & Resources

- [ ] `tpt-des-queue`: closed-form M/M/1, M/M/c, M/G/1 (Pollaczek-Khinchine), Erlang B/C loss formulas
- [ ] `tpt-des-queue`: network of queues — Jackson networks, open/closed queuing networks with routing matrices
- [ ] `tpt-des-queue`: discrete-event simulation of queues with arbitrary inter-arrival/service distributions, via `tpt-math-prob-sampler`'s `InverseCdfSampler`/`sample_categorical` and `tpt-math-prob-dist`'s `rand_distr` wrappers
- [ ] `tpt-des-queue`: FIFO, LIFO, priority, and processor-sharing disciplines
- [ ] `tpt-des-resource`: resource pools with capacity constraints and preemption policies
- [ ] `tpt-des-resource`: Stores (discrete containers) and buffers (continuous fluid models)
- [ ] `tpt-des-resource`: seize-delay-release patterns with timeout and cancellation support
- [ ] **Checkpoint:** deadlock detection needs resource-allocation-graph cycle detection — confirm whether `tpt-math-graph` already exposes it, or whether a small upstream addition there is needed before implementing deadlock detection locally in `tpt-des-resource`
- [ ] `tpt-des-resource`: deadlock detection via resource allocation graph cycle detection
- [ ] Integration test: M/M/1 DES simulation matches Erlang analytical formula via `proptest`

## Phase 2 — Process-Oriented & Advanced Models

- [ ] `tpt-des-process`: generator-based process definitions using Rust's async/await or custom yield primitives
- [ ] `tpt-des-process`: blocking operations — `wait_for(resource)`, `delay(duration)`, `wait_until(condition)`
- [ ] `tpt-des-process`: process interruption, preemption, and resumption
- [ ] **Checkpoint:** confirm `tpt-control`'s current API surface before wiring the hybrid discrete-continuous integration hook in `tpt-des-process` (spec.txt section 5)
- [ ] `tpt-des-process`: integration hook for hybrid discrete-continuous simulation with `tpt-control`
- [ ] `tpt-des-petri`: place/transition Petri nets with marking representation
- [ ] `tpt-des-petri`: stochastic Petri nets (SPN) with exponentially distributed firing times (via `tpt-math-prob-dist`)
- [ ] `tpt-des-petri`: reachability graph generation and invariant analysis (P-invariants, T-invariants)
- [ ] `tpt-des-petri`: conversion to continuous-time Markov chains for steady-state analysis
- [ ] `tpt-des-markov`: transition matrix construction and validation (row-stochastic enforcement), built on `tpt-math-prob-markov`'s `MarkovChain`
- [ ] `tpt-des-markov`: steady-state probability computation via power iteration or eigendecomposition, using `tpt-math-linalg-dense`'s `DMatrix`
- [ ] `tpt-des-markov`: absorbing Markov chains — expected time to absorption, absorption probabilities
- [ ] `tpt-des-markov`: first-passage time and mean recurrence time calculations
- [ ] `tpt-des-verify`: Kani proofs for entity conservation in closed queuing networks
- [ ] `tpt-des-verify`: Kani proofs that Markov chain transition matrices maintain probability mass (rows sum to 1.0)

## Phase 3 — Statistics

- [ ] `tpt-des-stats`: warm-up period detection (Welch's method, MSIR)
- [ ] `tpt-des-stats`: output analysis — batch means, replication/deletion, and regenerative methods
- [ ] `tpt-des-stats`: confidence interval computation for means, quantiles, and proportions
- [ ] `tpt-des-stats`: time-series analysis of queue lengths and resource utilization
- [ ] `tpt-des-verify`: `proptest` strategies for generating valid queuing networks, Petri net topologies, and transition matrices
- [ ] End-to-end test: multi-class queuing network — simulate, then verify `tpt-des-stats` reproduces known analytical confidence bounds

## Phase 4 — Umbrella Crate, Docs & Examples

- [ ] `tpt-des` umbrella crate: feature-gated re-exports of all 8 sub-crates
- [ ] Top-level `README.md`; per-crate README/API docs; doctests for public API surface
- [ ] Worked example: M/M/1 queue simulation validated against Erlang formulas
- [ ] Worked example: stochastic Petri net reachability analysis
- [ ] Worked example: Markov chain steady-state / absorption analysis
- [ ] Worked example: resource contention with deadlock detection
- [ ] Benchmarks (`criterion`, matching `tpt-math`/`tpt-dsp` convention) for event-calendar scheduling and matrix-exponential hot paths

## Phase 5 — Release v1.0

- [ ] Full `cargo-deny` audit pass across all sub-crates
- [ ] `CHANGELOG.md`; align versions across all workspace members
- [ ] Confirm dual LICENSE files and `authors = ["TPT Solutions"]` consistent across every crate
- [ ] Decide publish target: crates.io vs. internal/path-dependency-only
- [ ] Tag and publish v1.0.0
