# Changelog

All notable changes to `tpt-des-resource` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Workspace-wide notes are in the [workspace changelog](../../CHANGELOG.md).

## [Unreleased]

### Breaking

- `PreemptPolicy::Fifo` is renamed to `PreemptPolicy::PreemptivePriority`. The
  old name described the queueing order, not preemption.
- `seize` rejects a request larger than the pool's capacity with
  `ResourceError::InvalidParameter`.
- `Buffer::pour_duration` and `Buffer::drain_duration` return
  `Option<Time<Seconds>>`, and return `None` when the result overflows.
- `ResourceError` is `#[non_exhaustive]`. Matches need a wildcard arm.

### Fixed

- `PreemptivePriority` could double-grant units in release builds. The take lived
  inside a `debug_assert!`.
- An entity with two queued requests lost a wait-for edge, so deadlocks could go
  undetected.
- A new request could jump ahead of queued waiters (starvation).
- `held_fraction` returned the inverse.
- `Store::put` could overflow near `i64::MAX`.
- `Buffer` accepted NaN.

## [0.1.0] — 2026-10-07

### Added

- Runtime and `const`-generic capacity pools.
- Discrete stores and continuous fluid buffers with rate operations.
- A seize–delay–release controller with FIFO or priority queueing, priority
  preemption, and request cancellation.
- Deadlock detection over the wait-for graph.
