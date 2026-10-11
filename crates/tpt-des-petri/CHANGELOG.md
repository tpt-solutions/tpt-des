# Changelog

All notable changes to `tpt-des-petri` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Workspace-wide notes are in the [workspace changelog](../../CHANGELOG.md).

## [Unreleased]

### Breaking

- Typed time. `StochasticPetriNet::simulate(horizon)`, `SpnRun::horizon`, and the
  durations in `SpnRun::occupancy` use `Time<Seconds>`.
- `SpnError` gains `InvalidHorizon`, which covers negative horizons only. A NaN
  horizon is no longer reported this way, because it cannot be built as a `Time`.
- `SpnError` gains `TokenOverflow`, and `ReachabilityError` gains
  `TokenOverflow`.
- `StochasticPetriNet::to_ctmc` refuses more than `MAX_CTMC_STATES` (4096)
  states.
- `place_invariants` returns non-negative minimal invariants (Farkas), not an
  arbitrary null-space basis.

### Changed

- Reachability overflow returns `ReachabilityError::TokenOverflow`. It used to
  panic inside the search.
- `SpnError` and `ReachabilityError` are `#[non_exhaustive]`.

### Fixed

- Parallel input arcs from one place were checked one by one, so `fire` could
  underflow.
- Output overflow now refuses the firing.
- An infinite simulation horizon never ended.

## [0.1.0] — 2026-10-07

### Added

- Place/transition nets with weighted arcs, enabling, firing, and the incidence
  matrix.
- Stochastic Petri nets simulated with the Gillespie direct method.
- Reachability graphs, deadlock markings, and place and transition invariants.
- Conversion of bounded stochastic nets to a CTMC.
