# Spec versus implementation

This page compares [`spec.txt`](../spec.txt) with the code as of the 0.1.x
line. The spec is the original design. Where the two disagree, the code and
its tests are the reference, and this page says which.

**Status key.** *Done* means implemented and tested. *Partial* means part of
the spec item is implemented. *Not implemented* means no code exists for it.

## Core (`tpt-des-core`)

| Spec item | Status | Where / notes |
|---|---|---|
| Unit-safe clock with phantom time units | Done | `Time<Unit>` with `Ticks`, `Seconds`, `Hours` |
| Binary-heap event calendar, `O(log n)`, cancellation | Done | `EventCalendar`; cancellation uses lazy tombstones |
| `Entity` lifecycle (`on_create`, `on_event`, `on_terminate`) | Done | `Entity` trait |
| Deterministic replay via a seeded RNG | Done | `SimRng` (SplitMix64); replay is tested bit-for-bit |
| `no_std` + `alloc` core | Done, core only | CI builds `tpt-des-core` for `thumbv6m-none-eabi` and `wasm32-unknown-unknown` |

## Queuing (`tpt-des-queue`)

| Spec item | Status | Where / notes |
|---|---|---|
| Closed forms: M/M/1, M/M/c, M/G/1, Erlang B/C | Done | `analytic`; Erlang C is computed in log space, so large `c` is safe |
| Jackson networks | Done | `JacksonNetwork` (open networks with routing matrices) |
| Closed networks with routing matrices | Done | Single-chain by exact MVA (`ClosedNetwork`). Multi-class by exact MVA over the population lattice (`MultiClassClosed`), with visit ratios from each class's routing matrix. The lattice is capped at `MAX_STATES` (100 000). Exact for processor-sharing nodes, and for FCFS nodes with exponential service that is the same across classes. |
| Multi-server simulation and capacity planning | Done | `run_queue_multi` runs c servers. `planning::size_for_mean_wait` gives the fewest servers that meet a mean-wait target. `planning::validate_mmc` compares simulation with M/M/c. `planning::mean_wait_to_precision` adds replications until the CI half-width meets a target. Processor sharing is single-server only. |
| Job-level trace export | Done | `run_queue_traced`, with `trace_to_csv`, `trace_to_json`, and `trace_to_chrome` (Chrome Trace Event format, one lane per concurrent service), plus `trace_fingerprint`, a 64-bit replay check over the seed and records. |
| Parallel replications | Done (validation only) | Feature `parallel` (rayon) in `tpt-des-queue`, used by `validate_mmc`. Results match the serial run exactly. `mean_wait_to_precision` stays serial, because its stopping rule depends on replication order. |
| Typed time | Done | Times and durations in public APIs are `Time<Seconds>`. Rates and dimensionless values stay `f64`. See the changelog for the breaking list. |
| DES queues with arbitrary distributions | Done | `RateSampler`; `FromClosure` takes any distribution |
| FIFO, LIFO, priority, processor sharing | Done | Priority classes come from `QueueConfig::priority`. Before the Phase 6 fixes, priority behaved as FIFO. |

## Resources (`tpt-des-resource`)

| Spec item | Status | Where / notes |
|---|---|---|
| Resource pools with capacity and preemption | Done | `ResourcePool`, `FinitePool<CAP>`, `PreemptPolicy` |
| Stores (discrete) and buffers (fluid) | Done | `Store`, `Buffer` |
| Deadlock detection via resource-graph cycles | Done | `deadlock::WaitForGraph`, `find_cycle` |
| Seize–delay–release with timeout and cancellation | Done | `ResourceController`; timeouts are a caller timer plus `cancel` |

## Processes (`tpt-des-process`)

| Spec item | Status | Where / notes |
|---|---|---|
| Coroutine-based processes | Done | Written as `async` bodies. The spec also mentioned generator-style yield primitives, which are not implemented. |
| `wait_for(resource)`, `delay`, `wait_until` | Done | Named `acquire`, `delay`, and `wait_until` in the code |
| Interruption, preemption, resumption | Done | `interrupt` delivers an `Interrupt` to the pending operation. Preempting a *holder* is done by the resource controller. |
| Hybrid discrete–continuous integration with `tpt-control` | Partial | `tpt-control` is not in this workspace, so `hybrid::ContinuousModel` is defined locally. A solver that implements that trait can plug in with no bridge changes. |

## Petri nets (`tpt-des-petri`)

| Spec item | Status | Where / notes |
|---|---|---|
| Place/transition nets with markings | Done | `PetriNet`, `Marking` |
| Stochastic Petri nets, exponential firing | Done | `StochasticPetriNet::simulate` (Gillespie) |
| Reachability graphs and P/T-invariants | Done | `reachability`; `place_invariants` (Farkas), `transition_invariants` |
| Conversion to a CTMC for steady state | Done | `StochasticPetriNet::to_ctmc`, bounded by `MAX_CTMC_STATES` |

## Markov chains (`tpt-des-markov`)

| Spec item | Status | Where / notes |
|---|---|---|
| Row-stochastic validation | Done | `Dtmc::from_rows`, `from_weights` |
| Steady state by power iteration or eigendecomposition | Done, by a different method | Steady state is a direct linear solve, not power iteration. It converges for periodic chains, which power iteration does not. |
| Absorbing chains: time to absorption and absorption probabilities | Done | `absorb::analyze` |
| First-passage and mean recurrence times | Done | `fpt::mean_first_passage`, `mean_recurrence_times` (Kac) |

## Statistics (`tpt-des-stats`)

| Spec item | Status | Where / notes |
|---|---|---|
| Warm-up detection (Welch, MSIR) | Done | `warmup::welch`, `welch_warmup`, `mser`, `mser_stride`. `msir` is an alias for `mser_stride` (use stride 5 for MSER-5). |
| Batch means, replication/deletion, regenerative | Done | `batch` |
| Confidence intervals for means, quantiles, proportions | Done | `ci` (Student-t, order statistics, Wilson) |
| Time-series analysis of queues and utilization | Partial | `TimeWeightedAvg` gives exact time averages; `autocorrelation` and `RunningStats` are available. There is no dedicated time-series type. |

## Verification (`tpt-des-verify`)

| Spec item | Status | Where / notes |
|---|---|---|
| proptest strategies: networks, Petri topologies, transition matrices | Partial | Strategies exist for queue parameters, row-stochastic and sub-stochastic matrices, acyclic edge lists, markings, and schedule plans. There is no Petri-topology strategy. |
| Kani: calendar heap ordering | Done (harness) | `calendar_pops_are_sorted`, `calendar_cancelled_events_never_fire`, `calendar_live_len_is_exact` |
| Kani: no deadlock in acyclic resource graphs | Done (harness) | `acyclic_wait_graph_has_no_deadlock`, `acyclic_resource_graph_has_no_deadlock` |
| Kani: entity conservation in closed networks | Done (harness) | `closed_network_conserves_entities`, which runs the real `Simulation` |
| Kani: Markov rows sum to 1 | Done (harness) | `dtmc_normalization_maintains_probability_mass` |

The Kani harnesses are not run in every environment. CI runs them as a
non-blocking job. Run `just kani` to check them locally.

## Integration and scope

| Spec item | Status | Where / notes |
|---|---|---|
| Gradient or differentiable hooks (§1, §5) | Cut for 0.1.x | No code and no design. Scope decision, recorded in `spec.txt`. |
| `tpt-systems-optimisation` integration (server-count optimization) | Deferred | Out of scope for this pass. The single-server and multi-class solvers it would call now exist. |
| End-to-end multi-class network test (Phase 3 in the spec) | Partial | Unit-level checks: a single class matches the single-chain MVA, customer counts are conserved, identical classes aggregate correctly, and routing is validated. The server-count optimization test is deferred with the integration above. |
| `tpt-swarm`, `tpt-robotics`, `tpt-eve` / `tpt-anima` | Not implemented | Named in the spec as ecosystem integrations, outside this repository |
| Command-line runner | Done (unpublished) | `crates/tpt-des-cli`, binary `tpt-des`: `run`, `validate`, `size`, and `network` on TOML files |
| WebAssembly bindings and playground | Done (unpublished) | `crates/tpt-des-wasm`. Bindings were tested in Node. The playground page was not run in a browser. |

## Licensing

The crates ship as **MIT OR Apache-2.0**, and `spec.txt`'s license line now says
the same (the original design said *MIT*). The dependency policy (no
Apache-2.0-only crates, enforced by `cargo deny`) is unchanged. The spec's
differentiable and "AI-native" wording is marked as cut for 0.1.x.
