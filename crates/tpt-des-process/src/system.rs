//! The process executor: async coroutines scheduled by the simulation
//! clock.
//!
//! Architecture: all state lives behind one `Rc<RefCell<Shared>>`. The
//! executor never holds the lock while polling a future — it takes the
//! pinned future out, polls it, and puts it back — so process bodies (and
//! the operation futures in [`ops`]) can freely lock the state during
//! their own polls without re-entrancy.

use crate::ops::{Acquire, Delay, WaitUntil};
use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::rc::Rc;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Waker};
use std::sync::Arc;
use tpt_math_prob_core::Rng as _;

pub use crate::ops::Interrupt;

/// Identifies a spawned process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u64);

/// What a process body returns.
pub type ProcessOutput = Result<(), Interrupt>;

/// The async context handed to process bodies.
///
/// Cloned freely; every method takes the internal lock only for the
/// duration of the call, so never hold a borrow across an `await`.
#[derive(Clone)]
pub struct Sim {
    pub(crate) shared: Rc<RefCell<Shared>>,
}

impl core::fmt::Debug for Sim {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let shared = self.shared.borrow();
        f.debug_struct("Sim")
            .field("now", &shared.now)
            .field("processes", &shared.procs.len())
            .finish()
    }
}

impl Sim {
    /// Current simulation time.
    pub fn now(&self) -> f64 {
        self.shared.borrow().now
    }

    /// Move the simulation clock forward to `t` (never backwards).
    ///
    /// Hybrid bridges call this to keep the discrete clock synchronized
    /// with continuous integration time.
    pub fn set_time(&self, t: f64) {
        let mut shared = self.shared.borrow_mut();
        shared.now = shared.now.max(t);
    }

    /// Next uniform `f64` in `[0, 1)` from the seeded simulation RNG.
    pub fn uniform(&self) -> f64 {
        self.shared.borrow_mut().rng.next_f64()
    }

    /// Spawn a process from an async closure receiving this handle.
    pub fn spawn<F, Fut>(&self, body: F) -> ProcessId
    where
        F: FnOnce(Sim) -> Fut,
        Fut: Future<Output = ProcessOutput> + 'static,
    {
        self.spawn_future(body(self.clone()))
    }

    /// Spawn an already-built future as a process.
    pub fn spawn_future<Fut>(&self, body: Fut) -> ProcessId
    where
        Fut: Future<Output = ProcessOutput> + 'static,
    {
        let mut shared = self.shared.borrow_mut();
        let pid = ProcessId(shared.next_pid);
        shared.next_pid += 1;
        shared.procs.insert(pid, ProcEntry::new(Box::pin(body)));
        shared.ready.push_back(pid);
        pid
    }

    /// Suspend the calling process for `duration` seconds.
    ///
    /// Resolves `Err(Interrupt)` if the process is interrupted first.
    pub fn delay(&self, duration: f64) -> Delay {
        Delay::new(self.clone(), self.current_pid(), duration)
    }

    /// Acquire `amount` units of `resource` (all-or-nothing, FIFO waiters).
    pub fn acquire(&self, resource: u64, amount: usize) -> Acquire {
        Acquire::new(self.clone(), self.current_pid(), resource, amount)
    }

    /// Return `amount` units to `resource`; wakes FIFO waiters that now fit.
    pub fn release(&self, resource: u64, amount: usize) {
        let woken: Vec<ProcessId> = {
            let mut shared = self.shared.borrow_mut();
            *shared.resources.entry(resource).or_insert(0) += amount;
            let mut woken = Vec::new();
            loop {
                let available = shared.resources.get(&resource).copied().unwrap_or(0);
                let front = shared
                    .resource_waiters
                    .get(&resource)
                    .and_then(|q| q.front().copied());
                match front {
                    Some((pid, units)) if units <= available => {
                        shared
                            .resource_waiters
                            .get_mut(&resource)
                            .expect("front existed")
                            .pop_front();
                        *shared.resources.get_mut(&resource).expect("checked") -= units;
                        shared.ready.push_back(pid);
                        woken.push(pid);
                    }
                    _ => break,
                }
            }
            woken
        };
        let _ = woken;
    }

    /// Units currently free of `resource`.
    pub fn available(&self, resource: u64) -> usize {
        self.shared
            .borrow()
            .resources
            .get(&resource)
            .copied()
            .unwrap_or(0)
    }

    /// Suspend until `predicate` returns true; re-checked on every
    /// [`notify_all`](Sim::notify_all).
    pub fn wait_until<F>(&self, predicate: F) -> WaitUntil
    where
        F: Fn(&Sim) -> bool + 'static,
    {
        WaitUntil::new(self.clone(), self.current_pid(), predicate)
    }

    /// Re-check every registered `wait_until` predicate and wake the
    /// processes whose predicates now hold. Call after any state change a
    /// condition might depend on.
    pub fn notify_all(&self) {
        let woken: Vec<ProcessId> = {
            let mut shared = self.shared.borrow_mut();
            let sim = Sim {
                shared: self.shared.clone(),
            };
            let mut woken = Vec::new();
            let mut i = 0;
            while i < shared.conditions.len() {
                let satisfied = (shared.conditions[i].predicate)(&sim);
                if satisfied {
                    let condition = shared.conditions.remove(i);
                    woken.extend(condition.procs);
                } else {
                    i += 1;
                }
            }
            woken
        };
        let mut shared = self.shared.borrow_mut();
        shared.ready.extend(woken);
    }

    /// Interrupt `target`: its pending operation resolves with
    /// [`Interrupt`]. The coroutine body decides to recover (continue
    /// awaiting) or abort (return the error). No-op if already finished.
    ///
    /// A process may interrupt itself while running.
    pub fn interrupt(&self, target: ProcessId) {
        let mut shared = self.shared.borrow_mut();
        if shared.procs.get(&target).is_none_or(|e| e.finished) {
            return;
        }
        shared.interrupted.insert(target);
        cancel_pending(&mut shared, target);
        shared.ready.push_back(target);
    }

    /// True if the process finished (normally or by returning an error).
    pub fn is_finished(&self, pid: ProcessId) -> bool {
        let shared = self.shared.borrow();
        shared.procs.get(&pid).is_none_or(|e| e.finished)
    }

    /// How many processes are registered (finished ones included).
    pub fn process_count(&self) -> usize {
        self.shared.borrow().procs.len()
    }

    fn current_pid(&self) -> ProcessId {
        let shared = self.shared.borrow();
        shared
            .polling
            .expect("blocking operations may only be awaited inside a spawned process")
    }
}

pub(crate) struct ProcEntry {
    pub(crate) future: Pin<Box<dyn Future<Output = ProcessOutput>>>,
    pub(crate) finished: bool,
}

impl ProcEntry {
    fn new(future: Pin<Box<dyn Future<Output = ProcessOutput>>>) -> Self {
        Self {
            future,
            finished: false,
        }
    }
}

pub(crate) struct Condition {
    pub(crate) id: u64,
    pub(crate) predicate: Box<dyn Fn(&Sim) -> bool>,
    pub(crate) procs: Vec<ProcessId>,
}

/// Mutable simulation state, behind one mutex.
pub(crate) struct Shared {
    pub(crate) now: f64,
    pub(crate) rng: tpt_des_core::SimRng,
    pub(crate) procs: BTreeMap<ProcessId, ProcEntry>,
    pub(crate) ready: VecDeque<ProcessId>,
    pub(crate) resources: BTreeMap<u64, usize>,
    pub(crate) resource_waiters: BTreeMap<u64, VecDeque<(ProcessId, usize)>>,
    pub(crate) conditions: Vec<Condition>,
    pub(crate) timers: tpt_des_core::EventCalendar<tpt_des_core::Seconds, ProcessId>,
    pub(crate) next_pid: u64,
    pub(crate) next_condition: u64,
    /// Processes with a pending interrupt (SimPy-style flag: delivered at
    /// the next poll of a pending operation). Kept outside `ProcEntry`
    /// because the executor removes entries from `procs` while polling.
    pub(crate) interrupted: BTreeSet<ProcessId>,
    /// Live delay timers per process, for cancellation.
    pub(crate) pending_timers: BTreeMap<ProcessId, tpt_des_core::EventId>,
    /// The process currently being polled (context for `current_pid`).
    pub(crate) polling: Option<ProcessId>,
}

/// Cancel every pending registration of `pid` (timers, resource queues,
/// condition lists).
pub(crate) fn cancel_pending(shared: &mut Shared, pid: ProcessId) {
    if let Some(event) = shared.pending_timers.remove(&pid) {
        shared.timers.cancel(event);
    }
    for queue in shared.resource_waiters.values_mut() {
        queue.retain(|&(w, _)| w != pid);
    }
    for condition in &mut shared.conditions {
        condition.procs.retain(|&p| p != pid);
    }
}

/// The simulation engine: spawn async processes and run the event loop.
#[derive(Clone, Debug)]
pub struct ProcessSystem {
    sim: Sim,
}

impl Default for ProcessSystem {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ProcessSystem {
    /// A system with the given RNG seed.
    pub fn new(seed: u64) -> Self {
        let shared = Shared {
            now: 0.0,
            rng: tpt_des_core::SimRng::new(seed),
            procs: BTreeMap::new(),
            ready: VecDeque::new(),
            resources: BTreeMap::new(),
            resource_waiters: BTreeMap::new(),
            conditions: Vec::new(),
            timers: tpt_des_core::EventCalendar::new(),
            next_pid: 0,
            next_condition: 0,
            interrupted: BTreeSet::new(),
            pending_timers: BTreeMap::new(),
            polling: None,
        };
        Self {
            sim: Sim {
                shared: Rc::new(RefCell::new(shared)),
            },
        }
    }

    /// The per-process handle (captured by spawned bodies).
    pub fn handle(&self) -> Sim {
        self.sim.clone()
    }

    /// Current simulation time.
    pub fn now(&self) -> f64 {
        self.sim.now()
    }

    /// Move the simulation clock forward (hybrid bridges).
    pub fn set_time(&self, t: f64) {
        self.sim.set_time(t)
    }

    /// Spawn a process from an async closure receiving the handle.
    pub fn spawn<F, Fut>(&self, body: F) -> ProcessId
    where
        F: FnOnce(Sim) -> Fut,
        Fut: Future<Output = ProcessOutput> + 'static,
    {
        self.sim.spawn(body)
    }

    /// Interrupt a process.
    pub fn interrupt(&self, target: ProcessId) {
        self.sim.interrupt(target)
    }

    /// True if the process finished.
    pub fn is_finished(&self, pid: ProcessId) -> bool {
        self.sim.is_finished(pid)
    }

    /// Register a resource with initial free units.
    pub fn add_resource(&self, resource: u64, units: usize) {
        self.sim
            .shared
            .borrow_mut()
            .resources
            .insert(resource, units);
    }

    /// Run until every process finished, or events are exhausted (which
    /// includes deadlocked condition waits).
    pub fn run(&mut self) {
        self.run_until(f64::INFINITY);
    }

    /// Run until `horizon`, or until events are exhausted.
    pub fn run_until(&mut self, horizon: f64) {
        enum Action {
            Poll(ProcessId),
            Pump,
            Stop,
        }
        loop {
            // Fire all timers up to the horizon into the ready queue.
            self.pump(horizon);
            let action = {
                let mut shared = self.sim.shared.borrow_mut();
                if let Some(&pid) = shared.ready.front() {
                    Action::Poll(pid)
                } else {
                    match shared.timers.next_time() {
                        Some(t) if t <= horizon => Action::Pump,
                        _ => Action::Stop,
                    }
                }
            };
            match action {
                Action::Poll(pid) => {
                    let popped = {
                        let mut shared = self.sim.shared.borrow_mut();
                        shared.ready.pop_front()
                    };
                    if popped == Some(pid) {
                        self.poll_one(pid);
                    }
                }
                Action::Pump => continue,
                Action::Stop => break,
            }
        }
    }

    fn pump(&self, horizon: f64) {
        let mut shared = self.sim.shared.borrow_mut();
        while let Some(popped) = shared.timers.pop_until(horizon) {
            shared.now = shared.now.max(popped.time);
            let pid = popped.payload;
            shared.pending_timers.remove(&pid);
            shared.ready.push_back(pid);
        }
    }

    fn poll_one(&self, pid: ProcessId) {
        // Take the future out so bodies can re-enter the lock freely.
        let mut entry = {
            let mut shared = self.sim.shared.borrow_mut();
            shared.polling = Some(pid);
            shared.procs.remove(&pid)
        };
        let Some(mut entry) = entry.take() else {
            let mut shared = self.sim.shared.borrow_mut();
            shared.polling = None;
            return;
        };
        if !entry.finished {
            let waker = noop_waker();
            let mut cx = Context::from_waker(&waker);
            if entry.future.as_mut().poll(&mut cx).is_ready() {
                entry.finished = true;
            }
        }

        let mut shared = self.sim.shared.borrow_mut();
        shared.polling = None;
        shared.procs.insert(pid, entry);
    }
}

struct NoopWake;

impl std::task::Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn noop_waker() -> Waker {
    Waker::from(Arc::new(NoopWake))
}
