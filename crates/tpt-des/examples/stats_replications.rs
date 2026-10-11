//! Output analysis: independent replications with a confidence interval.
//!
//! Each replication runs an M/M/1 queue from empty with a different seed and
//! discards a warm-up period. The replication means are independent, so a
//! Student-t interval across them is the cleanest estimate of the steady-state
//! mean sojourn time. The analytic value is `W = 1/(μ − λ)`.
//!
//! Run with `cargo run -p tpt-des --example stats_replications --features "std queue stats"`.

use tpt_des::kernel::Time;
use tpt_des::queue::{mm1, run_queue, Discipline, Exponential, QueueConfig};
use tpt_des::stats::ci::mean_of_samples_ci;

const REPLICATIONS: u64 = 50;
const HORIZON: f64 = 50_000.0;
const WARMUP: f64 = 5_000.0;
const CONFIDENCE: f64 = 0.95;

fn main() {
    let (lambda, mu) = (0.7, 1.0);
    let exact = mm1(lambda, mu).expect("stable parameters");

    let means: Vec<f64> = (0..REPLICATIONS)
        .map(|seed| {
            let config = QueueConfig {
                arrival: Box::new(Exponential::new(lambda).expect("positive rate")),
                service: Box::new(Exponential::new(mu).expect("positive rate")),
                discipline: Discipline::Fifo,
                capacity: None,
                priority: None,
            };
            run_queue(
                config,
                Time::from_raw(HORIZON),
                Time::from_raw(WARMUP),
                seed,
            )
            .mean_sojourn()
            .raw()
        })
        .collect();

    let grand_mean = means.iter().sum::<f64>() / means.len() as f64;
    let (lo, hi) = mean_of_samples_ci(&means, CONFIDENCE).expect("at least two replications");

    println!("replications          : {REPLICATIONS}");
    println!("mean sojourn (sim)    : {grand_mean:.4}");
    println!(
        "{:.0}% confidence interval : [{lo:.4}, {hi:.4}]",
        100.0 * CONFIDENCE
    );
    println!("analytic W = 1/(μ−λ) : {:.4}", exact.w);

    let covered = lo <= exact.w && exact.w <= hi;
    println!("interval covers exact : {covered}");
    assert!(covered, "the 95% interval should cover the analytic value");
}
