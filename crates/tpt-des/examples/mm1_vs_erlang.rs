//! Worked example: simulate an M/M/1 queue and validate the measured mean
//! sojourn time against the analytical (Erlang/Cromer) formula
//! `W = 1/(μ − λ)`.

use tpt_des::kernel::Time;
use tpt_des::queue::{mm1, run_queue, Discipline, Exponential, QueueConfig};

fn main() {
    let (lambda, mu) = (0.75, 1.0);
    let exact = mm1(lambda, mu).expect("stable parameters");

    let metrics = run_queue(
        QueueConfig {
            arrival: Box::new(Exponential::new(lambda).unwrap()),
            service: Box::new(Exponential::new(mu).unwrap()),
            discipline: Discipline::Fifo,
            capacity: None,
            priority: None,
        },
        Time::from_raw(100_000.0),
        Time::from_raw(10_000.0),
        2024,
    );

    println!("M/M/1 with λ = {lambda}, μ = {mu} (ρ = {:.3})", exact.rho);
    println!("  metric       simulated   analytical");
    println!(
        "  W  (sojourn) {:>10.4}   {:>10.4}",
        metrics.mean_sojourn().raw(),
        exact.w
    );
    println!(
        "  Wq (wait)    {:>10.4}   {:>10.4}",
        metrics.mean_wait().raw(),
        exact.wq
    );
    println!(
        "  L  (number)  {:>10.4}   {:>10.4}",
        metrics.time_avg_number(),
        exact.l
    );
    println!(
        "  ρ  (util)    {:>10.4}   {:>10.4}",
        metrics.utilization(),
        exact.rho
    );

    let rel = (metrics.mean_sojourn().raw() - exact.w).abs() / exact.w;
    println!("\nrelative error on W: {rel:.2}%");
    assert!(rel < 0.15, "simulation diverged from Erlang theory");
}
