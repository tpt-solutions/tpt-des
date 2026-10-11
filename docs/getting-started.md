# Getting started

This page takes you from nothing to a validated simulation in about ten
minutes.

## 1. Add the dependency

The crates are not on crates.io yet, so depend on the workspace by path (or
by git). When a release is published, switch to a version requirement.

```toml
[dependencies]
tpt-des = { path = "../tpt-des/crates/tpt-des", features = ["std", "queue"] }
```

Choose the features for the models you need. See
[choosing a crate](choosing-a-crate.md).

## 2. Run an M/M/1 queue

An M/M/1 queue has Poisson arrivals at rate λ, exponential service at rate μ,
and one server. Its mean sojourn time is `W = 1/(μ − λ)`, so you can check
the simulation against a known answer.

```rust
use tpt_des::queue::{mm1, run_queue, Discipline, Exponential, QueueConfig};

fn main() {
    let (lambda, mu) = (0.75, 1.0);

    let metrics = run_queue(
        QueueConfig {
            arrival: Box::new(Exponential::new(lambda).unwrap()),
            service: Box::new(Exponential::new(mu).unwrap()),
            discipline: Discipline::Fifo,
            capacity: None,
            priority: None,
        },
        Time::from_raw(50_000.0), // horizon: simulated time to run
        Time::from_raw(5_000.0),  // warm-up: discard measurements before this time
        42,       // seed: the same seed gives the same result
    );

    let exact = mm1(lambda, mu).unwrap();
    println!("simulated W = {:.3}", metrics.mean_sojourn());
    println!("analytic  W = {:.3}", exact.w);
}
```

Run it with `cargo run --release`. The simulated and analytic values should
agree to within a few percent.

## 3. Learn the pattern

Every model in this library has the same three parts:

1. **Configure.** Build the model from distributions and parameters. The
   queue takes a `QueueConfig`, a Petri net takes a `PetriNet` plus rates, and
   a process model takes a `ProcessSystem`.
2. **Run.** Give a horizon, and a seed when it matters. Runs are
   deterministic, so the same inputs always produce the same output.
3. **Measure.** Read metrics, and warm up and replicate before you trust
   them. [Validating a model](validating-a-model.md) covers this step.

## 4. Look at the examples

The umbrella crate has runnable examples. Each one declares the features it
needs, so run it with those features:

```sh
cargo run -p tpt-des --example mm1_vs_erlang --features "std queue"
cargo run -p tpt-des --example process_bank_teller --features "std process"
cargo run -p tpt-des --example stats_replications --features "std queue stats"
cargo run -p tpt-des --example hybrid_bridge --features "std process"
cargo run -p tpt-des --example call_center_erlang --features "std queue"
cargo run -p tpt-des --example inventory_store --features "std resource"
cargo run -p tpt-des --example jackson_network --features "std queue"
cargo run -p tpt-des --example markov_absorption --features "std markov"
cargo run -p tpt-des --example spn_reachability --features "std petri"
cargo run -p tpt-des --example deadlock_detection --features "std resource"
```

Each example checks its own result and exits with an error if the check
fails.

## Next steps

- [Choosing a crate](choosing-a-crate.md): which crate fits which question.
- [Validating a model](validating-a-model.md): how to show that a model is right.
- [FAQ](faq.md): answers to common questions.
- [Spec versus implementation](status.md): what is built and what is not.
