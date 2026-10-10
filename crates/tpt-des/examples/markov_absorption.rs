//! Worked example: gambler's ruin as an absorbing Markov chain — expected
//! time to absorption and absorption probabilities, plus steady state and
//! first-passage analysis of the surviving ergodic chain.

use tpt_des::markov::{absorb, fpt, Dtmc};

fn main() {
    // Gambler's ruin on {0..4}: absorbing at 0 (broke) and 4 (target),
    // fair coin flips in between.
    let n = 5;
    let mut rows = vec![vec![0.0; n]; n];
    rows[0][0] = 1.0;
    rows[4][4] = 1.0;
    for i in 1..4 {
        rows[i][i - 1] = 0.5;
        rows[i][i + 1] = 0.5;
    }
    let chain = Dtmc::from_rows(rows).unwrap();

    let analysis = absorb::analyze(&chain).unwrap();
    println!("gambler's ruin on 0..=4 (fair coin):");
    for (idx, &state) in analysis.transient_states.iter().enumerate() {
        println!(
            "  from {}: expected flips to absorption = {:.3}, P(reach 4) = {:.3}",
            state,
            analysis.expected_time_to_absorption[idx],
            analysis.absorption_probabilities[idx][1]
        );
    }

    // Mean first-passage times on the interior chain (treating 0 as target).
    let m0 = fpt::mean_first_passage(&chain, 0).unwrap();
    println!(
        "\nmean steps to hit 0: from 1 = {:.3}, from 2 = {:.3} (∞ if unreachable)",
        m0[1], m0[2]
    );

    // A purely ergodic chain for steady state: the two-state weather model.
    let weather = Dtmc::from_rows(vec![vec![0.9, 0.1], vec![0.5, 0.5]]).unwrap();
    let pi = weather.stationary(1e-14, 10_000).unwrap();
    let recurrence = fpt::mean_recurrence_times(&pi);
    println!("\ntwo-state weather chain:");
    println!("  stationary = ({:.6}, {:.6})", pi[0], pi[1]);
    println!(
        "  mean recurrence times = ({:.3}, {:.3})",
        recurrence[0], recurrence[1]
    );
}
