//! Discrete-event simulation of single-station queues.
//!
//! [`SimQueue`] is a [`tpt-des-core`] entity combining an arrival source, an
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
use tpt_des_core::{Ctx, Entity, EntityId, Seconds, SimRng, Simulation, Time};
use tpt_math_prob_core::Distribution as _;
use tpt_math_prob_dist::{exp, Dist};

/// A source of i.i.d. durations (inter-arrival or service times).
pub trait RateSampler: fmt::Debug {
    /// Draw the next duration.
    fn draw(&mut self, rng: &mut SimRng) -> f64;
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
    fn draw(&mut self, rng: &mut SimRng) -> f64 {
        // `Dist` adapts rand_distr's `Exp` to the tpt-math `Rng` trait.
        Dist::new(exp(self.rate).expect("rate validated at construction")).sample(rng.core_mut())
    }
}

/// Durations from a user-supplied closure (arbitrary distributions).
pub struct FromClosure {
    name: &'static str,
    f: Box<dyn Fn(&mut SimRng) -> f64 + Send>,
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
    pub fn new(name: &'static str, f: impl Fn(&mut SimRng) -> f64 + Send + 'static) -> Self {
        Self {
            name,
            f: Box::new(f),
        }
    }
}

impl RateSampler for FromClosure {
    fn draw(&mut self, rng: &mut SimRng) -> f64 {
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
pub struct QueueMetrics {
    /// Total arrivals accepted (blocked arrivals excluded).
    pub arrived: u64,
    /// Arrivals rejected because the queue was at capacity.
    pub blocked: u64,
    /// Jobs that finished service.
    pub completed: u64,
    /// Sum of queue-wait times (arrival → first service).
    pub total_wait: f64,
    /// Sum of sojourn times (arrival → departure).
    pub total_sojourn: f64,
    /// Time-integral of the number of jobs in the system.
    pub area_number: f64,
    /// Time-integral of the number of jobs in service.
    pub area_busy: f64,
    /// Absolute time of the last observation.
    pub last_t: f64,
    /// Window start for the current accumulation ([`SimQueue::reset_metrics`]).
    pub reset_at: f64,
}

impl QueueMetrics {
    /// Mean queue-wait over completed jobs (0 if none completed).
    pub fn mean_wait(&self) -> f64 {
        if self.completed == 0 {
            0.0
        } else {
            self.total_wait / self.completed as f64
        }
    }

    /// Mean sojourn time over completed jobs.
    pub fn mean_sojourn(&self) -> f64 {
        if self.completed == 0 {
            0.0
        } else {
            self.total_sojourn / self.completed as f64
        }
    }

    /// Time-average number of jobs in the system.
    pub fn time_avg_number(&self) -> f64 {
        let span = self.last_t - self.reset_at;
        if span <= 0.0 {
            0.0
        } else {
            self.area_number / span
        }
    }

    /// Fraction of time the server had at least one job (or, under
    /// processor sharing, the mean number in service).
    pub fn utilization(&self) -> f64 {
        let span = self.last_t - self.reset_at;
        if span <= 0.0 {
            0.0
        } else {
            self.area_busy / span
        }
    }
}

/// One job in the system.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Job {
    id: u64,
    arrived_at: f64,
    priority: i64,
    /// Remaining work (processor sharing only).
    work: f64,
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
}

impl fmt::Debug for QueueConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueConfig")
            .field("arrival", &self.arrival)
            .field("service", &self.service)
            .field("discipline", &self.discipline)
            .field("capacity", &self.capacity)
            .finish()
    }
}

/// A simulated single-station queue, usable as a `tpt-des-core` entity.
pub struct SimQueue {
    id: EntityId,
    config: QueueConfig,
    /// Warm-up cutoff: jobs arriving before this time are excluded from the
    /// wait/sojourn accumulators, and the time integrals restart here.
    warmup: f64,
    waiting: Vec<Job>,
    in_service: Vec<Job>, // 0..1 jobs, or 0..n under processor sharing
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
        Self::with_warmup(id, config, 0.0)
    }

    /// Create a queue entity with a warm-up cutoff at `warmup` (absolute
    /// time): arrivals before it are excluded from the measurements.
    pub fn with_warmup(id: EntityId, config: QueueConfig, warmup: f64) -> Self {
        Self {
            id,
            config,
            warmup,
            waiting: Vec::new(),
            in_service: Vec::new(),
            next_job: 0,
            pending_departure: None,
            last_population: 0,
            last_update: 0.0,
            metrics: QueueMetrics::default(),
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
            ..QueueMetrics::default()
        };
    }

    /// Apply the warm-up cutoff once, on the first event at/after it.
    fn maybe_apply_warmup(&mut self, now: f64) {
        if self.warmup > 0.0 && now >= self.warmup && self.metrics.reset_at < self.warmup {
            self.reset_metrics();
            self.metrics.reset_at = now;
            self.metrics.last_t = now;
        }
    }

    /// True if a job that arrived at `t` counts toward the measurements.
    fn counts(&self, arrived_at: f64) -> bool {
        self.warmup <= 0.0 || arrived_at >= self.warmup
    }

    fn update_integrals(&mut self, now: f64) {
        let dt = now - self.metrics.last_t;
        if dt > 0.0 {
            let number = self.waiting.len() + self.in_service.len();
            self.metrics.area_number += number as f64 * dt;
            self.metrics.area_busy += self.in_service.len() as f64 * dt;
        }
        self.metrics.last_t = now;
    }

    /// Pick the next job from the waiting line per the discipline.
    fn take_next(&mut self) -> Option<Job> {
        match self.config.discipline {
            Discipline::Fifo => (!self.waiting.is_empty()).then(|| self.waiting.remove(0)),
            Discipline::Lifo => self.waiting.pop(),
            Discipline::Priority => {
                let mut best = 0usize;
                for (i, job) in self.waiting.iter().enumerate() {
                    if job.priority < self.waiting[best].priority
                        || (job.priority == self.waiting[best].priority
                            && job.arrived_at < self.waiting[best].arrived_at)
                    {
                        best = i;
                    }
                }
                (!self.waiting.is_empty()).then(|| self.waiting.remove(best))
            }
            Discipline::ProcessorSharing => None, // handled separately
        }
    }

    fn schedule_next_arrival(&mut self, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        let delay = self.config.arrival.draw(ctx.rng);
        let job = self.next_job;
        ctx.schedule::<0>(Time::from_raw(delay), self.id, QueueEvent::Arrival(job));
        self.next_job += 1;
    }

    /// Processor sharing, phase 1: advance remaining work of all jobs
    /// currently in service to `now` (each of the `last_population` jobs in
    /// service consumed `elapsed` / n seconds of work). Must be called
    /// *before* any membership change.
    fn ps_advance(&mut self, now: f64) {
        let elapsed = now - self.last_update;
        let n = self.last_population;
        if n > 0 && elapsed > 0.0 {
            for job in self.in_service.iter_mut() {
                job.work -= elapsed / n as f64;
            }
        }
        self.last_update = now;
    }

    /// Processor sharing, phase 2: after a membership change, schedule the
    /// next completion (minimum remaining work · population).
    fn ps_reschedule(&mut self, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        self.last_population = self.in_service.len();
        if let Some(id) = self.pending_departure.take() {
            ctx.cancel(id);
        }
        if let Some(next) = self
            .in_service
            .iter()
            .enumerate()
            .min_by(|a, b| {
                a.1.work
                    .total_cmp(&b.1.work)
                    .then_with(|| a.1.id.cmp(&b.1.id))
            })
            .map(|(i, j)| (i, j.id, j.work))
        {
            // Clamp at zero: the minimum-work job may sit a rounding hair
            // below zero at its own completion time.
            let delay = next.2.max(0.0) * self.in_service.len() as f64;
            let event = ctx.schedule::<0>(
                Time::from_raw(delay),
                self.id,
                QueueEvent::Departure(next.1),
            );
            self.pending_departure = Some(event);
        }
    }
}

impl Entity<Seconds, QueueEvent> for SimQueue {
    fn on_create(&mut self, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        self.metrics.last_t = 0.0;
        self.metrics.reset_at = 0.0;
        self.last_update = 0.0;
        self.schedule_next_arrival(ctx);
    }

    fn on_event(&mut self, event: QueueEvent, ctx: &mut Ctx<'_, Seconds, QueueEvent>) {
        let now = ctx.now.raw();
        self.maybe_apply_warmup(now);
        self.update_integrals(now);
        match event {
            QueueEvent::Arrival(_) => {
                self.schedule_next_arrival(ctx);

                let in_system = self.waiting.len() + self.in_service.len();
                if let Some(cap) = self.config.capacity {
                    if in_system >= cap {
                        self.metrics.blocked += 1;
                        return;
                    }
                }
                self.metrics.arrived += 1;

                match self.config.discipline {
                    Discipline::ProcessorSharing => {
                        // Existing jobs consumed work up to now; the new job
                        // starts fresh from this instant.
                        self.ps_advance(now);
                        let work = self.config.service.draw(ctx.rng);
                        self.in_service.push(Job {
                            id: event_id_of(&event),
                            arrived_at: now,
                            priority: 0,
                            work,
                        });
                        self.ps_reschedule(ctx);
                    }
                    _ => {
                        let job = Job {
                            id: event_id_of(&event),
                            arrived_at: now,
                            priority: 0,
                            work: 0.0,
                        };
                        if self.in_service.is_empty() {
                            self.start_service(job, ctx);
                        } else {
                            self.waiting.push(job);
                        }
                    }
                }
            }
            QueueEvent::Departure(job_id) => match self.config.discipline {
                Discipline::ProcessorSharing => {
                    if let Some(pos) = self.in_service.iter().position(|j| j.id == job_id) {
                        let job = self.in_service.remove(pos);
                        if self.counts(job.arrived_at) {
                            self.metrics.completed += 1;
                            self.metrics.total_sojourn += now - job.arrived_at;
                        }
                    }
                    self.ps_advance(now);
                    self.ps_reschedule(ctx);
                }
                _ => {
                    if let Some(pos) = self.in_service.iter().position(|j| j.id == job_id) {
                        let job = self.in_service.remove(pos);
                        if self.counts(job.arrived_at) {
                            self.metrics.completed += 1;
                            self.metrics.total_sojourn += now - job.arrived_at;
                        }
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
            self.metrics.total_wait += ctx.now.raw() - job.arrived_at;
        }
        let service = self.config.service.draw(ctx.rng);
        let mut started = job;
        started.work = service;
        self.in_service.push(started);
        ctx.schedule::<0>(
            Time::from_raw(service),
            self.id,
            QueueEvent::Departure(job.id),
        );
    }
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
/// # Examples
///
/// An M/M/1 queue at ρ = 0.5 has mean sojourn `W = 1/(μ−λ) = 2`:
///
/// ```
/// use tpt_des_queue::{run_queue, Discipline, Exponential, QueueConfig};
///
/// let metrics = run_queue(
///     QueueConfig {
///         arrival: Box::new(Exponential::new(0.5).unwrap()),
///         service: Box::new(Exponential::new(1.0).unwrap()),
///         discipline: Discipline::Fifo,
///         capacity: None,
///     },
///     20_000.0, // horizon
///     2_000.0,  // warm-up
///     42,       // seed
/// );
/// assert!((metrics.mean_sojourn() - 2.0).abs() < 0.4);
/// ```
pub fn run_queue(config: QueueConfig, horizon: f64, warmup: f64, seed: u64) -> QueueMetrics {
    let mut queue = SimQueue::with_warmup(EntityId(0), config, warmup);
    {
        let mut sim = Simulation::new(seed);
        let _id = sim.add_entity(&mut queue);
        sim.run_until(Time::<Seconds>::from_raw(horizon));
        sim.finalize();
    }
    queue.metrics().clone()
}
