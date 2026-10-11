//! The event calendar: a from-scratch, cancellation-capable priority queue.
//!
//! The calendar is a binary heap ordered by `(time, priority, insertion
//! sequence)` — earliest time first, then lowest priority number first, then
//! FIFO among equal `(time, priority)` pairs. [`schedule_at`] is `O(log n)`
//! and [`cancel`] is `O(1)`. Popping is amortized `O(log n)`, using *lazy
//! cancellation*: a cancelled event is tombstoned and discarded when it
//! surfaces from the heap, rather than being searched for and repaired.
//!
//! Each scheduled event occupies a slot in a slab. An [`EventId`] names a slot
//! and a generation, so a stale handle (already fired, cancelled, or whose slot
//! has been reused) is recognised and rejected in constant time.
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
use alloc::collections::BinaryHeap;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;

/// A handle to a scheduled event, usable for cancellation.
///
/// Handles are only meaningful within the calendar that produced them. The
/// raw value packs the slot index (low 32 bits) and the slot's generation
/// (high 32 bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub(crate) u64);

impl EventId {
    fn new(slot: u32, gen: u32) -> Self {
        EventId((u64::from(gen) << 32) | u64::from(slot))
    }

    fn slot(self) -> usize {
        (self.0 & u64::from(u32::MAX)) as usize
    }

    fn gen(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// The raw numeric id (useful for tracing / deterministic replay logs).
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// What a slot currently holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotState {
    /// Not in use. The slot is on the free list.
    Free,
    /// Scheduled and live: its heap entry will fire.
    Pending,
    /// Cancelled: its heap entry is a tombstone, discarded when it surfaces.
    Cancelled,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    gen: u32,
    state: SlotState,
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
    slot: u32,
    gen: u32,
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
///
/// Invariant: every heap entry corresponds to exactly one slot in
/// `Pending` or `Cancelled` state, and the slot is released only when that
/// entry leaves the heap. So `live` counts the `Pending` slots and the
/// `Cancelled` slots are the tombstones still in the heap.
pub struct EventCalendar<Unit, E> {
    heap: BinaryHeap<HeapEntry<E>>,
    slots: Vec<Slot>,
    /// Indices of `Free` slots, reused before the slab grows.
    free: Vec<u32>,
    /// Number of `Pending` slots (scheduled and not cancelled).
    live: usize,
    next_seq: u64,
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
            .field("cancelled_pending", &(self.heap.len() - self.live))
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
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
            next_seq: 0,
            _unit: PhantomData,
        }
    }

    /// Number of live (scheduled, not cancelled) events.
    ///
    /// Cancelled entries that have not yet been lazily discarded still sit in
    /// the heap but are excluded from this count.
    pub fn live_len(&self) -> usize {
        self.live
    }

    /// True if no live events remain.
    pub fn is_empty(&self) -> bool {
        self.live == 0
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
        let slot = match self.free.pop() {
            Some(s) => s,
            None => {
                let s = u32::try_from(self.slots.len())
                    .expect("event calendar holds at most 2^32 slots");
                self.slots.push(Slot {
                    gen: 0,
                    state: SlotState::Free,
                });
                s
            }
        };
        let entry_slot = &mut self.slots[slot as usize];
        entry_slot.state = SlotState::Pending;
        let gen = entry_slot.gen;

        let seq = self.next_seq;
        self.next_seq += 1;
        self.heap.push(HeapEntry {
            time: time.raw(),
            priority: PRIORITY,
            seq,
            slot,
            gen,
            target,
            payload,
        });
        self.live += 1;
        EventId::new(slot, gen)
    }

    /// Cancel a pending event. `O(1)`.
    ///
    /// Returns `true` if the event was pending and is now cancelled; `false`
    /// if it had already fired or was already cancelled. Cancellation is lazy:
    /// the tombstoned entry is discarded when it surfaces from the heap.
    pub fn cancel(&mut self, id: EventId) -> bool {
        match self.slots.get_mut(id.slot()) {
            Some(s) if s.gen == id.gen() && s.state == SlotState::Pending => {
                s.state = SlotState::Cancelled;
                self.live -= 1;
                true
            }
            _ => false,
        }
    }

    /// Free `slot` for reuse and bump its generation, so every outstanding
    /// handle to it goes stale.
    fn release(&mut self, slot: u32) {
        let s = &mut self.slots[slot as usize];
        s.gen = s.gen.wrapping_add(1);
        s.state = SlotState::Free;
        self.free.push(slot);
    }

    /// Remove the heap's top entry. Returns it if it is live, or `None` after
    /// discarding a tombstone. Callers loop over tombstones.
    fn take_top(&mut self) -> Option<HeapEntry<E>> {
        let entry = self.heap.pop()?;
        let state = self.slots[entry.slot as usize].state;
        self.release(entry.slot);
        match state {
            SlotState::Pending => {
                self.live -= 1;
                Some(entry)
            }
            SlotState::Cancelled => None,
            SlotState::Free => unreachable!("a heap entry always owns its slot"),
        }
    }

    fn popped(entry: HeapEntry<E>) -> Popped<E> {
        Popped {
            time: entry.time,
            priority: entry.priority,
            id: EventId::new(entry.slot, entry.gen),
            target: entry.target,
            payload: entry.payload,
        }
    }

    /// Remove and return the next event to fire, skipping cancelled entries.
    ///
    /// The heap invariant (pops are non-decreasing in
    /// `(time, priority, seq)`) is proved for small schedules by the Kani
    /// harnesses in `tpt-des-verify`.
    pub fn pop(&mut self) -> Option<Popped<E>> {
        while !self.heap.is_empty() {
            if let Some(live) = self.take_top() {
                return Some(Self::popped(live));
            }
        }
        None
    }

    /// Time of the next live (non-cancelled) event, without consuming it.
    ///
    /// Discards tombstones that have surfaced at the top of the heap (and
    /// frees their slots, keeping [`live_len`](Self::live_len) consistent);
    /// amortized `O(log n)`.
    pub fn next_time(&mut self) -> Option<f64> {
        loop {
            let top = self.heap.peek()?;
            if self.slots[top.slot as usize].state == SlotState::Cancelled {
                let _ = self.take_top();
                continue;
            }
            return Some(top.time);
        }
    }

    /// Like [`pop`](Self::pop), but never returns events scheduled after
    /// `max_time` (raw units): the loop stops there, leaving the entry in the
    /// heap. Used by `Simulation::run_until` to bound a run in time.
    pub fn pop_until(&mut self, max_time: f64) -> Option<Popped<E>> {
        loop {
            let top = self.heap.peek()?;
            if top.time > max_time {
                return None;
            }
            if let Some(live) = self.take_top() {
                return Some(Self::popped(live));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Ticks;
    use alloc::vec::Vec;

    fn cal() -> EventCalendar<Ticks, u64> {
        EventCalendar::new()
    }

    const A: EntityId = EntityId(0);

    #[test]
    fn next_time_after_cancel_keeps_live_len_consistent() {
        let mut c = cal();
        let a = c.schedule_at::<0>(Time::from_raw(1.0), A, 1);
        c.schedule_at::<0>(Time::from_raw(2.0), A, 2);
        assert!(c.cancel(a));
        assert_eq!(c.next_time(), Some(2.0));
        assert_eq!(c.live_len(), 1); // used to underflow after next_time
        assert_eq!(c.pop().unwrap().payload, 2);
        assert!(c.is_empty());
        assert_eq!(c.next_time(), None);
    }

    #[test]
    fn pop_until_respects_horizon_without_losing_events() {
        let mut c = cal();
        c.schedule_at::<0>(Time::from_raw(1.0), A, 1);
        c.schedule_at::<0>(Time::from_raw(5.0), A, 5);
        assert_eq!(c.pop_until(2.0).unwrap().payload, 1);
        assert!(c.pop_until(2.0).is_none());
        assert_eq!(c.live_len(), 1);
        assert_eq!(c.pop_until(9.0).unwrap().payload, 5);
    }

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
    fn stale_handle_cannot_cancel_a_reused_slot() {
        let mut c = cal();
        let old = c.schedule_at::<0>(Time::from_raw(1.0), A, 1);
        assert_eq!(c.pop().unwrap().payload, 1); // frees the slot
        let new = c.schedule_at::<0>(Time::from_raw(2.0), A, 2); // reuses it
        assert_ne!(old, new, "reused slot must issue a new generation");
        assert!(!c.cancel(old), "stale handle must not cancel the new event");
        assert_eq!(c.live_len(), 1);
        assert_eq!(c.pop().unwrap().payload, 2);
    }

    #[test]
    fn tombstone_slot_is_not_reused_before_it_surfaces() {
        let mut c = cal();
        let doomed = c.schedule_at::<0>(Time::from_raw(5.0), A, 5);
        assert!(c.cancel(doomed));
        // The tombstone still owns its slot, so this must get a new one.
        let other = c.schedule_at::<0>(Time::from_raw(1.0), A, 1);
        assert_ne!(doomed.slot(), other.slot());
        assert_eq!(c.pop().unwrap().payload, 1);
        assert!(c.pop().is_none());
    }

    #[test]
    fn live_len_accounts_for_lazy_tombstones() {
        let mut c = cal();
        let ids: Vec<EventId> = (0..10)
            .map(|t| c.schedule_at::<0>(Time::from_raw(t as f64), A, t))
            .collect();
        assert_eq!(c.live_len(), 10);
        // Cancel the first five scheduled — they fire at times 0..5.
        for id in &ids[..5] {
            assert!(c.cancel(*id));
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
