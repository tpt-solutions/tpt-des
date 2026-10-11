//! Worked example: a stochastic Petri net for a bounded M/M/1/2 queue —
//! reachability analysis, invariant conservation, and exact steady state
//! via CTMC conversion.

use tpt_des::kernel::Time;
use tpt_des::petri::{reachability, Marking, PetriNet, StochasticPetriNet};

fn main() {
    // Places: queue (jobs waiting/in service), slots (capacity tokens).
    // Transitions: arrive (λ = 1, needs a slot), serve (μ = 2, needs a job).
    let mut net = PetriNet::new();
    let queue = net.add_place("queue");
    let slots = net.add_place("slots");
    let arrive = net.add_transition("arrive");
    let serve = net.add_transition("serve");
    net.add_arc_pt(slots, arrive, 1);
    net.add_arc_tp(arrive, queue, 1);
    net.add_arc_pt(queue, serve, 1);
    net.add_arc_tp(serve, slots, 1);

    let mut spn = StochasticPetriNet::new(net);
    spn.set_rate(arrive, 1.0).unwrap();
    spn.set_rate(serve, 2.0).unwrap();

    // Reachability from the empty queue: 3 states (queue = 0, 1, 2).
    let m0 = Marking(vec![0, 2]);
    let graph = reachability(spn.net(), &m0, 1_000).unwrap();
    println!("reachable markings: {}", graph.len());
    for (i, m) in graph.states.iter().enumerate() {
        println!(
            "  state {i}: {m}  (deadlock: {})",
            graph.edges[i].is_empty()
        );
    }

    // Conservation: queue + slots = 2 in every reachable marking.
    let conserved = graph
        .states
        .iter()
        .all(|m| (m.at(queue) + m.at(slots)) == 2);
    println!("queue + slots conserved: {conserved}");
    assert!(conserved);

    // Exact steady state via CTMC conversion (ρ = 0.5, truncated geometric).
    let (ctmc, states) = spn.to_ctmc(&m0, 1_000).unwrap();
    let pi = ctmc.stationary().unwrap();
    println!("\nstationary distribution (exact, via CTMC):");
    for (k, p) in pi.iter().enumerate() {
        println!("  P(queue = {k}) = {p:.6}");
    }

    // Gillespie simulation occupancy should approach the exact values.
    let run = spn.simulate(&m0, Time::from_raw(100_000.0), 7).unwrap();
    println!("\nsimulated occupancy over {:.0} time units:", run.horizon);
    for k in 0..states.len() {
        let marking = Marking(vec![k as u32, (2 - k as u32)]);
        println!("  P(queue = {k}) ≈ {:.6}", run.occupancy_fraction(&marking));
    }
}
