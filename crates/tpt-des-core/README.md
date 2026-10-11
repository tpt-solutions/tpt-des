# tpt-des-core

The simulation engine behind every other `tpt-des` crate: a unit-safe clock, a from-scratch event calendar, entity lifecycle hooks, and a seeded, deterministic random number generator.

Most users reach this crate through the [`tpt-des`](../tpt-des/README.md) umbrella, but it can be used on its own when you only need the event loop.

## Features

- **Unit-safe time.** `Time<Unit>` carries a phantom unit (`Ticks`, `Seconds`, `Hours`), so adding seconds to hours is a compile error. Conversions are explicit (`to_seconds`, `from_hours`, `from_minutes`, `from_millis`).
- **Event calendar.** `EventCalendar` is a binary heap with `O(log n)` scheduling. Events fire in `(time, priority, insertion order)` order. Cancellation is by `EventId`, and stale ids are rejected.
- **Compile-time priorities.** The priority is a `const` generic (`schedule::<PRIORITY>`), so a call site's priority is part of the program text. A lower number fires first at equal times.
- **Entities.** The `Entity` trait has `on_create`, `on_event`, and `on_terminate` hooks. `Ctx` lets an entity schedule and cancel events while it handles one.
- **Deterministic replay.** `SimRng` is a seeded SplitMix64 generator. The same seed and the same schedule give a bit-for-bit identical run. `draws()` counts the random draws, which gives a cheap fingerprint of a run.
- **`no_std` + `alloc`.** The crate builds without `std` for `thumbv6m-none-eabi` and `wasm32-unknown-unknown`. CI checks both targets.

## Quick start

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

## Module map

| Module | Main items | Purpose |
|---|---|---|
| `time` | `Time<Unit>`, `Ticks`, `Seconds`, `Hours`, `SECONDS_PER_HOUR` | Typed simulation time. |
| `calendar` | `EventCalendar`, `EventId`, `Popped` | Priority queue of pending events. |
| `entity` | `Entity`, `Ctx`, `EntityId` | Entity trait and the context passed to handlers. |
| `sim` | `Simulation`, `SimClock`, `RunStats` | The run loop: `step`, `run`, `run_until`, and statistics. |
| `rng` | `SimRng` | Seeded generator for reproducible runs. |

Useful entry points on `Simulation`:

- `new(seed)` builds a simulation. `add_entity` registers an entity and returns its `EntityId`.
- `start_at::<PRIORITY>(time, target, payload)` seeds the first event.
- `step()` processes one event. `run(max_events)` processes up to `max_events` events. `run_until(horizon)` processes every event at or before `horizon`, and then moves the clock to the horizon.
- `stats()` reports events processed and events dropped.

## Installation

```toml
[dependencies]
tpt-des-core = "0.1"
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Enables `alloc` and the `std`-dependent numerics. |
| `alloc` | no | Heap collections (maps, the event heap, dynamic dispatch) without `std`. |
| `serde` | no | Serializes `Time<Unit>` as its raw `f64` value. |

Build without `std`:

```sh
cargo build -p tpt-des-core --no-default-features --features alloc
```

## Notes and limits

- `Time::from_raw` panics on a non-finite value, and `from_raw` checks its input in every build profile.
- The clock never moves backwards. `SimClock::advance_to` panics if asked to move it backwards.
- Event handlers run one at a time, in calendar order. The crate does not run anything in parallel.

## Testing

```sh
cargo test -p tpt-des-core
cargo bench -p tpt-des-core     # criterion: calendar hot path
```

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
