//! A machine that fails and gets repaired, modelled as a stochastic Petri
//! net. The exact steady state comes from the equivalent CTMC; the
//! simulated occupancy should agree with it. Edit the rates to explore.

use tpt_des::kernel::Time;
use tpt_des::petri::{Marking, PetriNet, StochasticPetriNet};

const FAIL_RATE: f64 = 0.1;
const REPAIR_RATE: f64 = 1.0;

fn main() {
    let mut net = PetriNet::new();
    let up = net.add_place("up");
    let down = net.add_place("down");
    let fail = net.add_transition("fail");
    let repair = net.add_transition("repair");
    net.add_arc_pt(up, fail, 1);
    net.add_arc_tp(fail, down, 1);
    net.add_arc_pt(down, repair, 1);
    net.add_arc_tp(repair, up, 1);

    let mut spn = StochasticPetriNet::new(net);
    spn.set_rate(fail, FAIL_RATE).expect("valid rate");
    spn.set_rate(repair, REPAIR_RATE).expect("valid rate");

    let initial = Marking(vec![1, 0]);
    let (ctmc, markings) = spn.to_ctmc(&initial, 1_000).expect("bounded net");
    let steady = ctmc.stationary().expect("irreducible chain");
    for (marking, p) in markings.iter().zip(&steady) {
        println!("exact steady state  {marking}: {p:.4}");
    }

    let run = spn.simulate(&initial, Time::from_raw(100_000.0), 11).expect("valid horizon");
    for (marking, time) in &run.occupancy {
        println!("simulated occupancy {marking}: {:.4}", time.raw() / run.horizon.raw());
    }
}
