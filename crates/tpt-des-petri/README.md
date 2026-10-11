# tpt-des-petri

Place/transition Petri nets for modelling concurrency, synchronisation, and resource flow: firing semantics, stochastic (Gillespie) simulation, reachability analysis, invariants, and exact conversion to a continuous-time Markov chain.

## Features

- **Nets.** `PetriNet` with weighted arcs (`add_arc_pt`, `add_arc_tp`), enabling (`is_enabled`, `enabled`), firing (`fire`), and the incidence matrix.
- **Stochastic simulation.** `StochasticPetriNet` attaches an exponential rate to each transition. `simulate` runs the Gillespie direct method and reports firing counts, the final marking, and the time spent in each marking (the occupancy).
- **Exact steady state.** `to_ctmc` builds the reachability graph and returns it as a `Ctmc`, so `tpt-des-markov` can solve for the stationary distribution of a bounded net.
- **Reachability.** `reachability` explores the state space breadth-first up to a state bound and lists the deadlock markings.
- **Invariants.** `place_invariants` returns the minimal non-negative place invariants (Farkas), and `transition_invariants` returns the T-invariants. `is_conservation_law` and `weighted_total` check a weighting against a net.

## Quick start

A machine that breaks down and gets repaired, simulated under the Gillespie method:

```rust
use tpt_des_core::Time;
use tpt_des_petri::{Marking, PetriNet, StochasticPetriNet};

let mut net = PetriNet::new();
let up = net.add_place("up");
let down = net.add_place("down");
let fail = net.add_transition("fail");
let repair = net.add_transition("repair");
net.add_arc_pt(up, fail, 1);
net.add_arc_tp(fail, down, 1);
net.add_arc_pt(down, repair, 1);
net.add_arc_tp(repair, up, 1);

let mut spn = StochasticPetriNet::new(net);
spn.set_rate(fail, 0.1).unwrap();
spn.set_rate(repair, 1.0).unwrap();

let run = spn
    .simulate(&Marking(vec![1, 0]), Time::from_raw(10_000.0), 7)
    .unwrap();
assert!(run.total_firings > 0);
```

The same net, solved exactly. Availability should be `1.0 / (1.0 + 0.1) ≈ 0.909`:

```rust
use tpt_des_core::Time;
use tpt_des_petri::{Marking, PetriNet, StochasticPetriNet};

let mut net = PetriNet::new();
let up = net.add_place("up");
let down = net.add_place("down");
let fail = net.add_transition("fail");
let repair = net.add_transition("repair");
net.add_arc_pt(up, fail, 1);
net.add_arc_tp(fail, down, 1);
net.add_arc_pt(down, repair, 1);
net.add_arc_tp(repair, up, 1);

let mut spn = StochasticPetriNet::new(net);
spn.set_rate(fail, 0.1).unwrap();
spn.set_rate(repair, 1.0).unwrap();

let (ctmc, markings) = spn.to_ctmc(&Marking(vec![1, 0]), 1_000).unwrap();
let pi = ctmc.stationary().unwrap();
let up_index = markings.iter().position(|m| m.0 == vec![1, 0]).unwrap();
assert!((pi[up_index] - 1.0 / 1.1).abs() < 1e-9);
```

## Module map

| Module | Main items |
|---|---|
| `net` | `PetriNet`, `Marking`, `Transition` |
| `spn` | `StochasticPetriNet`, `SpnRun`, `SpnError`, `MAX_CTMC_STATES` |
| `reach` | `reachability`, `ReachabilityGraph`, `ReachabilityError`, `DEFAULT_MAX_STATES` |
| `invariant` | `place_invariants`, `transition_invariants`, `is_conservation_law`, `weighted_total` |

## Installation

```toml
[dependencies]
tpt-des-petri = "0.1"
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Enables `alloc` and the `std` numerics of the crates it uses. |
| `alloc` | no | Heap collections without `std`. |

## Notes and limits

- `to_ctmc` refuses a net with more than `MAX_CTMC_STATES` (4096) reachable markings. The generator is stored densely, so the bound keeps memory in check.
- Reachability stops at a state bound and returns an error rather than running forever. Pass a bound that fits the net.
- A firing that would push a place past `u32::MAX` tokens is refused, and reported as `SpnError::TokenOverflow`.
- `simulate` needs a finite horizon. `Time` cannot hold a non-finite value, so an infinite run is not expressible.
- Firing rates must be finite and non-negative. Otherwise `set_rate` returns `SpnError::InvalidRate`.
- `SpnError` and `ReachabilityError` are `#[non_exhaustive]`. Matches on them need a wildcard arm.

## Testing

```sh
cargo test -p tpt-des-petri
```

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
