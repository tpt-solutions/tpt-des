//! Capacity sizing with the Erlang formulas.
//!
//! A call centre receives 120 calls an hour, and each call takes 3 minutes on
//! average, so the offered load is `a = λ/μ = 6` erlangs. Two questions:
//!
//! 1. **Agents.** How many agents keep the probability that a call waits
//!    (Erlang C) at or below 20%?
//! 2. **Trunk lines.** How many lines keep the probability that a call is
//!    blocked outright (Erlang B) at or below 1%?
//!
//! Both are closed-form, so this example needs no simulation. The Erlang C
//! answer is cross-checked against the mean wait from `mmc`.
//!
//! Run with `cargo run -p tpt-des --example call_center_erlang --features std`.

use tpt_des::queue::{erlang_b, erlang_c, mmc};

const CALLS_PER_HOUR: f64 = 120.0;
const MEAN_CALL_MINUTES: f64 = 3.0;

fn main() {
    let lambda = CALLS_PER_HOUR / 60.0; // calls per minute
    let mu = 1.0 / MEAN_CALL_MINUTES; // calls served per minute per agent
    let load = lambda / mu;
    println!("offered load a = {load:.1} erlangs");

    println!("\nagents  P(wait)  mean wait (min)");
    let mut agents = None;
    for c in 1..=20u32 {
        if (c as f64) <= load {
            println!("{c:>6}  unstable");
            continue;
        }
        let p_wait = erlang_c(load, c).expect("c exceeds the load");
        let wq = mmc(lambda, mu, c).expect("stable").wq;
        println!("{c:>6}  {p_wait:>7.3}  {wq:>15.3}");
        if agents.is_none() && p_wait <= 0.20 {
            agents = Some(c);
        }
    }
    let agents = agents.expect("some agent count meets the 20% target");
    println!("\nsmallest team with P(wait) ≤ 20%: {agents} agents");

    println!("\nlines  P(block)");
    let mut lines = None;
    for n in 1..=20u32 {
        let p_block = erlang_b(load, n).expect("valid parameters");
        if n <= 12 || p_block <= 0.01 {
            println!("{n:>5}  {p_block:>8.4}");
        }
        if lines.is_none() && p_block <= 0.01 {
            lines = Some(n);
        }
    }
    let lines = lines.expect("some line count meets the 1% target");
    println!("\nsmallest trunk group with P(block) ≤ 1%: {lines} lines");

    assert!(agents >= 1 && lines >= 1);
}
