//! WebAssembly bindings for `tpt-des`.
//!
//! Each function takes plain numbers (seeds are `u32`, so JavaScript passes them
//! as ordinary numbers) and returns a JSON string, so a page needs
//! no types from this crate. Bad input returns `{"error": "..."}` and never
//! panics, because a panic in WebAssembly aborts the page.

use tpt_des_core::{Seconds, Time};
use tpt_des_queue::planning::{size_for_mean_wait, validate_mmc};
use tpt_des_queue::sim::{run_queue_multi, Discipline, Exponential, QueueConfig};
use wasm_bindgen::prelude::*;

/// Simulate an M/M/1-style station (one server by default) and return the
/// mean `Wq`, `W`, and `L` over `replications` seeds as JSON.
#[wasm_bindgen]
pub fn run_queue_json(
    lambda: f64,
    mu: f64,
    servers: u32,
    horizon: f64,
    warmup: f64,
    replications: u32,
    seed: u32,
) -> String {
    match run_queue(lambda, mu, servers, horizon, warmup, replications, seed) {
        Ok(json) => json,
        Err(e) => error_json(&e),
    }
}

/// Compare a FIFO M/M/c simulation with the analytic values. Returns the report
/// rows as JSON, with a `passes` flag.
#[wasm_bindgen]
pub fn validate_mmc_json(
    lambda: f64,
    mu: f64,
    servers: u32,
    horizon: f64,
    warmup: f64,
    replications: u32,
    seed: u32,
) -> String {
    let checked = check_times(horizon, warmup).and_then(|(h, w)| {
        validate_mmc(lambda, mu, servers, h, w, replications, u64::from(seed))
            .map_err(|e| e.to_string())
    });
    match checked {
        Ok(report) => {
            let rows: Vec<String> = report
                .rows
                .iter()
                .map(|r| {
                    format!(
                        "{{\"metric\":\"{}\",\"analytic\":{},\"simulated\":{},\"std_error\":{},\"passes\":{}}}",
                        r.metric, r.analytic, r.simulated.mean, r.simulated.std_error, r.passes
                    )
                })
                .collect();
            format!(
                "{{\"servers\":{},\"passes\":{},\"rows\":[{}]}}",
                report.servers,
                report.passes(),
                rows.join(",")
            )
        }
        Err(e) => error_json(&e),
    }
}

/// The fewest servers whose analytic mean wait is at most `max_wait`.
#[wasm_bindgen]
pub fn size_json(lambda: f64, mu: f64, max_wait: f64) -> String {
    let target = match finite_time(max_wait) {
        Ok(t) => t,
        Err(e) => return error_json(&e),
    };
    match size_for_mean_wait(lambda, mu, target) {
        Ok(s) => format!(
            "{{\"servers\":{},\"wq\":{},\"utilization\":{}}}",
            s.servers, s.analytic.wq, s.analytic.rho
        ),
        Err(e) => error_json(&e.to_string()),
    }
}

fn run_queue(
    lambda: f64,
    mu: f64,
    servers: u32,
    horizon: f64,
    warmup: f64,
    replications: u32,
    seed: u32,
) -> Result<String, String> {
    if replications < 2 {
        return Err("replications must be at least 2".to_owned());
    }
    let (h, w) = check_times(horizon, warmup)?;
    let mut wq = Vec::new();
    let mut w_sum = Vec::new();
    let mut l = Vec::new();
    for i in 0..replications {
        let config = QueueConfig {
            arrival: Box::new(Exponential::new(lambda).map_err(|e| e.to_string())?),
            service: Box::new(Exponential::new(mu).map_err(|e| e.to_string())?),
            discipline: Discipline::Fifo,
            capacity: None,
            priority: None,
        };
        let m = run_queue_multi(
            config,
            servers as usize,
            h,
            w,
            u64::from(seed).wrapping_add(u64::from(i)),
        );
        wq.push(m.mean_wait().raw());
        w_sum.push(m.mean_sojourn().raw());
        l.push(m.time_avg_number());
    }
    let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
    Ok(format!(
        "{{\"wq\":{},\"w\":{},\"l\":{},\"replications\":{}}}",
        mean(&wq),
        mean(&w_sum),
        mean(&l),
        replications
    ))
}

/// Typed times for a horizon and warm-up, or an error for a value that is not
/// a finite time or that breaks `0 ≤ warm-up < horizon`.
fn check_times(horizon: f64, warmup: f64) -> Result<(Time<Seconds>, Time<Seconds>), String> {
    let h = finite_time(horizon)?;
    let w = finite_time(warmup)?;
    if !(horizon > 0.0 && (0.0..horizon).contains(&warmup)) {
        return Err("need 0 ≤ warmup < horizon, with horizon > 0".to_owned());
    }
    Ok((h, w))
}

fn finite_time(value: f64) -> Result<Time<Seconds>, String> {
    if value.is_finite() {
        Ok(Time::from_raw(value))
    } else {
        Err(format!("time value {value} is not finite"))
    }
}

/// `{"error": "..."}` with the message escaped for JSON.
fn error_json(message: &str) -> String {
    let mut escaped = String::new();
    for c in message.chars() {
        match c {
            '"' | '\\' => {
                escaped.push('\\');
                escaped.push(c);
            }
            c => escaped.push(c),
        }
    }
    format!("{{\"error\":\"{escaped}\"}}")
}
