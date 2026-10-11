# tpt-des-queue

Queueing theory in two forms: closed-form results you can check by hand, and discrete-event simulation of queues that the formulas cannot cover.

Use it to size a call centre (Erlang B and C), to predict a single server's waiting time (M/M/1, M/G/1), to solve a network of stations (Jackson networks and mean value analysis), or to simulate a queue with any arrival and service distribution, then check the simulation against the formula.

## Features

- **Closed form.** M/M/1 (`mm1`, `mm1_pn`), M/M/c (`mmc`), M/G/1 with the Pollaczek–Khinchine formula (`mg1`), and Erlang B and C (`erlang_b`, `erlang_c`). The Erlang routines use stable recursions, and M/M/c works in log space, so large server counts do not overflow.
- **Networks.** Open Jackson networks (`JacksonNetwork`) with traffic equations solved by `tpt-math-linalg-dense`. Single-class closed networks (`ClosedNetwork`) and multi-class closed networks (`MultiClassClosed`) are solved by exact mean value analysis.
- **Simulation.** `run_queue` simulates one station. `run_queue_multi` adds parallel servers, and `run_queue_traced` also returns a record for each job.
- **Disciplines.** FIFO, LIFO, priority (lowest number first, FIFO among equals), and processor sharing (exact, work-conserving).
- **Planning.** `size_for_mean_wait` finds the fewest servers that meet a wait target. `validate_mmc` checks a simulation against M/M/c and returns an assertable report. `mean_wait_to_precision` adds replications until a confidence half-width target is met.
- **Traces.** Export job records as CSV, JSON, or Chrome Trace Event format, or compute a 64-bit fingerprint for replay checks.

## Quick start

Simulate an M/M/1 queue and compare it with the formula:

```rust
use tpt_des_core::Time;
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
    Time::from_raw(20_000.0), // horizon
    Time::from_raw(2_000.0),  // warm-up: discarded from the statistics
    42,                       // seed: the same seed replays the same run
);
let exact = mm1(lambda, mu).unwrap();
assert!((metrics.mean_sojourn().raw() - exact.w).abs() / exact.w < 0.25);
```

Closed-form sizing needs no simulation:

```rust
use tpt_des_queue::erlang_c;

// Probability that a call waits, with 6 erlangs of traffic and 10 agents.
let p_wait = erlang_c(6.0, 10).unwrap();
assert!(p_wait > 0.0 && p_wait < 1.0);
```

## Module map

| Module | Main items |
|---|---|
| `analytic` | `mm1`, `mm1_pn`, `mmc`, `mg1`, `erlang_b`, `erlang_c`, and their result structs |
| `network` | `JacksonNetwork`, `ClosedNetwork`, `MvaResult` |
| `multiclass` | `MultiClassClosed`, `ClassSpec`, `MultiClassResult` |
| `sim` | `run_queue`, `run_queue_multi`, `run_queue_traced`, `QueueConfig`, `QueueMetrics`, `Discipline`, `JobRecord` |
| `planning` | `size_for_mean_wait`, `validate_mmc`, `mean_wait_to_precision` |
| `trace` | `trace_to_csv`, `trace_to_json`, `trace_to_chrome`, `trace_fingerprint` |

Metrics from a run:

- `mean_wait()`, `mean_sojourn()`, and `mean_wait_for_priority(p)` return `Time<Seconds>`.
- `time_avg_number()` is the time-average number in the system.
- `utilization()` is the busy fraction. It is close to ρ for a stable single server.

## Installation

```toml
[dependencies]
tpt-des-queue = "0.1"
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Required. The numerics depend on crates that need `std` or `alloc`. |
| `alloc` | no | Heap collections without `std`. |
| `serde` | no | `Serialize` on result types. Configs are not serialized, because they hold boxed samplers. |
| `parallel` | no | Runs `validate_mmc` replications on a rayon thread pool. Results are identical to the serial run, because each replication keeps its seed and the order is preserved. |

## Notes and limits

- `run_queue` models one station with one server. Use `run_queue_multi` for `c` servers, or the closed-form `mmc` and `erlang_c`.
- Processor sharing supports one server only in `run_queue_multi`.
- `mean_wait_to_precision` stays serial even with `parallel`, because its stopping rule depends on the order of replications.
- `run_queue` panics unless `0 ≤ warmup < horizon` and `horizon` is finite.
- Priority classes are drawn per arrival through `QueueConfig::priority`. Leave it as `None` for a single class.
- Results are `#[non_exhaustive]`. Matches on error enums and result structs need a wildcard arm.

## Testing

```sh
cargo test -p tpt-des-queue
cargo test -p tpt-des-queue --release
```

The `mm1_vs_analytic` suite compares replication means with the closed forms, using a stated tolerance.

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
