//! The blocking operation futures: [`Delay`], [`Acquire`], [`WaitUntil`].
//!
//! Each operation checks the process interrupt flag first: an interrupted
//! pending operation resolves with [`Interrupt`], its registration is
//! cancelled, and the flag is consumed — the coroutine continues from the
//! await point and decides whether to recover or abort.

use crate::system::{ProcessId, ProcessOutput, Sim};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use tpt_des_core::{Seconds, Time};

/// Raised in a process's pending operation when it is interrupted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Interrupt;

impl Interrupt {
    /// The message ("process interrupted").
    pub fn message(&self) -> &'static str {
        "process interrupted"
    }
}

impl core::fmt::Display for Interrupt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Interrupt {}

/// Suspend until a duration elapses.
pub struct Delay {
    sim: Sim,
    pid: ProcessId,
    deadline: f64,
    registered: bool,
}

impl core::fmt::Debug for Delay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Delay")
            .field("pid", &self.pid)
            .field("deadline", &self.deadline)
            .field("registered", &self.registered)
            .finish()
    }
}

impl Delay {
    pub(crate) fn new(sim: Sim, pid: ProcessId, duration: f64) -> Self {
        let deadline = sim.now() + duration;
        Self {
            sim,
            pid,
            deadline,
            registered: false,
        }
    }
}

impl Future for Delay {
    type Output = ProcessOutput;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut shared = this.sim.shared.borrow_mut();
        // An interrupt raised before this poll is delivered here and the
        // flag is consumed (SimPy semantics: recover or abort).
        if shared.interrupted.remove(&this.pid) {
            if let Some(event) = shared.pending_timers.remove(&this.pid) {
                shared.timers.cancel(event);
            }
            return Poll::Ready(Err(Interrupt));
        }
        if shared.now >= this.deadline {
            shared.pending_timers.remove(&this.pid);
            return Poll::Ready(Ok(()));
        }
        if !this.registered {
            this.registered = true;
            let event = shared.timers.schedule_at::<0>(
                Time::<Seconds>::from_raw(this.deadline),
                tpt_des_core::EntityId(this.pid.0 as usize),
                this.pid,
            );
            shared.pending_timers.insert(this.pid, event);
        }
        Poll::Pending
    }
}

/// Acquire units of a resource (all-or-nothing, FIFO waiters).
pub struct Acquire {
    sim: Sim,
    pid: ProcessId,
    resource: u64,
    amount: usize,
    queued: bool,
}

impl core::fmt::Debug for Acquire {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Acquire")
            .field("pid", &self.pid)
            .field("resource", &self.resource)
            .field("amount", &self.amount)
            .field("queued", &self.queued)
            .finish()
    }
}

impl Acquire {
    pub(crate) fn new(sim: Sim, pid: ProcessId, resource: u64, amount: usize) -> Self {
        Self {
            sim,
            pid,
            resource,
            amount,
            queued: false,
        }
    }
}

impl Future for Acquire {
    type Output = ProcessOutput;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut shared = this.sim.shared.borrow_mut();
        if shared.interrupted.remove(&this.pid) {
            if let Some(queue) = shared.resource_waiters.get_mut(&this.resource) {
                queue.retain(|&(pid, _)| pid != this.pid);
            }
            return Poll::Ready(Err(Interrupt));
        }

        let available = shared.resources.get(&this.resource).copied().unwrap_or(0);
        // FIFO fairness: a queued requester may only take units when it is
        // at the front of its resource's queue (or never queued).
        let front_pid = shared
            .resource_waiters
            .get(&this.resource)
            .and_then(|q| q.front().map(|&(pid, _)| pid));
        let may_take = if this.queued {
            front_pid == Some(this.pid)
        } else {
            front_pid.is_none()
        };

        if may_take && available >= this.amount {
            if this.queued {
                shared
                    .resource_waiters
                    .get_mut(&this.resource)
                    .expect("queued flag implies queue")
                    .pop_front();
                this.queued = false;
            }
            *shared.resources.get_mut(&this.resource).expect("known") -= this.amount;
            return Poll::Ready(Ok(()));
        }
        if !this.queued {
            this.queued = true;
            shared
                .resource_waiters
                .entry(this.resource)
                .or_default()
                .push_back((this.pid, this.amount));
        }
        Poll::Pending
    }
}

/// Suspend until a predicate over the simulation state becomes true.
///
/// The predicate is owned by the condition registry and re-checked on every
/// [`Sim::notify_all`](crate::Sim::notify_all); there is no polling loop.
/// When the registry removes a satisfied condition, the next poll of this
/// future resolves `Ok(())`.
pub struct WaitUntil {
    sim: Sim,
    pid: ProcessId,
    condition_id: Option<u64>,
}

impl core::fmt::Debug for WaitUntil {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WaitUntil")
            .field("pid", &self.pid)
            .field("condition_id", &self.condition_id)
            .finish()
    }
}

impl WaitUntil {
    pub(crate) fn new<F>(sim: Sim, pid: ProcessId, predicate: F) -> Self
    where
        F: Fn(&Sim) -> bool + 'static,
    {
        let id = {
            let mut shared = sim.shared.borrow_mut();
            let id = shared.next_condition;
            shared.next_condition += 1;
            shared.conditions.push(crate::system::Condition {
                id,
                predicate: Box::new(predicate),
                procs: alloc::vec![pid],
            });
            id
        };
        Self {
            sim,
            pid,
            condition_id: Some(id),
        }
    }
}

impl Future for WaitUntil {
    type Output = ProcessOutput;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut shared = this.sim.shared.borrow_mut();
        if shared.interrupted.remove(&this.pid) {
            let pid = this.pid;
            shared
                .conditions
                .retain(|condition| !condition.procs.contains(&pid));
            return Poll::Ready(Err(Interrupt));
        }
        match this.condition_id.take() {
            Some(id) => {
                // Condition still registered and unsatisfied → keep waiting.
                match shared.conditions.iter().position(|c| c.id == id) {
                    Some(_) => {
                        this.condition_id = Some(id);
                        Poll::Pending
                    }
                    // Removed by notify_all: the predicate held.
                    None => Poll::Ready(Ok(())),
                }
            }
            None => Poll::Ready(Ok(())),
        }
    }
}
