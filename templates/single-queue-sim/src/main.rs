//! A single-server queue, simulated with replications and checked against
//! the M/M/1 formula. Edit the rates and the replication count, then run
//! `cargo run --release`.

use tpt_des::queue::{mm1, run_queue, Discipline, Exponential, QueueConfig};
use tpt_des::stats::ci::mean_of_samples_ci;

const ARRIVAL_RATE: f64 = 0.7;
const SERVICE_RATE: f64 = 1.0;
const REPLICATIONS: u64 = 30;
const HORIZON: f64 = 50_000.0;
const WARMUP: f64 = 5_000.0;

fn main() {
    let means: Vec<f64> = (0..REPLICATIONS)
        .map(|seed| {
            let config = QueueConfig {
                arrival: Box::new(Exponential::new(ARRIVAL_RATE).expect("positive rate")),
                service: Box::new(Exponential::new(SERVICE_RATE).expect("positive rate")),
                discipline: Discipline::Fifo,
                capacity: None,
                priority: None,
            };
            run_queue(config, HORIZON, WARMUP, seed).mean_sojourn()
        })
        .collect();

    let (lo, hi) = mean_of_samples_ci(&means, 0.95).expect("at least two replications");
    let exact = mm1(ARRIVAL_RATE, SERVICE_RATE).expect("stable queue");
    println!("mean sojourn, simulated : [{lo:.3}, {hi:.3}] (95% CI)");
    println!("mean sojourn, M/M/1     : {:.3}", exact.w);
}
