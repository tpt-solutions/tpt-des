# tpt-des-petri

Place/transition Petri nets: firing semantics, stochastic (Gillespie) simulation, reachability, invariants, and conversion to a CTMC.

## What's here

- `PetriNet` with weighted arcs, `is_enabled`, `fire`, and the incidence matrix.
- `StochasticPetriNet`: exponential firing times, with `simulate` (occupancy and firing counts) and `to_ctmc` (exact steady state for bounded nets).
- `reachability` (state-bounded BFS, deadlock markings) and `place_invariants` / `transition_invariants`. Place invariants are non-negative minimal supports (Farkas).

## Example

```rust
use tpt_des_petri::{Marking, PetriNet, StochasticPetriNet};

// A machine that breaks down and gets repaired.
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

let run = spn.simulate(&Marking(vec![1, 0]), 10_000.0, 7).unwrap();
assert!(run.total_firings > 0);
```

## Install

```toml
[dependencies]
tpt-des-petri = "0.1"
```

## Feature flags

- `std` (default): enables `alloc` and the `std` numerics.
- `alloc`: heap collections, without `std`.

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
