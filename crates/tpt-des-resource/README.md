# tpt-des-resource

Resource contention for discrete-event models: capacity pools, discrete stores, continuous fluid buffers, and a seize–delay–release controller with queueing, preemption, cancellation, and deadlock detection.

Use it when entities compete for something limited (machines, operators, tokens, tank volume), and you need to know who waits, who gets it next, and whether the waiting ever stops.

## Features

- **Pools.** `ResourcePool` holds a runtime capacity. `FinitePool<CAP>` holds a compile-time capacity, so the bound is checked by the type system. Both give all-or-nothing acquisition.
- **Stores.** `Store` holds discrete items, with an optional capacity. `put` and `get` return what was actually moved.
- **Buffers.** `Buffer` is a continuous fluid level. `pour` and `drain` move as much as the buffer allows, and `pour_duration` and `drain_duration` give the time a transfer takes at a rate.
- **Seize–delay–release.** `ResourceController` manages requests by entity and resource. It supports FIFO or priority queueing, `PreemptPolicy::PreemptivePriority` (an urgent request can evict less urgent holders), and cancellation of a pending request.
- **Deadlock detection.** The controller keeps a wait-for graph. `detect_deadlock` returns the cycle of entities that are waiting on each other.

## Quick start

```rust
use tpt_des_resource::{ResourceController, SeizeOutcome, SeizeRequest};

let mut rc = ResourceController::new();
rc.add_pool(0, 1).unwrap();

// Entity 0 takes the only unit. Entity 1 has to wait in line.
assert!(matches!(rc.seize(0, SeizeRequest::new(0, 1)), Ok(SeizeOutcome::Granted(_))));
assert!(matches!(rc.seize(1, SeizeRequest::new(0, 1)), Ok(SeizeOutcome::Queued(_))));

// Entity 0 releases; the unit passes to entity 1.
assert_eq!(rc.release(0, 0).unwrap(), vec![1]);
```

A pool with a compile-time capacity:

```rust
use tpt_des_resource::pool::FinitePool;

let mut machines: FinitePool<4> = FinitePool::new();
assert!(machines.acquire(3));
assert_eq!(machines.available(), 1);
```

## Module map

| Module | Main items |
|---|---|
| `pool` | `ResourcePool`, `FinitePool<CAPACITY>` |
| `store` | `Store` (discrete items), `Buffer` (fluid levels) |
| `sdr` | `ResourceController`, `SeizeRequest`, `SeizeOutcome`, `PreemptPolicy`, `RequestId` |
| `deadlock` | `WaitForGraph`, `find_cycle_in_digraph` |
| `error` | `ResourceError` |

## Installation

```toml
[dependencies]
tpt-des-resource = "0.1"
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Enables `alloc` and `tpt-des-core/std`. |
| `alloc` | no | Heap collections without `std`. |

## Notes and limits

- `seize` rejects a request larger than the pool's capacity with `ResourceError::InvalidParameter`. Such a request could never be granted.
- `Buffer` rejects NaN input.
- `PreemptPolicy` has three variants. `Fifo` queues waiters in arrival order with no preemption. `Priority` orders waiters by priority with no preemption. `PreemptivePriority` also lets a more urgent request evict lower-priority holders.
- Priorities follow the convention that a lower number is more urgent. Equal priorities queue FIFO.
- Deadlock detection looks for cycles in the wait-for graph. It reports a cycle that exists now. It does not predict a future one.
- `ResourceError` is `#[non_exhaustive]`. Matches need a wildcard arm.

## Testing

```sh
cargo test -p tpt-des-resource
```

The regression tests in `tests/` cover the fixes listed in the changelog.

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
