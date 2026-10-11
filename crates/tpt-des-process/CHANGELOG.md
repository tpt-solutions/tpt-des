# Changelog

All notable changes to `tpt-des-process` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Workspace-wide notes are in the [workspace changelog](../../CHANGELOG.md).

## [Unreleased]

### Breaking

- Typed time. `Sim::now`, `Sim::set_time`, `Sim::delay`, `ProcessSystem::now`,
  `ProcessSystem::set_time`, and `ProcessSystem::run_until` use `Time<Seconds>`.
  Bridge types use `Time<Seconds>` too.
- `ContinuousModel::integrate(dt)` takes `Time<Seconds>`. Implementers must
  change their signature.
- `Sim::delay` panics on a negative or NaN duration.
- `Sim::acquire` panics when the request exceeds the resource's capacity.
- `HybridBridge::new` and `step` validate their inputs.

### Fixed

- The executor advanced the clock to the next timer before running processes
  that were already runnable at the current time. A process spawned at t = 0.5
  therefore started at t = 1.0, and waits came out too short. It now runs
  runnable processes first and fires one timer at a time. The regression test is
  `spawned_process_runs_before_clock_advances`.
- A contended `acquire` never completed after `release`, because the woken
  waiter was dequeued before it could take its units.
- `release` clamps at capacity.
- `notify_all` no longer panics when a predicate reads simulation state.
- Finished processes are dropped.
- Self-interruption works.

## [0.1.0] — 2026-10-07

### Added

- Process-oriented simulation on `async`/`await`: `delay`, resource `acquire` and
  `release`, `wait_until` with `notify_all`, and `interrupt`.
- The `hybrid` module, with `ContinuousModel` and `HybridBridge`.
