# Changelog

All notable changes to the `tpt-des` umbrella crate are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

Changes to the member crates are listed in each member's own changelog. The
[workspace changelog](../../CHANGELOG.md) has the full history.

## [Unreleased]

### Breaking

- `tpt_des::core` is renamed to `tpt_des::kernel`. The Cargo feature is still
  `core`.

### Added

- `parallel` feature: parallel validation and replications, through
  `tpt-des-queue`.
- Examples in `examples/`: `process_bank_teller` (checked against Erlang C),
  `stats_replications`, `hybrid_bridge`, `call_center_erlang`, `inventory_store`,
  and `jackson_network`.
- Docs in `docs/` (workspace level): getting started, choosing a crate,
  validating a model, FAQ, and a status table.

### Fixed

- `process_bank_teller` imports `tpt_des::queue` but did not declare the feature.
  It now does, and CI passes it.
- The `std` feature also enables `tpt-des-process/std`.

## [0.1.0] — 2026-10-07

### Added

- Feature-gated re-exports of the member crates, with `std`, `core`, `queue`,
  `resource`, `process`, `petri`, `markov`, `stats`, and `verify` features.
