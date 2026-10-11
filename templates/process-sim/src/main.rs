//! Process-oriented model: customers arrive, wait for one of two servers,
//! are served, and leave. Each customer is an `async` process. Edit the
//! constants to change the model.

use std::cell::RefCell;
use std::rc::Rc;

use tpt_des::kernel::Time;
use tpt_des::process::{ProcessSystem, Sim};

const SERVERS: u64 = 0;
const CUSTOMERS: usize = 10_000;
const ARRIVAL_RATE: f64 = 1.6;
const SERVICE_RATE: f64 = 1.0;

fn exponential(sim: &Sim, rate: f64) -> f64 {
    -(1.0 - sim.uniform()).ln() / rate
}

fn main() {
    let waits = Rc::new(RefCell::new(Vec::with_capacity(CUSTOMERS)));
    let mut system = ProcessSystem::new(1);
    system.add_resource(SERVERS, 2);

    let waits_for_arrivals = waits.clone();
    system.spawn(move |sim: Sim| async move {
        for _ in 0..CUSTOMERS {
            sim.delay(Time::from_raw(exponential(&sim, ARRIVAL_RATE))).await?;
            let waits = waits_for_arrivals.clone();
            sim.spawn(move |sim: Sim| async move {
                let arrived = sim.now();
                sim.acquire(SERVERS, 1).await?;
                waits.borrow_mut().push((sim.now() - arrived).raw());
                sim.delay(Time::from_raw(exponential(&sim, SERVICE_RATE))).await?;
                sim.release(SERVERS, 1);
                Ok(())
            });
        }
        Ok(())
    });

    system.run();

    let waits = waits.borrow();
    let mean_wait = waits.iter().sum::<f64>() / waits.len() as f64;
    println!("served {} customers; mean wait {:.3}", waits.len(), mean_wait);
}
