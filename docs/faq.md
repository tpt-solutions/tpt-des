# FAQ

## Why should I trust the results?

Each analytic formula has tests, and the simulation tests compare against
those formulas. The [validating a model](validating-a-model.md) guide explains
how to run the same checks on your own model. The Kani harnesses cover the
event calendar, closed-network conservation, Markov probability mass, Petri
token conservation, and deadlock freedom in acyclic graphs. They run in CI as
a non-blocking job.

## Are runs reproducible?

Yes. Simulations draw randomness from one seeded generator (`SimRng`). The
same seed, the same model, and the same schedule give bit-for-bit identical
output on the same platform and toolchain. Samplers use the platform's `ln`
and `exp`, so results can differ in the last bits on another platform. The
library does not read the clock or depend on hash-map iteration order.

## How do I use it on a microcontroller?

Only `tpt-des-core` builds without `std`. Use it with
`default-features = false`, and enable only the `core` feature of the umbrella.
The core builds for `thumbv6m-none-eabi` and `wasm32-unknown-unknown` in CI.
The queue, resource, process, Petri, Markov, and stats crates need `std`.

## Why does a function return NaN instead of an error?

The math helpers in `tpt-des-stats::dist`, such as `ln_gamma` and the
quantile functions, return `NaN` outside their domain. This matches the usual
convention in numerical libraries, and it avoids a `Result` on every call in
inner loops. The estimators in `ci`, `batch`, and `timeseries` return
`Result<_, StatsError>`, so check those.

## Why does a function panic?

Some inputs can never produce a meaningful result, and returning an error
would hide a modelling bug. Examples: a negative or NaN delay, a time that is
NaN or infinite, a request larger than a resource's capacity, and a
`run_queue` warm-up that is not before the horizon. Each panic message says
what was wrong. The
[changelog](../CHANGELOG.md) lists these where they are new.

## Why is priority not what I expected?

Priority classes come from `QueueConfig::priority`. If you leave it as `None`,
every job is in class 0, and the priority discipline is first-come first-served.
Set a function that draws a class per arrival to see the effect. Lower class
numbers are served first.

## Can I use it to optimize a system?

Not directly. The library produces simulation output and intervals, and it
does not provide gradients or an optimizer. You can drive it from an external
optimizer: run the model for each candidate parameter set, with fixed seeds
(common random numbers), and compare the intervals. The `tpt-systems-optimisation`
integration in the spec is not implemented.

## Why is the Markov steady state computed differently from the spec?

The spec mentions power iteration or eigendecomposition. Power iteration does
not converge for periodic chains, so `Dtmc::stationary` solves the balance
equations directly, with the normalization constraint in place of one
equation. `tol` bounds the residual of that solution. See the
[status](status.md) page.

## Why is the core crate re-exported as `kernel`, not `core`?

`tpt_des::kernel` re-exports `tpt-des-core`. It was called `core` until the
0.1.x review follow-ups; the name was changed because a glob import of the
umbrella (`use tpt_des::*`) would shadow Rust's built-in `core` crate. The
Cargo feature is still named `core`, so `features = ["core"]` is unchanged.

## Is it on crates.io?

Not yet. Publishing is a maintainer decision, tracked in `todo.md`
(Phase 5). Until then, depend on the workspace by path.

## Where do I report a bug?

Open an issue on the repository. For a security problem, see
[SECURITY.md](../SECURITY.md).
