# tpt-des-core

The simulation engine: a unit-safe clock, a from-scratch event calendar, entity lifecycle hooks, and a deterministic seeded RNG.

## What's here

- `Time<Unit>` with phantom units (`Ticks`, `Seconds`, `Hours`), so mixing units is a compile error.
- `EventCalendar`: a binary-heap calendar with `O(log n)` scheduling, cancellation by id, and compile-time `const` priorities that break ties at equal times.
- `Entity` (with `on_create`, `on_event`, `on_terminate`) and `Simulation`, which runs entities in registration order.
- `SimRng`: a seeded SplitMix64 generator. The same seed and schedule replay bit-for-bit, and `draws()` gives a cheap fingerprint of the run.

## Example

```rust
use tpt_des_core::{Ctx, Entity, EntityId, Seconds, Simulation, Time};

struct Ticker { ticks: u32 }

impl Entity<Seconds, ()> for Ticker {
    fn on_event(&mut self, _: (), ctx: &mut Ctx<'_, Seconds, ()>) {
        self.ticks += 1;
        if self.ticks < 10 {
            ctx.schedule::<0>(Time::from_raw(1.0), EntityId(0), ());
        }
    }
}

let mut ticker = Ticker { ticks: 0 };
let mut sim = Simulation::new(42);
sim.add_entity(&mut ticker);
sim.start_at::<0>(Time::from_raw(0.0), EntityId(0), ());
sim.run(u64::MAX);
assert_eq!(ticker.ticks, 10);
```

## Install

```toml
[dependencies]
tpt-des-core = "0.1"
```

## Feature flags

- `std` (default): enables `alloc` and the `std`-dependent numerics.
- `alloc`: heap collections, without `std`.

With `default-features = false` the crate builds for `thumbv6m-none-eabi` and `wasm32-unknown-unknown`; CI checks both.

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
