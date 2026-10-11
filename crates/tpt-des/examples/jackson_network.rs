//! An open Jackson network with feedback, and its closed-network analogue.
//!
//! Three stations in series, with feedback from station 1 to station 0:
//!
//! ```text
//!   external ──▶ [0] ──▶ [1] ──0.6──▶ [2] ──▶ exit
//!                 ▲       │
//!                 └─0.4───┘
//! ```
//!
//! External arrivals enter station 0 at rate 1.0. The traffic equations
//! `λ = γ + Pᵀλ` give the station arrival rates; the product form then gives
//! each station's utilization and mean queue length. Finally the same service
//! demands are fed to exact mean value analysis (MVA) for a closed network.
//!
//! Run with `cargo run -p tpt-des --example jackson_network --features std`.

use tpt_des::queue::{ClosedNetwork, JacksonNetwork};

fn main() {
    let external = [1.0, 0.0, 0.0];
    let routing = [
        vec![0.0, 1.0, 0.0],
        vec![0.4, 0.0, 0.6],
        vec![0.0, 0.0, 0.0], // station 2 exits the network
    ];
    let service_rates = [3.0, 2.0, 2.5];
    let net = JacksonNetwork::new(&external, &routing, &service_rates).expect("valid network");

    // Analytic traffic equations: λ0 = 1 + 0.4·λ0 ⇒ λ0 = λ1 = 1/0.6, λ2 = 0.6·λ1.
    let lambda = net.node_arrivals().expect("solvable traffic equations");
    // λ2 = 0.6·λ1 = 0.6 · (1/0.6) = 1.
    let expected = [1.0 / 0.6, 1.0 / 0.6, 1.0];
    println!("arrival rates λ_i : {:.4?}", lambda);
    for (got, want) in lambda.iter().zip(expected) {
        assert!((got - want).abs() < 1e-9, "traffic equation mismatch");
    }

    let rho = net.utilizations().expect("stable network");
    let (lengths, total) = net.mean_lengths().expect("stable network");
    println!("utilizations ρ_i  : {:.4?}", rho);
    println!("mean lengths L_i  : {:.4?}", lengths);
    println!("total L           : {total:.4}");

    // Closed analogue: demands D_i = V_i / μ_i, with visit ratios V = λ/γ.
    let demands: Vec<f64> = lambda
        .iter()
        .zip(service_rates)
        .map(|(l, mu)| l / mu)
        .collect();
    let closed = ClosedNetwork::new(&demands).expect("positive demands");
    let bottleneck = 1.0 / demands.iter().cloned().fold(0.0, f64::max);
    println!("\nclosed network with demands {:.4?}", demands);
    println!("bottleneck throughput bound: {bottleneck:.4}");
    println!("  N   X(N)    L_total");
    let mut last = None;
    for n in 1..=8u32 {
        let r = closed.mva(n).expect("valid population");
        println!(
            "{n:>3}  {:.4}  {:.4}",
            r.throughput,
            r.queue_lengths.iter().sum::<f64>()
        );
        last = Some(r);
    }
    let last = last.expect("at least one population");
    assert!(
        last.throughput <= bottleneck + 1e-9,
        "throughput cannot exceed the bottleneck"
    );
    let total_customers: f64 = last.queue_lengths.iter().sum();
    assert!(
        (total_customers - 8.0).abs() < 1e-6,
        "customers are conserved in the closed network"
    );
}
