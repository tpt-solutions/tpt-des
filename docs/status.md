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
| Closed networks with routing matrices | Partial | Single-chain closed networks by exact MVA (`ClosedNetwork`). Multi-class closed networks are not implemented. |
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
| Gradient or differentiable hooks (§1, §5) | Not implemented | No code and no design |
| `tpt-systems-optimisation` integration (server-count optimization) | Not implemented | Needs the sibling crate and a multi-class network |
| End-to-end multi-class network test (Phase 3 in the spec) | Not implemented | Depends on multi-class closed networks |
| `tpt-swarm`, `tpt-robotics`, `tpt-eve` / `tpt-anima` | Not implemented | Named in the spec as ecosystem integrations, outside this repository |

## Licensing

The spec's header says *MIT*. The crates ship as **MIT OR Apache-2.0**. The
dependency policy (no Apache-2.0-only crates, enforced by `cargo deny`) is the
same in both. `spec.txt` has not been edited, because it is the design
document.
