# tpt-des-process

Process-oriented simulation: write each entity as an `async` function that waits on time and resources, then run the set of processes on a deterministic executor.

## What's here

- `ProcessSystem` and `Sim`: `spawn`, `delay`, `acquire` and `release` (FIFO, all-or-nothing), `wait_until` with `notify_all`, and `interrupt`.
- `hybrid::HybridBridge`: steps a continuous model in fixed increments and interrupts a process when a guard crosses a threshold. The `ContinuousModel` trait is the seam where a solver such as `tpt-control` plugs in.

## Example

```rust
use tpt_des_process::{ProcessSystem, Sim};

let mut system = ProcessSystem::new(1);
system.add_resource(0, 1); // one machine

system.spawn(|sim: Sim| async move {
    sim.acquire(0, 1).await?;   // wait for the machine
    sim.delay(5.0).await?;      // run for five time units
    sim.release(0, 1);
    Ok(())
});
system.run();
assert_eq!(system.now(), 5.0);
```

## Install

```toml
[dependencies]
tpt-des-process = "0.1"
```

## Feature flags

- `std` (default): required. The executor uses `std::task::Wake`, and the workspace forbids `unsafe`, so this crate is std-only.

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
