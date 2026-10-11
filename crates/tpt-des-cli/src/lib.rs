//! Command-line runner for `tpt-des`, driven by TOML files.
//!
//! A file names a model with `kind`:
//!
//! * `kind = "queue"`: one station with `servers` identical servers, exponential
//!   arrivals and service, and a discipline (`fifo`, `lifo`, or `ps`).
//! * `kind = "jackson"`: an open Jackson network with a routing matrix.
//! * `kind = "multiclass"`: a multi-class closed network with `[[classes]]`.
//!
//! Commands:
//!
//! * `run`: simulate a queue over `replications` seeds and report the mean and
//!   standard error of `Wq`, `W`, and `L`.
//! * `validate`: compare a FIFO M/M/c simulation with the analytic values. Exits
//!   with status 1 if any metric disagrees.
//! * `size`: the fewest servers whose analytic mean wait is at most `max_wait`.
//! * `network`: solve a Jackson or multi-class network analytically.
//!
//! [`execute`] runs one command on file contents and returns its output, so the
//! binary and the tests share one code path.

use serde::Deserialize;
use std::fmt::Write as _;
use tpt_des_core::Time;
use tpt_des_queue::planning::{size_for_mean_wait, validate_mmc};
use tpt_des_queue::sim::{run_queue_multi, Discipline, Exponential, QueueConfig};
use tpt_des_queue::{ClassSpec, JacksonNetwork, MultiClassClosed};

/// A parsed model file.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Model {
    /// A single station, simulated or sized.
    Queue(QueueSpec),
    /// An open Jackson network, solved analytically.
    Jackson(JacksonSpec),
    /// A multi-class closed network, solved by exact MVA.
    Multiclass(MulticlassSpec),
}

/// Parameters of a single queueing station.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueSpec {
    /// Arrival rate λ (customers per time unit).
    pub lambda: f64,
    /// Service rate μ per server.
    pub mu: f64,
    /// Number of identical servers.
    #[serde(default = "one")]
    pub servers: usize,
    /// `fifo`, `lifo`, or `ps` (processor sharing, one server only).
    #[serde(default = "fifo")]
    pub discipline: String,
    /// Simulated time per replication.
    pub horizon: f64,
    /// Simulated time discarded before measuring.
    #[serde(default)]
    pub warmup: f64,
    /// Seed of the first replication; replication `i` uses `seed + i`.
    #[serde(default = "one_u64")]
    pub seed: u64,
    /// Independent replications.
    #[serde(default = "five")]
    pub replications: u32,
    /// Target mean queue wait, for `size`.
    pub max_wait: Option<f64>,
}

/// An open Jackson network.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JacksonSpec {
    /// Exogenous arrival rate at each node.
    pub external: Vec<f64>,
    /// Routing probabilities: `routing[i][j]` is P(node i → node j).
    pub routing: Vec<Vec<f64>>,
    /// Service rate at each node.
    pub service_rates: Vec<f64>,
}

/// A multi-class closed network.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MulticlassSpec {
    /// The customer classes.
    pub classes: Vec<ClassFile>,
}

/// One class of a multi-class network, as written in a file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassFile {
    /// Number of circulating customers of this class.
    pub population: u32,
    /// Row-stochastic routing matrix for this class.
    pub routing: Vec<Vec<f64>>,
    /// Mean service time of this class at each node.
    pub service_times: Vec<f64>,
}

fn one() -> usize {
    1
}

fn one_u64() -> u64 {
    1
}

fn five() -> u32 {
    5
}

fn fifo() -> String {
    "fifo".to_owned()
}

/// What a command produced: its text, and whether it counts as success.
#[derive(Debug, PartialEq)]
pub struct Outcome {
    /// Text for standard output.
    pub output: String,
    /// False when the command ran but its check failed (for `validate`).
    pub ok: bool,
}

/// Run `command` on the contents of a model file.
///
/// # Errors
///
/// A message for an unknown command, unreadable TOML, a model that does not
/// fit the command, or invalid parameters.
pub fn execute(command: &str, text: &str) -> Result<Outcome, String> {
    let model: Model = toml::from_str(text).map_err(|e| format!("invalid model file: {e}"))?;
    match (command, model) {
        ("run", Model::Queue(spec)) => run_simulation(&spec),
        ("validate", Model::Queue(spec)) => validate(&spec),
        ("size", Model::Queue(spec)) => size(&spec),
        ("network", Model::Jackson(spec)) => jackson(&spec),
        ("network", Model::Multiclass(spec)) => multiclass(&spec),
        ("run" | "validate" | "size", _) => Err(format!("`{command}` needs `kind = \"queue\"`")),
        ("network", Model::Queue(_)) => {
            Err("`network` needs `kind = \"jackson\"` or `kind = \"multiclass\"`".to_owned())
        }
        (other, _) => Err(format!("unknown command `{other}`")),
    }
}

fn discipline_of(name: &str, servers: usize) -> Result<Discipline, String> {
    match name {
        "fifo" => Ok(Discipline::Fifo),
        "lifo" => Ok(Discipline::Lifo),
        "ps" if servers == 1 => Ok(Discipline::ProcessorSharing),
        "ps" => Err("processor sharing is single-server only".to_owned()),
        other => Err(format!(
            "unknown discipline `{other}` (use fifo, lifo, or ps)"
        )),
    }
}

fn queue_config(spec: &QueueSpec, discipline: Discipline) -> Result<QueueConfig, String> {
    let bad = |e| format!("invalid rate: {e}");
    Ok(QueueConfig {
        arrival: Box::new(Exponential::new(spec.lambda).map_err(bad)?),
        service: Box::new(Exponential::new(spec.mu).map_err(bad)?),
        discipline,
        capacity: None,
        priority: None,
    })
}

fn time(seconds: f64) -> Result<Time<tpt_des_core::Seconds>, String> {
    if !seconds.is_finite() {
        return Err(format!("time value {seconds} is not finite"));
    }
    Ok(Time::from_raw(seconds))
}

fn run_simulation(spec: &QueueSpec) -> Result<Outcome, String> {
    if spec.replications < 2 {
        return Err("`replications` must be at least 2".to_owned());
    }
    let discipline = discipline_of(&spec.discipline, spec.servers)?;
    let horizon = time(spec.horizon)?;
    let warmup = time(spec.warmup)?;

    let mut wq = Vec::new();
    let mut w = Vec::new();
    let mut l = Vec::new();
    for i in 0..spec.replications {
        let config = queue_config(spec, discipline)?;
        let m = run_queue_multi(
            config,
            spec.servers,
            horizon,
            warmup,
            spec.seed.wrapping_add(u64::from(i)),
        );
        wq.push(m.mean_wait().raw());
        w.push(m.mean_sojourn().raw());
        l.push(m.time_avg_number());
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} server(s), {} discipline, {} replications",
        spec.servers, spec.discipline, spec.replications
    );
    let _ = writeln!(out, "{:<4} {:>14} {:>14}", "", "mean", "std error");
    for (name, samples) in [("Wq", &wq), ("W", &w), ("L", &l)] {
        let (mean, se) = mean_and_std_error(samples);
        let _ = writeln!(out, "{name:<4} {mean:>14.6} {se:>14.6}");
    }
    Ok(Outcome {
        output: out,
        ok: true,
    })
}

fn mean_and_std_error(samples: &[f64]) -> (f64, f64) {
    let n = samples.len() as f64;
    let mean = samples.iter().sum::<f64>() / n;
    let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, (var / n).sqrt())
}

fn validate(spec: &QueueSpec) -> Result<Outcome, String> {
    if spec.discipline != "fifo" {
        return Err("`validate` compares FIFO M/M/c only".to_owned());
    }
    let report = validate_mmc(
        spec.lambda,
        spec.mu,
        u32::try_from(spec.servers).map_err(|_| "too many servers".to_owned())?,
        time(spec.horizon)?,
        time(spec.warmup)?,
        spec.replications,
        spec.seed,
    )
    .map_err(|e| format!("cannot validate: {e}"))?;
    Ok(Outcome {
        output: report.to_string(),
        ok: report.passes(),
    })
}

fn size(spec: &QueueSpec) -> Result<Outcome, String> {
    let max_wait = spec
        .max_wait
        .ok_or_else(|| "`size` needs `max_wait`".to_owned())?;
    let sizing = size_for_mean_wait(spec.lambda, spec.mu, time(max_wait)?)
        .map_err(|e| format!("cannot size: {e}"))?;
    Ok(Outcome {
        output: format!(
            "servers: {}\nanalytic Wq: {:.6} (target {max_wait})\nutilization per server: {:.6}\n",
            sizing.servers, sizing.analytic.wq, sizing.analytic.rho
        ),
        ok: true,
    })
}

fn jackson(spec: &JacksonSpec) -> Result<Outcome, String> {
    let net = JacksonNetwork::new(&spec.external, &spec.routing, &spec.service_rates)
        .map_err(|e| format!("invalid network: {e}"))?;
    let arrivals = net
        .node_arrivals()
        .map_err(|e| format!("cannot solve: {e}"))?;
    let (lengths, total) = net
        .mean_lengths()
        .map_err(|e| format!("cannot solve: {e}"))?;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<6} {:>12} {:>12} {:>12}",
        "node", "arrivals", "rho", "L"
    );
    for (i, (lambda, l)) in arrivals.iter().zip(&lengths).enumerate() {
        let rho = lambda / spec.service_rates[i];
        let _ = writeln!(out, "{i:<6} {lambda:>12.6} {rho:>12.6} {l:>12.6}");
    }
    let _ = writeln!(out, "total L = {total:.6}");
    Ok(Outcome {
        output: out,
        ok: true,
    })
}

fn multiclass(spec: &MulticlassSpec) -> Result<Outcome, String> {
    let classes: Vec<ClassSpec> = spec
        .classes
        .iter()
        .map(|c| ClassSpec {
            population: c.population,
            routing: c.routing.clone(),
            service_times: c.service_times.clone(),
        })
        .collect();
    let net = MultiClassClosed::new(&classes).map_err(|e| format!("invalid network: {e}"))?;
    let result = net.mva().map_err(|e| format!("cannot solve: {e}"))?;
    let mut out = String::new();
    let _ = writeln!(out, "{:<6} {:>14}", "class", "throughput");
    for (c, x) in result.throughput.iter().enumerate() {
        let _ = writeln!(out, "{c:<6} {x:>14.6}");
    }
    let utils: Vec<String> = result
        .utilizations
        .iter()
        .map(|u| format!("{u:.6}"))
        .collect();
    let _ = writeln!(out, "node utilizations: {}", utils.join(", "));
    Ok(Outcome {
        output: out,
        ok: true,
    })
}

/// The usage text for `--help` and for argument errors.
pub const USAGE: &str = "usage: tpt-des <run|validate|size|network> <model.toml>

  run       simulate a queue over several replications
  validate  compare a FIFO M/M/c simulation with the analytic values
  size      fewest servers whose analytic mean wait meets max_wait
  network   solve a Jackson or multi-class closed network

Exit status: 0 on success, 1 when `validate` finds a mismatch, 2 on error.
";
