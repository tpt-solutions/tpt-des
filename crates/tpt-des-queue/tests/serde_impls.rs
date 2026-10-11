//! Compile-time check that the result types serialize when the `serde`
//! feature is on. Configs hold boxed samplers and are deliberately not
//! serializable.
#![cfg(feature = "serde")]

use serde::Serialize;
use tpt_des_queue::sim::{ClassWait, JobRecord, QueueMetrics};
use tpt_des_queue::{Mg1Result, Mm1Result, MmcResult, MultiClassResult};

fn assert_serialize<T: Serialize>() {}

#[test]
fn result_types_implement_serialize() {
    assert_serialize::<Mm1Result>();
    assert_serialize::<MmcResult>();
    assert_serialize::<Mg1Result>();
    assert_serialize::<QueueMetrics>();
    assert_serialize::<ClassWait>();
    assert_serialize::<JobRecord>();
    assert_serialize::<MultiClassResult>();
    assert_serialize::<tpt_des_queue::network::MvaResult>();
    assert_serialize::<tpt_des_queue::planning::ValidationReport>();
    assert_serialize::<tpt_des_queue::planning::Sizing>();
}
