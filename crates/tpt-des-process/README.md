# tpt-des-process

Process-oriented simulation: write each entity as an `async` function that waits on time, resources, and conditions, then run all of them on a deterministic executor.

This style suits models where an entity has a story ("arrive, wait for a clerk, serve, leave"). It is often easier to read than wiring the same story by hand as events. The `async`/`await` syntax is the only thing that the processes yield on, so there is no hidden scheduling.

## Features

- **Delays.** `Sim::delay(d)` suspends a process for a simulated duration.
- **Resources.** `acquire(resource, n)` and `release(resource, n)` wait for and return units of a named resource. Granting is FIFO and all-or-nothing.
- **Conditions.** `wait_until(predicate)` suspends a process until a predicate over simulation state is true. `notify_all()` re-checks the waiters. Predicates are never polled.
- **Interruption.** `interrupt(pid)` makes a waiting process return `Err(Interrupt)` from the operation it is blocked on, in the style of SimPy.
- **Hybrid models.** `hybrid::HybridBridge` steps a continuous model in fixed increments and interrupts a process when a guard crosses a threshold. The `ContinuousModel` trait is the seam where a numerical solver can be plugged in.
- **Determinism.** `ProcessSystem::new(seed)` seeds the random number generator, and the executor is single-threaded, so runs replay exactly.

## Quick start

```rust
use tpt_des_core::Time;
use tpt_des_process::{ProcessSystem, Sim};

let mut system = ProcessSystem::new(1);
system.add_resource(0, 1); // one machine

system.spawn(|sim: Sim| async move {
    sim.acquire(0, 1).await?;              // wait for the machine
    sim.delay(Time::from_raw(5.0)).await?; // run for five time units
    sim.release(0, 1);
    Ok(())
});
system.run();
assert_eq!(system.now().raw(), 5.0);
```

A process that loops over a fixed sequence of delays:

```rust
use tpt_des_core::Time;
use tpt_des_process::ProcessSystem;

let mut sys = ProcessSystem::new(42);
sys.spawn(|sim| async move {
    for _ in 0..3 {
        sim.delay(Time::from_raw(2.0)).await?;
    }
    Ok(())
});
sys.run();
assert_eq!(sys.now().raw(), 6.0);
```

## Module map

| Module | Main items |
|---|---|
| `system` | `ProcessSystem` (owns the executor), `Sim` (the handle a process uses), `ProcessId`, `ProcessOutput` |
| `ops` | `Delay`, `Acquire`, `WaitUntil` (futures returned by `Sim`), `Interrupt` |
| `hybrid` | `ContinuousModel`, `HybridBridge` |

A process body returns `ProcessOutput`, which is `Result<(), Interrupt>`. Use `?` on each blocking operation, so that an interrupt ends the process cleanly.

## Installation

```toml
[dependencies]
tpt-des-process = "0.1"
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Required. The executor uses `std::task::Wake`, and the workspace forbids `unsafe`, so the crate has no `no_std` build. |

## Notes and limits

- `Sim::delay` panics on a negative or NaN duration.
- `Sim::acquire` panics when the request is larger than the resource's capacity, because it could never be granted.
- `release` clamps at capacity, and a finished process is dropped.
- `HybridBridge::new` and `step` validate their inputs.
- Process bodies run in the order they became runnable. The executor does not run them in parallel.

## Testing

```sh
cargo test -p tpt-des-process
```

The regression tests in `tests/` cover the executor ordering and the acquire, release, and interrupt fixes listed in the changelog.

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
