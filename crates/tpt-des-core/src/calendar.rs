//! The event calendar: a from-scratch, cancellation-capable priority queue.
//!
//! The calendar is a binary heap ordered by `(time, priority, insertion
//! sequence)` — earliest time first, then lowest priority number first, then
//! FIFO among equal `(time, priority)` pairs. Both [`schedule_at`] and
//! [`cancel`] run in `O(log n)`; popping runs in amortized `O(log n)` using
//! *lazy cancellation*: a cancelled event is tombstoned and discarded when it
//! surfaces from the heap, rather than being searched for and repaired.
//!
//! [`schedule_at`]: EventCalendar::schedule_at
//! [`cancel`]: EventCalendar::cancel
//!
//! # Compile-time priorities
//!
//! The event priority is a `const` generic parameter, so a call site's
//! priority is part of the program text:
//!
//! ```
//! use tpt_des_core::{EntityId, EventCalendar, Time, Ticks};
//!
//! let mut cal: EventCalendar<Ticks, &'static str> = EventCalendar::new();
//! let _ = cal.schedule_at::<5>(Time::from_raw(1.0), EntityId(0), "low priority");
//! let _ = cal.schedule_at::<0>(Time::from_raw(1.0), EntityId(0), "high priority");
//!
//! // Same time: the lower priority number fires first, FIFO otherwise.
//! assert_eq!(cal.pop().unwrap().payload, "high priority");
//! assert_eq!(cal.pop().unwrap().payload, "low priority");
//!
//! // Cancellation is idempotent and tombstones the entry.
//! let id = cal.schedule_at::<0>(Time::from_raw(2.0), EntityId(0), "doomed");
//! assert!(cal.cancel(id));
//! assert!(!cal.cancel(id)); // already cancelled
//! assert!(cal.pop().is_none());
//! ```

use crate::entity::EntityId;
use crate::time::Time;
use alloc::collections::{BTreeSet, BinaryHeap};
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;

/// A handle to a scheduled event, usable for cancellation.
///
/// Handles are only meaningful within the calendar that produced them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub(crate) u64);

impl EventId {
    /// The raw numeric id (useful for tracing / deterministic replay logs).
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// One entry in the heap.
///
/// [`Ord`] is implemented so that the *largest* entry is the one that should
/// fire first (BinaryHeap is a max-heap): earlier time, then lower priority
/// number, then lower insertion sequence.
struct HeapEntry<E> {
    time: f64,
    priority: u8,
    seq: u64,
    id: u64,
    target: EntityId,
    payload: E,
}

impl<E> PartialEq for HeapEntry<E> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == core::cmp::Ordering::Equal
    }
}

impl<E> Eq for HeapEntry<E> {}

impl<E> PartialOrd for HeapEntry<E> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<E> Ord for HeapEntry<E> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // Reverse each comparison: the "greatest" entry fires first.
        other
            .time
            .total_cmp(&self.time)
            .then_with(|| other.priority.cmp(&self.priority))
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

/// An event popped from the calendar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Popped<E> {
    /// Time the event was scheduled for.
    pub time: f64,
    /// The event's (compile-time) priority.
    pub priority: u8,
    /// The cancellation handle (already consumed — the event has fired).
    pub id: EventId,
    /// The entity the event is addressed to.
    pub target: EntityId,
    /// The event payload.
    pub payload: E,
}

/// A binary-heap event calendar, ordered by `(time, priority, sequence)`.
///
/// Generic over the time unit (phantom type) and the event payload `E`.
/// Every event carries a [`target`](EntityId) — the entity it is addressed
/// to — so the simulation loop can dispatch without re-sorting anything.
pub struct EventCalendar<Unit, E> {
    heap: BinaryHeap<HeapEntry<E>>,
    /// Ids currently resident in the heap (pending, not yet fired/cancelled).
    pending: BTreeSet<u64>,
    /// Tombstones: ids cancelled while still pending.
    cancelled: BTreeSet<u64>,
    next_seq: u64,
    next_id: u64,
    _unit: PhantomData<fn() -> Unit>,
}

impl<Unit, E> fmt::Debug for EventCalendar<Unit, E>
where
    E: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventCalendar")
            .field("live", &self.live_len())
            .field("heap_entries", &self.heap.len())
            .field("cancelled_pending", &self.cancelled.len())
            .finish()
    }
}

impl<Unit, E> Default for EventCalendar<Unit, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Unit, E> EventCalendar<Unit, E> {
    /// Create an empty calendar.
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            pending: BTreeSet::new(),
            cancelled: BTreeSet::new(),
            next_seq: 0,
            next_id: 0,
            _unit: PhantomData,
        }
    }

    /// Number of live (scheduled, not cancelled) events.
    ///
    /// Cancelled entries that have not yet been lazily discarded still sit in
    /// the heap but are excluded from this count.
    pub fn live_len(&self) -> usize {
        self.heap.len() - self.cancelled.len()
    }

    /// True if no live events remain.
    pub fn is_empty(&self) -> bool {
        self.live_len() == 0
    }

    /// Total entries physically in the heap, including cancelled tombstones
    /// awaiting lazy discard. Exposed mainly for tests and the Kani proofs.
    pub fn heap_len(&self) -> usize {
        self.heap.len()
    }

    /// Schedule `payload` for `target` at absolute time `time`, with the
    /// compile-time priority `PRIORITY` (lower fires first among events at
    /// the same time).
    ///
    /// `O(log n)`. Returns a handle for [`cancel`](Self::cancel).
    pub fn schedule_at<const PRIORITY: u8>(
        &mut self,
        time: Time<Unit>,
        target: EntityId,
        payload: E,
    ) -> EventId {
        let id = self.next_id;
        self.next_id += 1;
        let seq = self.next_seq;
        self.next_seq += 1;
        self.heap.push(HeapEntry {
            time: time.raw(),
            priority: PRIORITY,
            seq,
            id,
            target,
            payload,
        });
        self.pending.insert(id);
        EventId(id)
    }

    /// Cancel a pending event. `O(log n)`.
    ///
    /// Returns `true` if the event was pending and is now cancelled; `false`
    /// if it had already fired or was already cancelled. Cancellation is lazy:
    /// the tombstoned entry is discarded when it surfaces from the heap.
    pub fn cancel(&mut self, id: EventId) -> bool {
        if self.pending.remove(&id.0) {
            self.cancelled.insert(id.0);
            true
        } else {
            false
        }
    }

    /// Remove and return the next event to fire, skipping cancelled entries.
    ///
    /// The heap invariant (pops are non-decreasing in
    /// `(time, priority, seq)`) is proved for small schedules by the Kani
    /// harnesses in `tpt-des-verify`.
    pub fn pop(&mut self) -> Option<Popped<E>> {
        loop {
            let entry = self.heap.pop()?;
            if self.cancelled.remove(&entry.id) {
                continue; // tombstone: discard lazily
            }
            self.pending.remove(&entry.id);
            return Some(Popped {
                time: entry.time,
                priority: entry.priority,
                id: EventId(entry.id),
                target: entry.target,
                payload: entry.payload,
            });
        }
    }

    /// Time of the next live (non-cancelled) event, without consuming it.
    ///
    /// Peeks past tombstones by temporarily popping; still amortized
    /// `O(log n)`.
    pub fn next_time(&mut self) -> Option<f64> {
        let mut held: Vec<HeapEntry<E>> = Vec::new();
        let mut result = None;
        while let Some(entry) = self.heap.pop() {
            if self.cancelled.contains(&entry.id) {
                continue; // tombstone: discard
            }
            result = Some(entry.time);
            held.push(entry);
            break;
        }
        for entry in held {
            self.heap.push(entry);
        }
        result
    }

    /// Like [`pop`](Self::pop), but never returns events scheduled after
    /// `max_time` (raw units): the loop stops there, leaving the entry in the
    /// heap. Used by `Simulation::run_until` to bound a run in time.
    pub fn pop_until(&mut self, max_time: f64) -> Option<Popped<E>> {
        loop {
            let entry = self.heap.pop()?;
            if entry.time > max_time {
                self.heap.push(entry);
                return None;
            }
            if self.cancelled.remove(&entry.id) {
                continue; // tombstone: discard lazily
            }
            self.pending.remove(&entry.id);
            return Some(Popped {
                time: entry.time,
                priority: entry.priority,
                id: EventId(entry.id),
                target: entry.target,
                payload: entry.payload,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Ticks;

    fn cal() -> EventCalendar<Ticks, u64> {
        EventCalendar::new()
    }

    const A: EntityId = EntityId(0);

    #[test]
    fn pops_in_time_order() {
        let mut c = cal();
        c.schedule_at::<0>(Time::from_raw(5.0), A, 5);
        c.schedule_at::<0>(Time::from_raw(1.0), A, 1);
        c.schedule_at::<0>(Time::from_raw(3.0), A, 3);
        assert_eq!(c.pop().unwrap().payload, 1);
        assert_eq!(c.pop().unwrap().payload, 3);
        assert_eq!(c.pop().unwrap().payload, 5);
        assert!(c.pop().is_none());
    }

    #[test]
    fn priority_then_fifo_at_same_time() {
        let mut c: EventCalendar<Ticks, char> = EventCalendar::new();
        c.schedule_at::<2>(Time::from_raw(1.0), A, 'a');
        c.schedule_at::<1>(Time::from_raw(1.0), A, 'b');
        c.schedule_at::<1>(Time::from_raw(1.0), A, 'c');
        let pops: Vec<char> = core::iter::from_fn(|| c.pop().map(|e| e.payload)).collect();
        assert_eq!(pops, ['b', 'c', 'a']);
    }

    #[test]
    fn targets_are_carried_through() {
        let mut c: EventCalendar<Ticks, u64> = cal();
        c.schedule_at::<7>(Time::from_raw(1.0), EntityId(3), 9);
        let popped = c.pop().unwrap();
        assert_eq!(popped.target, EntityId(3));
        assert_eq!(popped.priority, 7);
        assert_eq!(popped.payload, 9);
    }

    #[test]
    fn cancellation_removes_event() {
        let mut c = cal();
        c.schedule_at::<0>(Time::from_raw(1.0), A, 1);
        let gone = c.schedule_at::<0>(Time::from_raw(2.0), A, 2);
        assert!(c.cancel(gone));
        assert!(!c.cancel(gone));
        assert_eq!(c.live_len(), 1);
        assert_eq!(c.pop().unwrap().payload, 1);
        assert!(c.pop().is_none());
    }

    #[test]
    fn cancelling_fired_event_is_noop() {
        let mut c = cal();
        let id = c.schedule_at::<0>(Time::from_raw(1.0), A, 1);
        assert!(c.pop().is_some());
        assert!(!c.cancel(id));
        assert_eq!(c.live_len(), 0);
    }

    #[test]
    fn live_len_accounts_for_lazy_tombstones() {
        let mut c = cal();
        for t in 0..10 {
            c.schedule_at::<0>(Time::from_raw(t as f64), A, t);
        }
        assert_eq!(c.live_len(), 10);
        // Cancel the first five scheduled (ids 0..5) — they fire at times 0..5.
        for id in 0..5 {
            assert!(c.cancel(EventId(id)));
        }
        assert_eq!(c.live_len(), 5);
        assert_eq!(c.heap_len(), 10);
        // Pop everything: only 5 real events come out.
        let mut n = 0;
        while c.pop().is_some() {
            n += 1;
        }
        assert_eq!(n, 5);
        assert_eq!(c.heap_len(), 0);
    }
}
