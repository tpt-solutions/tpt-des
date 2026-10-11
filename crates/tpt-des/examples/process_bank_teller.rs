//! Process-oriented simulation: a two-teller bank.
//!
//! Customers arrive as a Poisson stream. Each one *acquires* a teller (a
//! two-unit resource), is served for an exponential time, and *releases* the
//! teller. The mean queue wait is compared with the M/M/2 result from the
//! Erlang C formula, `W_q = C(c, a) / (cμ − λ)`.
//!
//! Run with `cargo run -p tpt-des --example process_bank_teller --features process`.

use std::cell::RefCell;
use std::rc::Rc;

use tpt_des::process::{ProcessSystem, Sim};
use tpt_des::queue::mmc;

const TELLERS: u64 = 0;
const CUSTOMERS: usize = 200_000;
const ARRIVAL_RATE: f64 = 1.6; // customers per minute
const SERVICE_RATE: f64 = 1.0; // served per minute per teller

/// Draw an exponential variate with the given rate from the simulation RNG.
fn exponential(sim: &Sim, rate: f64) -> f64 {
    -(1.0 - sim.uniform()).ln() / rate
}

fn main() {
    let waits = Rc::new(RefCell::new(Vec::with_capacity(CUSTOMERS)));
    let mut system = ProcessSystem::new(2024);
    system.add_resource(TELLERS, 2);

    // The arrival process spawns one customer process per arrival.
    let waits_for_arrivals = waits.clone();
    system.spawn(move |sim: Sim| async move {
        for _ in 0..CUSTOMERS {
            sim.delay(exponential(&sim, ARRIVAL_RATE)).await?;
            let waits = waits_for_arrivals.clone();
            sim.spawn(move |sim: Sim| async move {
                let arrived = sim.now();
                sim.acquire(TELLERS, 1).await?;
                waits.borrow_mut().push(sim.now() - arrived);
                sim.delay(exponential(&sim, SERVICE_RATE)).await?;
                sim.release(TELLERS, 1);
                Ok(())
            });
        }
        Ok(())
    });

    system.run();

    let waits = waits.borrow();
    // Discard the first 10% as warm-up.
    let steady = &waits[waits.len() / 10..];
    let measured = steady.iter().sum::<f64>() / steady.len() as f64;

    let analytic = mmc(ARRIVAL_RATE, SERVICE_RATE, 2).expect("stable: λ < cμ");
    println!("customers served     : {}", waits.len());
    println!("mean queue wait (sim): {measured:.4} min");
    println!("mean queue wait (M/M/2): {:.4} min", analytic.wq);
    println!("P(wait) (Erlang C)   : {:.4}", analytic.erlang_c);

    let rel = (measured - analytic.wq).abs() / analytic.wq;
    println!("relative error       : {:.2}%", 100.0 * rel);
    assert!(rel < 0.10, "simulation diverged from Erlang C");
}
