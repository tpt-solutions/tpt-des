# tpt-des-cli

Command-line runner for [`tpt-des`](../tpt-des/README.md). It reads a TOML model file, runs one command on it, and prints the result. Use it to try a queue's behaviour without writing Rust, or to check a model in a script or CI job.

This crate is not published. Run it from the workspace.

## Usage

```sh
cargo run -p tpt-des-cli -- run model.toml        # simulate a queue, report mean ± SE
cargo run -p tpt-des-cli -- validate model.toml   # FIFO M/M/c check; exit 1 on mismatch
cargo run -p tpt-des-cli -- size model.toml       # fewest servers for max_wait
cargo run -p tpt-des-cli -- network model.toml    # Jackson or multi-class network
```

| Command | Model kinds | What it does |
|---|---|---|
| `run` | `queue` | Simulates the queue over `replications` seeds and reports the mean and standard error of `Wq`, `W`, and `L`. |
| `validate` | `queue` | Compares a FIFO M/M/c simulation with the analytic values, and fails if any metric disagrees. |
| `size` | `queue` | Finds the fewest servers whose analytic mean wait is at most `max_wait`. |
| `network` | `jackson`, `multiclass` | Solves the network analytically. |

Exit codes:

| Code | Meaning |
|---|---|
| `0` | The command ran, and any check passed. |
| `1` | `validate` ran, and at least one metric disagreed with the analytic value. |
| `2` | Usage error, unreadable file, invalid TOML, a model that does not fit the command, or invalid parameters. |

## Queue files

A queue file has `kind = "queue"` and these keys:

```toml
kind = "queue"
lambda = 2.4
mu = 1.0
servers = 3
discipline = "fifo"   # fifo | lifo | ps (ps: one server only)
horizon = 10000.0
warmup = 1000.0
seed = 3
replications = 6
max_wait = 0.5        # used by `size`
```

Arrivals and service times are exponential, so `run` and `validate` are exact checks against M/M/c. `max_wait` is needed only by `size`.

## Network files

A Jackson network sets `kind = "jackson"`, with the external arrival rates, the routing matrix, and the service rates for each node:

```toml
kind = "jackson"
external = [1.0, 0.0]
routing = [[0.0, 0.5], [0.0, 0.0]]
service_rates = [3.0, 2.0]
```

A multi-class closed network sets `kind = "multiclass"`, with one `[[classes]]` table per class. Each class gives its population, its routing matrix, and its mean service time at each node:

```toml
kind = "multiclass"

[[classes]]
population = 2
routing = [[0.0, 1.0], [1.0, 0.0]]
service_times = [1.0, 0.5]

[[classes]]
population = 1
routing = [[0.0, 1.0], [1.0, 0.0]]
service_times = [0.8, 0.4]
```

## Strict parsing

Unknown keys are errors. A typo such as `lamda` is reported rather than ignored, so a model never runs with a parameter silently set to its default.

## Library use

The binary is a thin wrapper over the library. `tpt_des_cli::execute(command, text)` runs one command on the contents of a model file and returns the output. The integration tests in `tests/` call the same function, so the binary and the tests share one code path.

## Testing

```sh
cargo test -p tpt-des-cli
```

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
