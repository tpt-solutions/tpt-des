# tpt-des-resource

Resource contention: capacity pools, discrete stores, continuous buffers, and a seize–delay–release controller with queueing, preemption, cancellation, and deadlock detection.

## What's here

- `ResourcePool` and `FinitePool<CAP>` (a compile-time capacity): all-or-nothing acquisition.
- `Store` (discrete items) and `Buffer` (fluid levels with rate-based pour and drain durations).
- `ResourceController`: `seize`, `release`, `cancel`, and FIFO or priority queueing. `PreemptPolicy::PreemptivePriority` lets an urgent request evict less urgent holders.
- `deadlock`: cycle detection over the wait-for graph, which reports the entities involved.

## Example

```rust
use tpt_des_resource::{ResourceController, SeizeOutcome, SeizeRequest};

let mut rc = ResourceController::new();
rc.add_pool(0, 1).unwrap();

assert!(matches!(rc.seize(0, SeizeRequest::new(0, 1)), Ok(SeizeOutcome::Granted(_))));
assert!(matches!(rc.seize(1, SeizeRequest::new(0, 1)), Ok(SeizeOutcome::Queued(_))));

// Entity 0 releases; the unit passes to entity 1.
assert_eq!(rc.release(0, 0).unwrap(), vec![1]);
```

## Install

```toml
[dependencies]
tpt-des-resource = "0.1"
```

## Feature flags

- `std` (default): enables `alloc` and `tpt-des-core/std`.
- `alloc`: heap collections, without `std`.

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
