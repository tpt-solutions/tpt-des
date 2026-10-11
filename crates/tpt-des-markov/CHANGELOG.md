# Changelog

All notable changes to `tpt-des-markov` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Workspace-wide notes are in the [workspace changelog](../../CHANGELOG.md).

## [Unreleased]

### Breaking

- `Dtmc::stationary` is a direct linear solve, not power iteration. `tol` is now a
  residual bound, and `max_iter` is ignored.
- Reducible chains return `MarkovError::Singular`.
- `MarkovError` gains the variants `Singular` and `InvalidTolerance`.
- `MarkovError` is `#[non_exhaustive]`. Matches need a wildcard arm.

### Changed

- `matrix_exponential` uses 16 Taylor terms, evaluated by Horner's rule (it used
  24 by repeated products). It is about 1.3 to 1.45 times faster on the `expm`
  benchmark.
- `Dtmc::as_dmatrix` reads the chain directly, without an intermediate copy.
- The Markov first-passage reachability check is one reverse BFS, not one forward
  search per state.

### Fixed

- `matrix_exponential` scaled by the largest entry, not the induced norm.
- A NaN tolerance disabled row validation.
- Absorbing-state detection used exact float equality.

## [0.1.0] — 2026-10-07

### Added

- Validated DTMC and CTMC types, with steady-state solvers and a matrix
  exponential.
- Absorbing-chain analysis and first-passage times.
