# Changelog

All notable changes to `tpt-des-cli` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

This crate is not published. Its changes are listed here for reference.

## [Unreleased]

### Added

- Binary `tpt-des` with the `run`, `validate`, `size`, and `network` commands,
  driven by TOML model files. Unknown keys are errors.
- Exit codes: `0` for success, `1` when `validate` finds a mismatch, and `2` for
  usage or input errors.
