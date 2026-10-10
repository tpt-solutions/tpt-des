# Changelog

All notable changes to the `tpt-des` workspace are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions adhere to [Semantic Versioning](https://semver.org).

## [0.1.0] — 2026-10-07

Initial release of the full phased build (spec.txt Phases 0–5).

### Added
- **tpt-des-core**: phantom-typed `Time<Unit>` (ticks/seconds/hours),
  from-scratch binary-heap event calendar with `O(log n)` scheduling,
  lazy tombstone cancellation, and compile-time `const` priorities;
  entity lifecycle traits (`on_create`/`on_event`/`on_terminate`);
  seeded `SplitMix64` simulation RNG with bit-exact deterministic replay;
  `no_std` + `alloc` compatibility (`thumbv6m-none-eabi` verified).
- **tpt-des-queue**: closed-form M/M/1, M/M/c, M/G/1 (Pollaczek–Khinchine),
  Erlang B (stable recursion) and Erlang C; open Jackson networks solved
  with `tpt-math-linalg-dense`; single-chain closed networks via exact
  mean value analysis; DES single-station queues with arbitrary
  inter-arrival/service distributions and FIFO/LIFO/priority/
  processor-sharing disciplines (exact work-conserving PS).
- **tpt-des-resource**: runtime and `const`-generic capacity pools,
  discrete stores, continuous fluid buffers with rate operations,
  seize-delay-release controller with FIFO/priority queueing, priority
  preemption, request cancellation (timeou
