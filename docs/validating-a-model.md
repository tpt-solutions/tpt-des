# Validating a model

A simulation is only useful once you know it matches reality, or at least
matches the theory it should follow. This page sets out the checks, in the
order to run them.

## 1. Check against a closed-form case

Replace your model with a simpler one that has a known answer, and check that
the simulation reproduces it. Good references:

| Your model | Check against | Function |
|---|---|---|
| One server, Poisson arrivals, exponential service | M/M/1 | `mm1` |
| Several identical servers | M/M/c and Erlang C | `mmc`, `erlang_c` |
| Loss system, no waiting room | Erlang B | `erlang_b` |
| Open network with feedback | Traffic equations, product form | `JacksonNetwork` |
| Closed network, one class | Mean value analysis | `ClosedNetwork::mva` |
| Absorbing chain | Gambler's ruin, fundamental matrix | `absorb::analyze` |

The repository has tests of this kind. `crates/tpt-des-queue/tests/mm1_vs_analytic.rs`
sweeps M/M/1 parameters, and `examples/mm1_vs_erlang.rs` shows the pattern.

## 2. Run long enough, and warm up

Measurements taken from an empty system are biased. The first part of the run
is the *transient*, and you should discard it.

- Find the warm-up point. `warmup::mser` takes a series of observations and
  returns how many to delete. Use `mser_stride(series, 5)` (MSER-5) if the
  observations are correlated, which is usual.
- Set the warm-up in `run_queue` (its third argument) or drop the early
  observations yourself.

A rough rule: the horizon should be much longer than the time it takes the
system to settle, and the post-warm-up window should contain thousands of
completed jobs.

## 3. Replicate, and report an interval

One long run gives one number with no error bar. Several independent
replications give an interval.

```rust
use tpt_des::stats::ci::mean_of_samples_ci;

// One mean per replication, each with its own seed. `run_one_replication`
// is your function: it runs the model with that seed and returns the mean.
let replication_means: Vec<f64> = (0..30).map(run_one_replication).collect();
let (lo, hi) = mean_of_samples_ci(&replication_means, 0.95)?;
```

`mean_of_samples_ci` returns an error for fewer than two replications, which
is deliberate: one number has no interval.

Check that the analytic value falls inside the interval about 95% of the
time. A single failure is not a bug, because a 95% interval misses 5% of the
time. Repeated failures are.

## 4. Check conservation

Simple accounting catches many bugs. For a queue:

- **Arrivals:** `metrics.arrived` should match the number of arrivals you
  generated, minus `metrics.blocked`.
- **Flow:** `metrics.completed` cannot exceed `metrics.arrived`.
- **Little's law:** the time-average number in the system should equal the
  arrival rate times the mean sojourn time, `L = λW`. Use `time_avg_number()`
  and `mean_sojourn()`.

For a closed network the total number of jobs must not change. The Kani
harness `closed_network_conserves_entities` checks this property for the
engine.

## 5. Check determinism

Run the model twice with the same seed and compare every metric. If they
differ, something outside the seed (a hash-map iteration order, wall-clock
time, or a global RNG) is affecting the run. The library's own tests check
this for the queue and the calendar.

## 6. Stress the edges

- Set the load close to 1 and check that the mean wait grows as theory
  predicts. Expect longer runs and wider intervals there.
- Use zero or near-zero service and arrival times to check the event
  ordering.
- Feed the model a capacity of 1 and check the blocking probability against
  Erlang B.

## What to report

When you publish a result, give the parameters, the seed or seeds, the warm-up
and horizon, the number of replications, the interval, and the analytic check
you ran. Someone else can then repeat the run.
