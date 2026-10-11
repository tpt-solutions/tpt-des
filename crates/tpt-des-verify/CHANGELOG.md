# Changelog

All notable changes to `tpt-des-verify` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Workspace-wide notes are in the [workspace changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

- A deadlock-freedom proof for acyclic wait graphs.
- The calendar property test varies the event priority.

### Fixed

- Two Kani harnesses checked local `u8` models rather than the real code. They now
  use `Simulation` and `PetriNet`.

## [0.1.0] — 2026-10-07

### Added

- proptest strategies for queue parameters, stochastic matrices, acyclic edge
  lists, markings, schedule plans, and cancellation masks.
- Property tests for calendar ordering, cancellation, and live-count bookkeeping.
- Kani bounded model checks for the calendar, closed-network conservation, Markov
  normalization, Petri token conservation, and deadlock freedom.
