# Choosing a crate

Start from the question you are answering, then pick the crate. Most projects
need one or two crates, and the umbrella `tpt-des` re-exports all of them
behind feature flags.

| If you need to… | Use | Start with |
|---|---|---|
| Schedule events and run entities on a clock | `tpt-des-core` | `Simulation`, `Entity`, `Time<Unit>` |
| Get a closed-form queue result (utilization, wait, Erlang B/C) | `tpt-des-queue` | `mm1`, `mmc`, `erlang_b`, `erlang_c` |
| Simulate one queue with any distributions and disciplines | `tpt-des-queue` | `run_queue`, `QueueConfig` |
| Model a network of queues | `tpt-des-queue` | `JacksonNetwork` (open), `ClosedNetwork` (closed, MVA) |
| Model shared machines, staff, or licences, with waiting and preemption | `tpt-des-resource` | `ResourceController` |
| Model discrete items moving through a buffer or store | `tpt-des-resource` | `Store` |
| Find deadlocks in a resource-allocation graph | `tpt-des-resource` | `deadlock::WaitForGraph` |
| Write each entity as a sequence of steps that wait on time and resources | `tpt-des-process` | `ProcessSystem`, `Sim` |
| Couple discrete events to a continuous quantity (a tank level, a temperature) | `tpt-des-process` | `hybrid::HybridBridge` |
| Model concurrency, synchronization, or bounded resources as a net | `tpt-des-petri` | `PetriNet`, `StochasticPetriNet` |
| Check a net for invariants or reachable deadlocks | `tpt-des-petri` | `reachability`, `place_invariants` |
| Analyze a Markov chain (steady state, absorption, first passage) | `tpt-des-markov` | `Dtmc`, `Ctmc`, `absorb`, `fpt` |
| Compute confidence intervals from simulation output | `tpt-des-stats` | `ci`, `batch`, `warmup` |
| Remove the warm-up transient from a run | `tpt-des-stats` | `warmup::mser`, `warmup::welch_warmup` |
| Test invariants with random inputs, or model-check the kernel | `tpt-des-verify` | proptest strategies; Kani harnesses |

## Which features do I need?

Enable only what you use. Every feature except `core` requires `std`.

- **Queueing models:** `std`, `queue`, and `stats` if you want intervals.
- **Process models:** `std`, `process`, and `resource` if processes wait on
  resources.
- **Chains and nets:** `std`, `markov`, or `petri`. `petri` also enables
  `markov`, since it can convert a net to a chain.
- **Embedded or WebAssembly targets:** `core` only, with
  `default-features = false`.

## Which should I not use?

- If you need **gradient-based optimization** of the model, this library does
  not provide it. The spec mentions differentiable hooks, but they are not
  implemented ([status](status.md)).
- If you need a **multi-class closed network**, only single-chain closed
  networks are implemented (see [status](status.md)).
- If you need **multiple servers in a simulated queue**, `run_queue` models
  one server. Use `mmc` and `erlang_c` for the closed form, or the process
  layer (`ProcessSystem` with a two-unit resource) to simulate them.
