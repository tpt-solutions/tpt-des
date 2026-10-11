//! Discrete-event simulation of single-station queues.
//!
//! [`SimQueue`] is a `tpt-des-core` entity combining an arrival source, an
//! unbounded (or bounded) waiting line, and a server; [`run_queue`] is the
//! one-call driver. Inter-arrival and service times come from any
//! [`RateSampler`] — [`Exponential`] (backed by `tpt-math-prob-dist`) or a
//! user closure — so M/M/1, M/D/1, M/G/1-style models all run through the
//! same engine, deterministically given a seed.
//!
//! Four queueing disciplines are supported: [`Discipline::Fifo`],
//! [`Discipline::Lifo`], [`Discipline::Priority`], and
//! [`Discipline::ProcessorSharing`] (work-conserving, speed-1 processor:
//! a job with remaining work `w` among `n` in service completes after
//! `w·n` further seconds).

use crate::error::QueueError;
use core::fmt;
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap, VecDeque};
use tpt_des_core::{Ctx, Entity, EntityId, Seconds, SimRng, Simulation, Time};
use tpt_math_prob_core::Distribution as _;
use tpt_math_prob_dist::{exp, Dist};

/// A source of i.i.d. durations (inter-arrival or service times).
pub trait RateSampler: fmt::Debug {
    /// Draw the next duration.
    fn draw(&mut self, rng: &mut SimRng) -> Time<Seconds>;
}

/// Exponential durations at the given rate — the Markovian case.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exponential {
    rate: f64,
}

impl Exponential {
    /// An exponential sampler with rate `rate > 0` (mean `1/rate`).
    pub fn new(rate: f64) -> Result<Self, QueueError> {
        if rate <= 0.0 || !rate.is_finite() {
            return Err(QueueError::InvalidParameter);
        }
        Ok(Self { rate })
    }

    /// The rate parameter.
    pub fn rate(&self) -> f64 {
        self.rate
    }
}

impl RateSampler for Exponential {
    fn draw(&mut self, rng: &mut SimRng) -> Time<Seconds> {
        // `Dist` adapts rand_distr's `Exp` to the tpt-math `Rng` trait.
        Time::from_raw(
            Dist::new(exp(self.rate).expect("rate validated at construction"))
                .sample(rng.core_mut()),
        )
    }
}

/// A user-supplied duration generator, as stored by [`FromClosure`].
pub type DurationFn = Box<dyn Fn(&mut SimRng) -> Time<Seconds> + Send>;

/// Durations from a user-supplied closure (arbitrary distributions).
pub struct FromClosure {
    name: &'static str,
    f: DurationFn,
}

impl fmt::Debug for FromClosure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromClosure")
            .field("name", &self.name)
            .finish()
    }
}

impl FromClosure {
    /// Wrap a deterministic-duration generator. `name` appears in Debug output.
    pub fn new(
        name: &'static str,
        f: impl Fn(&mut SimRng) -> Time<Seconds> + Send + 'static,
    ) -> Self {
        Self {
            name,
            f: Box::new(f),
        }
    }
}

impl RateSampler for FromClosure {
    fn draw(&mut self, rng: &mut SimRng) -> Time<Seconds> {
        (self.f)(rng)
    }
}

/// The queueing discipline: how the next job to serve is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Discipline {
    /// First in, first out.
    #[default]
    Fifo,
    /// Last in, first out.
    Lifo,
    /// Lowest priority number first, FIFO among equals.
    Priority,
    /// Work-conserving processor sharing (all waiting jobs served
    /// simultaneously at equal rate).
    ProcessorSharing,
}

/// Events understood by [`SimQueue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueEvent {
    /// A job arrives (job ids increase from 0).
    Arrival(u64),
    /// A job finishes service.
    Departure(u64),
}

/// Running measurements for one queue.
///
/// Integrals (`area_*`) are time-weighted, so `time_avg_number` and
/// `utilization` are exact for the simulated window.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct QueueMetrics {
    /// Total arrivals accepted (blocked arrivals excluded).
    pub arrived: u64,
    /// Arrivals rejected because the queue was at capacity.
    pub blocked: u64,
    /// Jobs that finished service.
    pub completed: u64,
    /// Sum of queue-wait times (arrival → first service).
    pub total_wait: Time<Seconds>,
    /// Sum of sojourn times (arrival → departure).
    pub total_sojourn: Time<Seconds>,
    /// Time-integral of the number of jobs in the system.
    pub area_number: f64,
    /// Time-integral of the number of jobs in service.
    pub area_busy: f64,
    /// Absolute time of the last observation.
    pub last_t: Time<Seconds>,
    /// Window start for the current accumulation ([`SimQueue::reset_metrics`]).
    pub reset_at: Time<Seconds>,
    /// Per-priority-class wait totals, keyed by job priority (lower number =
    /// more urgent). Every job lands in class `0` unless
    /// [`QueueConfig::priority`] is set.
    pub waits_by_priority: BTreeMap<i64, ClassWait>,
    /// Number of parallel servers the metrics were gathered with. Used to turn
    /// busy-server time into a per-server [`utilization`](Self::utilization).
    pub servers: usize,
}

/// Wait-time totals for one priority class.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct ClassWait {
    /// Jobs of this class that started service (post warm-up).
    pub started: u64,
    /// Sum of their queue-wait times.
    pub total_wait: Time<Seconds>,
}

impl QueueMetrics {
    /// Mean queue-wait of jobs in `priority`'s class, or `None` if no job of
    /// that class started service in the measured window.
    pub fn mean_wait_for_priority(&self, priority: i64) -> Option<Time<Seconds>> {
        self.waits_by_priority
            .get(&priority)
            .filter(|c| c.started > 0)
            .map(|c| c.total_wait / c.started as f64)
    }

    /// Mean queue-wait over completed jobs (0 if none completed).
    pub fn mean_wait(&self) -> Time<Seconds> {
        if self.completed == 0 {
            Time::ZERO
        } else {
            self.total_wait / self.completed as f64
        }
    }

    /// Mean sojourn time over completed jobs.
    pub fn mean_sojourn(&self) -> Time<Seconds> {
        if self.completed == 0 {
            Time::ZERO
        } else {
            self.total_sojourn / self.completed as f64
        }
    }

    /// Time-average number of jobs in the system.
    pub fn time_avg_number(&self) -> f64 {
        let span = (self.last_t - self.reset_at).raw();
        if span <= 0.0 {
            0.0
        } else {
            self.area_number / span
        }
    }

    /// Fraction of time the server had at least one job (or, under
    /// processor sharing, the mean number in service).
    pub fn utilization(&self) -> f64 {
        let span = (self.last_t - self.reset_at).raw();
        if span <= 0.0 {
            0.0
        } else {
            self.area_busy / (span * self.servers.max(1) as f64)
        }
    }
}

/// One job in the system.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Job {
    id: u64,
    arrived_at: f64,
    priority: i64,
    /// Time service began: NaN while waiting, equal to `arrived_at` under
    /// processor sharing (service starts on arrival).
    started_at: f64,
}

/// The life of one completed job, for trace export.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct JobRecord {
    /// Job number, in arrival order from 0.
    pub job: u64,
    /// Priority class (see [`QueueConfig::priority`]).
    pub priority: i64,
    /// Arrival time.
    pub arrived_at: Time<Seconds>,
    /// Time service began.
    pub started_at: Time<Seconds>,
    /// Departure time.
    pub departed_at: Time<Seconds>,
}

impl JobRecord {
    /// Time spent waiting before service.
    pub fn wait(&self) -> Time<Seconds> {
        self.started_at - self.arrived_at
    }

    /// Time from arrival to departure.
    pub fn sojourn(&self) -> Time<Seconds> {
        self.departed_at - self.arrived_at
    }
}

/// A job in processor sharing. `finish` is the value of the virtual-time
/// clock at which the job completes: each job in service accrues `1/n` of
/// virtual time per unit of real time, so `finish − virtual_time` is its
/// remaining work and the job with the smallest `finish` completes first.
struct PsEntry {
    finish: f64,
    job: Job,
}

impl PartialEq for PsEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for PsEntry {}

impl PartialOrd for PsEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PsEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.finish
            .total_cmp(&other.finish)
            .then(self.job.id.cmp(&other.job.id))
    }
}

/// Configuration for a simulated queue.
pub struct QueueConfig {
    /// Inter-arrival time sampler.
    pub arrival: Box<dyn RateSampler>,
    /// Service time sampler.
    pub service: Box<dyn RateSampler>,
    /// Queueing discipline.
    pub discipline: Discipline,
    /// Maximum number of jobs in the system (waiting + in service);
    /// `None` for an infinite buffer. Blocked arrivals are counted, not queued.
    pub capacity: Option<usize>,
    /// Priority class of each arriving job (lower number = more urgent),
    /// drawn from the simulation RNG. `None` puts every job in class `0`,
    /// which makes [`Discipline::Priority`] behave as FIFO. Only
    /// [`Discipline::Priority`] reads it.
    pub priority: Option<PriorityFn>,
}

/// A per-arrival priority-class draw. See [`QueueConfig::priority`].
pub type PriorityFn = Box<dyn FnMut(&mut SimRng) -> i64>;

impl fmt::Debug for QueueConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueConfig")
            .field("arrival", &self.arrival)
            .field("service", &self.service)
            .field("discipline", &self.discipline)
            .field("capacity", &self.capacity)
            .field("priority", &self.priority.as_ref().map(|_| "<fn>"))
            .finish()
    }
}

/// Jobs waiting for the server. FIFO and LIFO use a deque; priority uses a
/// binary heap, so each dispatch is O(log n) rather than a scan of the line.
#[derive(Default)]
struct WaitingLine {
    deque: VecDeque<Job>,
    by_priority: BinaryHeap<Urgent>,
}

impl WaitingLine {
    fn len(&self) -> usize {
        self.deque.len() + self.by_priority.len()
    }

    fn push(&mut self, job: Job, discipline: Discipline) {
        match discipline {
            Discipline::Priority => self.by_priority.push(Urgent(job)),
            _ => self.deque.push_back(job),
        }
    }

    fn take(&mut self, discipline: Discipline) -> Option<Job> {
        match discipline {
            Discipline::Fifo => self.deque.pop_front(),
            Discipline::Lifo => self.deque.pop_back(),
            Discipline::Priority => self.by_priority.pop().map(|u| u.0),
            Discipline::ProcessorSharing => None, // handled separately
        }
    }
}

/// Heap entry ordered so the most urgent job is the greatest: lowest priority
/// number first, then lowest job id, which is arrival order.
struct Urgent(Job);

impl Urgent {
    fn key(&self) -> (i64, u64) {
        (self.0.priority, self.0.id)
    }
}

impl PartialEq for Urgent {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for Urgent {}

impl PartialOrd for Urgent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Urgent {
    fn cmp(&self, other: &Self) -> Ordering {
        let (p, id) = self.key();
        let (op, oid) = other.key();
        op.cmp(&p).then(oid.cmp(&id))
    }
}

/// A simulated single-station queue, usable as a `tpt-des-core` entity.
pub struct SimQueue {
    id: EntityId,
    config: QueueConfig,
    /// Warm-up cutoff: jobs arriving before this time are excluded from the
    /// wait/sojourn accumulators, and the time integrals restart here.
    warmup: f64,
    servers: usize,
    /// Completed-job records, when tracing is enabled.
    trace: Option<Vec<JobRecord>>,
    waiting: WaitingLine,
    /// Jobs in service under FIFO, LIFO, or priority (at most `servers`).
    in_service: Vec<Job>,
    /// Jobs in service under processor sharing, earliest finish first.
    ps_jobs: BinaryHeap<Reverse<PsEntry>>,
    /// Attained service per job in service, as processor sharing advances.
    virtual_time: f64,
    next_job: u64,
    /// Departure event currently scheduled (processor sharing reschedules it
    /// whenever the population changes).
    pending_departure: Option<tpt_des_core::EventId>,
    /// Population between the last two events (processor sharing).
    last_population: usize,
    last_update: f64,
    metrics: QueueMetrics,
}

impl fmt::Debug for SimQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimQueue")
            .field("id", &self.id)
            .field("waiting", &self.waiting.len())
            .field("in_service", &self.in_service.len())
            .field("metrics", &self.metrics)
            .finish()
    }
}

impl SimQueue {
    /// Create a queue entity that must be registered as `id`.
    pub fn new(id: EntityId, config: QueueConfig) -> Self {
        Self::with_warmup(id, config, Time::ZERO)
    }

    /// Create a queue entity with a warm-up cutoff at `warmup` (absolute
    /// time): arrivals before it are excluded from the measurements.
    ///
    /// # Panics
    ///
    /// If `warmup` is NaN or negative.
    pub fn with_warmup(id: EntityId, config: QueueConfig, warmup: Time<Seconds>) -> Self {
        Self::with_servers(id, config, warmup, 1)
    }

    /// Create a queue entity with `servers` identical parallel servers and a
    /// warm-up cutoff, as in [`with_warmup`](Self::with_warmup).
    ///
    /// # Panics
    ///
    /// If `warmup` is NaN or negative, if `servers` is zero, or if `servers`
    /// exceeds one under [`Discipline::ProcessorSharing`] (the simulator's
    /// processor sharing is a single speed-1 processor).
    pub fn with_servers(
        id: EntityId,
        config: QueueConfig,
        warmup: Time<Seconds>,
        servers: usize,
    ) -> Self {
        assert!(
            warmup.raw().is_finite() && warmup.raw() >= 0.0,
            "warm-up must be a finite, non-negative time, got {warmup}"
        );
        assert!(servers >= 1, "a queue needs at least one server");
        assert!(
            servers == 1 || config.discipline != Discipline::ProcessorSharing,
            "processor sharing is single-server only"
        );
        Self {
            id,
            config,
            warmup: warmup.raw(),
            servers,
            trace: None,
            waiting: WaitingLine::default(),
            in_service: Vec::new(),
            ps_jobs: BinaryHeap::new(),
            virtual_time: 0.0,
            next_job: 0,
            pending_departure: None,
            last_population: 0,
            last_update: 0.0,
            metrics: QueueMetrics {
                servers,
                ..QueueMetrics::default()
            },
        }
    }

    /// The metrics accumulated so far.
    pub fn metrics(&self) -> &QueueMetrics {
        &self.metrics
    }

    /// Zero the accumulators (for warm-up deletion), keeping the state.
    pub fn reset_metrics(&mut self) {
        let last_t = self.metrics.last_t;
        self.metrics = QueueMetrics {
            last_t,
            reset_at: last_t,
            servers: self.servers,
            ..QueueMetrics::default()
        };
    }

    /// Advance the measurement clock to `now`, applying the warm-up cutoff
    /// on the way. The population integral is taken up to the exact cutoff
    /// (and then discarded), so no interval is lost or mis-attributed.
    fn advance_to(&mut self, now: f64) {
        if self.warmup > 0.0 && now >= self.warmup && self.metrics.reset_at.raw() < self.warmup {
            self.update_integrals(self.warmup);
            self.reset_metrics();
            self.metrics.reset_at = Time::from_raw(self.warmup);
        }
        self.update_integrals(now);
    }

    /// Close the measurement window at `horizon`: integrate the final span
    /// up to it, so `time_avg_number` and `utilization` cover the whole
    /// window. [`run_queue`] calls this after the run.
    ///
    /// # Panics
    ///
    /// If `horizon` is not later than the warm-up cutoff.
    pub fn finalize(&mut self, horizon: Time<Seconds>) {
        assert!(
            horizon.raw() >= self.warmup,
            "horizon {horizon} precedes the warm-up cutoff {}",
            self.warmup
        );
        self.advance_to(horizon.raw());
    }

    /// True if a job that arrived at `t` counts toward the measurements.
    fn counts(&self, arrived_at: f64) -> bool {
        self.warmup <= 0.0 || arrived_at >= self.warmup
    }

    /// Jobs in service, under any discipline.
    fn population(&self) -> usize {
        self.in_service.len() + self.ps_jobs.len()
    }

    fn update_integrals(&mut self, now: f64) {
        let dt = now - self.metrics.last_t.raw();
        if dt > 0.0 {
            let in_service = self.population();
            self.metrics.area_number += (self.waiting.len() + in_service) as f64 * dt;
            // Busy time counts busy servers. Under processor sharing the one
            // server is busy whenever anything is in service, so the count is
            // a 0/1 indicator rather than the job count.
            let busy_servers = match self.config.discipline {
                Discipline::ProcessorSharing => usize::from(in_service > 0),
                _ => self.in_service.len(),
            };
            self.metrics.area_busy += busy_servers as f64 * dt;
        }
        self.metrics.last_t = Time::from_raw(now);
    }

    /// Pick the next job from the waiting line per the discipline.
    fn take_next(&mut self) -> Option<Job> {
        self.waiting.take(self.config.discipline)
    }

    fn schedule_next_arrival(&mut self, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        let delay = checked_sample(self.config.arrival.draw(ctx.rng), "inter-arrival");
        let job = self.next_job;
        ctx.schedule::<0>(Time::from_raw(delay), self.id, QueueEvent::Arrival(job));
        self.next_job += 1;
    }

    /// Processor sharing, phase 1: advance the virtual-time clock to `now`.
    /// Each of the `last_population` jobs in service gains `elapsed / n` of
    /// service, so every job's remaining work shrinks by the same amount.
    /// Must be called *before* any membership change.
    fn ps_advance(&mut self, now: f64) {
        let elapsed = now - self.last_update;
        let n = self.last_population;
        if n > 0 && elapsed > 0.0 {
            self.virtual_time += elapsed / n as f64;
        }
        self.last_update = now;
    }

    /// Processor sharing, phase 2: after a membership change, schedule the
    /// next completion. The earliest finisher has the least remaining work
    /// `finish − virtual_time`, and it completes after that work times the
    /// population.
    fn ps_reschedule(&mut self, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        self.last_population = self.ps_jobs.len();
        if let Some(id) = self.pending_departure.take() {
            ctx.cancel(id);
        }
        if let Some(Reverse(next)) = self.ps_jobs.peek() {
            // Clamp at zero: the minimum-work job may sit a rounding hair
            // below zero at its own completion time.
            let remaining = (next.finish - self.virtual_time).max(0.0);
            let delay = remaining * self.last_population as f64;
            let event = ctx.schedule::<0>(
                Time::from_raw(delay),
                self.id,
                QueueEvent::Departure(next.job.id),
            );
            self.pending_departure = Some(event);
        }
    }
}

impl Entity<Seconds, QueueEvent> for SimQueue {
    fn on_create(&mut self, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        self.metrics.last_t = Time::ZERO;
        self.metrics.reset_at = Time::ZERO;
        self.last_update = 0.0;
        self.schedule_next_arrival(ctx);
    }

    fn on_event(&mut self, event: QueueEvent, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        let now = ctx.now.raw();
        self.advance_to(now);
        match event {
            QueueEvent::Arrival(_) => {
                self.schedule_next_arrival(ctx);

                let in_system = self.waiting.len() + self.population();
                if let Some(cap) = self.config.capacity {
                    if in_system >= cap {
                        self.metrics.blocked += 1;
                        return;
                    }
                }
                self.metrics.arrived += 1;

                let priority = match self.config.priority.as_mut() {
                    Some(draw) => draw(ctx.rng),
                    None => 0,
                };
                match self.config.discipline {
                    Discipline::ProcessorSharing => {
                        // Existing jobs consumed work up to now; the new job
                        // starts fresh from this instant.
                        self.ps_advance(now);
                        let work = checked_sample(self.config.service.draw(ctx.rng), "service");
                        let job = Job {
                            id: event_id_of(&event),
                            arrived_at: now,
                            priority,
                            started_at: now,
                        };
                        self.ps_jobs.push(Reverse(PsEntry {
                            finish: self.virtual_time + work,
                            job,
                        }));
                        self.ps_reschedule(ctx);
                    }
                    _ => {
                        let job = Job {
                            id: event_id_of(&event),
                            arrived_at: now,
                            priority,
                            started_at: f64::NAN,
                        };
                        if self.in_service.len() < self.servers {
                            self.start_service(job, ctx);
                        } else {
                            self.waiting.push(job, self.config.discipline);
                        }
                    }
                }
            }
            QueueEvent::Departure(job_id) => match self.config.discipline {
                Discipline::ProcessorSharing => {
                    self.ps_advance(now);
                    // The scheduled departure is always for the earliest finisher
                    // (earlier schedules were cancelled), so pop that job.
                    if let Some(Reverse(entry)) = self.ps_jobs.pop() {
                        debug_assert_eq!(entry.job.id, job_id, "departure for a non-minimal job");
                        let job = entry.job;
                        if self.counts(job.arrived_at) {
                            self.metrics.completed += 1;
                            self.metrics.total_sojourn += Time::from_raw(now - job.arrived_at);
                        }
                        self.record_departure(job, now);
                    }
                    self.ps_reschedule(ctx);
                }
                _ => {
                    if let Some(pos) = self.in_service.iter().position(|j| j.id == job_id) {
                        let job = self.in_service.remove(pos);
                        if self.counts(job.arrived_at) {
                            self.metrics.completed += 1;
                            self.metrics.total_sojourn += Time::from_raw(now - job.arrived_at);
                        }
                        self.record_departure(job, now);
                    }
                    if let Some(next) = self.take_next() {
                        self.start_service(next, ctx);
                    }
                }
            },
        }
    }
}

impl SimQueue {
    fn start_service(&mut self, job: Job, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        if self.counts(job.arrived_at) {
            let wait = ctx.now.raw() - job.arrived_at;
            self.metrics.total_wait += Time::from_raw(wait);
            let class = self
                .metrics
                .waits_by_priority
                .entry(job.priority)
                .or_default();
            class.started += 1;
            class.total_wait += Time::from_raw(wait);
        }
        let service = checked_sample(self.config.service.draw(ctx.rng), "service");
        let id = job.id;
        self.in_service.push(Job {
            started_at: ctx.now.raw(),
            ..job
        });
        ctx.schedule::<0>(Time::from_raw(service), self.id, QueueEvent::Departure(id));
    }

    /// Append a completed job to the trace, if tracing is on.
    fn record_departure(&mut self, job: Job, departed_at: f64) {
        if let Some(trace) = self.trace.as_mut() {
            trace.push(JobRecord {
                job: job.id,
                priority: job.priority,
                arrived_at: Time::from_raw(job.arrived_at),
                started_at: Time::from_raw(job.started_at),
                departed_at: Time::from_raw(departed_at),
            });
        }
    }
}

/// Reject NaN, infinite, and negative samples from a user distribution:
/// they would corrupt event ordering (in release builds too).
fn checked_sample(value: Time<Seconds>, what: &str) -> f64 {
    // `Time` construction already rejects non-finite values.
    assert!(
        value.raw() >= 0.0,
        "{what} sample must be non-negative, got {}",
        value.raw()
    );
    value.raw()
}

fn event_id_of(event: &QueueEvent) -> u64 {
    match event {
        QueueEvent::Arrival(id) | QueueEvent::Departure(id) => *id,
    }
}

/// Run one queue simulation to `horizon`, deleting the first `warmup` time
/// units from the metrics, and return the post-warm-up measurements.
///
/// Deterministic: the same `seed`, config, and horizons reproduce the same
/// metrics bit-for-bit.
///
/// # Panics
///
/// If `horizon` is not finite and positive, or `warmup` is not in
/// `[0, horizon)`.
///
/// # Examples
///
/// An M/M/1 queue at ρ = 0.5 has mean sojourn `W = 1/(μ−λ) = 2`:
///
/// ```
/// use tpt_des_queue::{run_queue, Discipline, Exponential, QueueConfig};
/// use tpt_des_core::Time;
///
/// let metrics = run_queue(
///     QueueConfig {
///         arrival: Box::new(Exponential::new(0.5).unwrap()),
///         service: Box::new(Exponential::new(1.0).unwrap()),
///         discipline: Discipline::Fifo,
///         capacity: None,
///         priority: None,
///     },
///     Time::from_raw(20_000.0), // horizon
///     Time::from_raw(2_000.0),  // warm-up
///     42,       // seed
/// );
/// assert!((metrics.mean_sojourn().raw() - 2.0).abs() < 0.4);
/// ```
pub fn run_queue(
    config: QueueConfig,
    horizon: Time<Seconds>,
    warmup: Time<Seconds>,
    seed: u64,
) -> QueueMetrics {
    run_queue_multi(config, 1, horizon, warmup, seed)
}

/// Like [`run_queue`], with `servers` identical parallel servers sharing one
/// waiting line (an M/M/c-style station when the config is exponential).
///
/// # Panics
///
/// As [`run_queue`], and if `servers` is zero or exceeds one under processor
/// sharing.
pub fn run_queue_multi(
    config: QueueConfig,
    servers: usize,
    horizon: Time<Seconds>,
    warmup: Time<Seconds>,
    seed: u64,
) -> QueueMetrics {
    run_inner(config, servers, horizon, warmup, seed, false).0
}

/// Like [`run_queue_multi`], and also returns one [`JobRecord`] per job that
/// completed, in departure order. Records include jobs that arrived during
/// warm-up, so filter on `arrived_at` to match the metrics.
///
/// # Panics
///
/// As [`run_queue_multi`].
pub fn run_queue_traced(
    config: QueueConfig,
    servers: usize,
    horizon: Time<Seconds>,
    warmup: Time<Seconds>,
    seed: u64,
) -> (QueueMetrics, Vec<JobRecord>) {
    run_inner(config, servers, horizon, warmup, seed, true)
}

fn run_inner(
    config: QueueConfig,
    servers: usize,
    horizon: Time<Seconds>,
    warmup: Time<Seconds>,
    seed: u64,
    traced: bool,
) -> (QueueMetrics, Vec<JobRecord>) {
    assert!(
        horizon.raw().is_finite() && horizon.raw() > 0.0,
        "horizon must be finite and positive, got {horizon}"
    );
    assert!(
        warmup.raw().is_finite() && (0.0..horizon.raw()).contains(&warmup.raw()),
        "warm-up must lie in [0, horizon), got {warmup} with horizon {horizon}"
    );
    let mut queue = SimQueue::with_servers(EntityId(0), config, warmup, servers);
    if traced {
        queue.trace = Some(Vec::new());
    }
    {
        let mut sim = Simulation::new(seed);
        let _id = sim.add_entity(&mut queue);
        sim.run_until(horizon);
        sim.finalize();
    }
    queue.finalize(horizon);
    let records = queue.trace.take().unwrap_or_default();
    (queue.metrics().clone(), records)
}

/// Render job records as CSV, one row per job, with `wait` and `sojourn`
/// columns computed from the times. Floats use Rust's shortest round-trip form.
pub fn trace_to_csv(records: &[JobRecord]) -> String {
    let mut out = String::from("job,priority,arrived_at,started_at,departed_at,wait,sojourn\n");
    for r in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            r.job,
            r.priority,
            r.arrived_at,
            r.started_at,
            r.departed_at,
            r.wait(),
            r.sojourn()
        ));
    }
    out
}
