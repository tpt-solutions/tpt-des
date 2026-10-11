# Changelog

All notable changes to `tpt-des-core` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Workspace-wide notes are in the [workspace changelog](../../CHANGELOG.md).

## [Unreleased]

### Breaking

- `Time<Unit>` is used in public APIs. `EventCalendar::pop_until` and `next_time`
  take or return `Time<Unit>`, and `Popped` is now `Popped<Unit, E>` with a
  `Time<Unit>` `time`.
- `Time::from_raw` and `SimClock::advance_to` check their inputs in every build
  profile. A non-finite time cannot be built as a `Time`, so it panics at
  construction.
- `Simulation::run_until` advances the clock to a finite horizon.
- `SimRng::core_mut` counts as one draw.
- `RunStats` gains `dropped_events`.

### Added

- `Time` gains `AddAssign`, `Div<f64>`, `Sum`, and `Display`.
- With the `serde` feature, `Time` implements `Serialize`.

### Changed

- The event calendar uses a slab with generation-tagged `EventId`s. Schedule and
  cancel are O(1) (cancel was O(log n)), and stale handles are rejected. The two
  calendar benchmarks run about 2.0 to 2.6 times faster.

## [0.1.0] — 2026-10-07

### Added

- Phantom-typed `Time<Unit>` (ticks, seconds, hours).
- A from-scratch binary-heap event calendar with O(log n) scheduling, lazy
  cancellation, and compile-time `const` priorities.
- Entity lifecycle traits: `on_create`, `on_event`, and `on_terminate`.
- A seeded SplitMix64 simulation RNG with bit-exact deterministic replay.
- `no_std` + `alloc` compatibility, verified for `thumbv6m-none-eabi`.
