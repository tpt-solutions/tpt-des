//! Criterion benchmark for the event calendar hot path: scheduling,
//! cancellation, and draining O(10⁶) events.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tpt_des_core::{EntityId, EventCalendar, Ticks, Time};

fn bench_schedule_pop(c: &mut Criterion) {
    c.bench_function("calendar/schedule+pop 100k", |b| {
        b.iter(|| {
            let mut cal: EventCalendar<Ticks, u64> = EventCalendar::new();
            for i in 0..100_000u64 {
                let t = (i * 7919) % 1_000_003; // pseudo-shuffled times
                cal.schedule_at::<0>(Time::from_raw(f64::from(t as u32)), EntityId(0), i);
            }
            let mut n = 0u64;
            while let Some(ev) = cal.pop() {
                n += ev.payload;
            }
            n
        })
    });
}

fn bench_schedule_cancel(c: &mut Criterion) {
    c.bench_function("calendar/schedule+cancel 100k", |b| {
        b.iter(|| {
            let mut cal: EventCalendar<Ticks, u64> = EventCalendar::new();
            let mut ids = Vec::with_capacity(100_000);
            for i in 0..100_000u64 {
                let t = (i * 7919) % 1_000_003;
                ids.push(cal.schedule_at::<1>(Time::from_raw(f64::from(t as u32)), EntityId(0), i));
            }
            // Cancel every other event.
            for (i, id) in ids.iter().enumerate() {
                if i % 2 == 0 {
                    cal.cancel(*id);
                }
            }
            while cal.pop().is_some() {}
            black_box(cal.live_len())
        })
    });
}

criterion_group!(benches, bench_schedule_pop, bench_schedule_cancel);
criterion_main!(benches);
