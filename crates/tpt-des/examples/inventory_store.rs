//! Inventory control with a discrete `Store` and a lead time.
//!
//! A shop sells between 0 and 10 units a day (uniform). It reorders 40 units
//! whenever stock falls to 10 or below and no order is already on the way.
//! Deliveries arrive after a 2-day lead time. The simulation reports the fill
//! rate: the share of demand met from stock.
//!
//! Run with `cargo run -p tpt-des --example inventory_store --features "std resource"`.

use tpt_des::core::{Ctx, Entity, EntityId, Simulation, Ticks, Time};
use tpt_des::resource::store::Store;

const CAPACITY: i64 = 60;
const REORDER_POINT: i64 = 10;
const ORDER_QTY: i64 = 40;
const LEAD_DAYS: f64 = 2.0;
const DAYS: u32 = 365;

#[derive(Clone, Copy, Debug)]
enum Event {
    /// A day ends: demand is served and the reorder rule is checked.
    Day,
    /// An order of this many units arrives.
    Delivery(i64),
}

struct Shop {
    me: EntityId,
    stock: Store,
    on_order: bool,
    day: u32,
    demand: i64,
    unmet: i64,
    orders: u32,
}

impl Entity<Ticks, Event> for Shop {
    fn on_create(&mut self, ctx: &mut Ctx<'_, Ticks, Event>) {
        // Start with one order's worth of stock and schedule the first day.
        self.stock.put(ORDER_QTY);
        ctx.schedule::<0>(Time::from_raw(1.0), self.me, Event::Day);
    }

    fn on_event(&mut self, event: Event, ctx: &mut Ctx<'_, Ticks, Event>) {
        match event {
            Event::Day => {
                self.day += 1;
                let demand = (ctx.uniform() * 11.0) as i64; // 0..=10
                let served = self.stock.get(demand);
                self.demand += demand;
                self.unmet += demand - served;

                if self.stock.level() <= REORDER_POINT && !self.on_order {
                    self.on_order = true;
                    self.orders += 1;
                    ctx.schedule::<0>(
                        Time::from_raw(LEAD_DAYS),
                        self.me,
                        Event::Delivery(ORDER_QTY),
                    );
                }
                if self.day < DAYS {
                    ctx.schedule::<0>(Time::from_raw(1.0), self.me, Event::Day);
                }
            }
            Event::Delivery(qty) => {
                // Units beyond capacity are refused by the store.
                self.stock.put(qty);
                self.on_order = false;
            }
        }
    }
}

fn main() {
    let mut shop = Shop {
        me: EntityId(0),
        stock: Store::with_capacity(CAPACITY).expect("positive capacity"),
        on_order: false,
        day: 0,
        demand: 0,
        unmet: 0,
        orders: 0,
    };

    {
        let mut sim: Simulation<'_, Ticks, Event> = Simulation::new(7);
        sim.add_entity(&mut shop);
        sim.run(u64::MAX);
    }

    let served = shop.demand - shop.unmet;
    let fill_rate = served as f64 / shop.demand as f64;
    println!("days simulated : {}", shop.day);
    println!("total demand   : {}", shop.demand);
    println!("unmet demand   : {}", shop.unmet);
    println!("orders placed  : {}", shop.orders);
    println!("fill rate      : {:.2}%", 100.0 * fill_rate);

    assert_eq!(shop.day, DAYS);
    assert!(
        fill_rate > 0.9,
        "a 40-unit order policy should serve most demand"
    );
}
