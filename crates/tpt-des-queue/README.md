# tpt-des-queue

Queueing theory in two forms: closed-form results you can check by hand, and discrete-event simulation of single-station queues.

## What's here

- **Closed form:** M/M/1, M/M/c (`mmc`), M/G/1 (Pollaczek–Khinchine), and Erlang B and C (`erlang_b`, `erlang_c`, with stable recursions).
- **Networks:** open Jackson networks (`JacksonNetwork`) and exact mean value analysis for closed networks (`ClosedNetwork`).
- **Simulation:** `run_queue` drives a single-station queue with any arrival and service distribution. Disciplines are FIFO, LIFO, priority, and processor sharing.

## Example

```rust
use tpt_des_queue::{mm1, run_queue, Discipline, Exponential, QueueConfig};

let (lambda, mu) = (0.5, 1.0);
let metrics = run_queue(
    QueueConfig {
        arrival: Box::new(Exponential::new(lambda).unwrap()),
        service: Box::new(Exponential::new(mu).unwrap()),
        discipline: Discipline::Fifo,
        capacity: None,
        priority: None,
    },
    20_000.0, // horizon
    2_000.0,  // warm-up
    42,       // seed
);
let exact = mm1(lambda, mu).unwrap();
assert!((metrics.mean_sojourn() - exact.w).abs() / exact.w < 0.25);
```

## Install

```toml
[dependencies]
tpt-des-queue = "0.1"
```

## Limitations

- `run_queue` models one station with one server. Multi-server queues are covered by the closed-form `mmc` and `erlang_c`, and by the process layer (`tpt-des-process`).
- Priority classes are drawn per arrival through `QueueConfig::priority`. The priority discipline selects the lowest number first, FIFO among equals.

## Feature flags

- `std` (default): required. The numerics depend on crates that need `std` or `alloc`.

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
