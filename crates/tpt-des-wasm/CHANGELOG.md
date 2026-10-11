# Changelog

All notable changes to `tpt-des-wasm` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions adhere to [Semantic Versioning](https://semver.org).

This crate is not published. Its changes are listed here for reference.

## [Unreleased]

### Added

- WebAssembly bindings: `run_queue_json`, `validate_mmc_json`, and `size_json`.
  Each returns a JSON string, and bad input returns `{"error": "..."}` without
  panicking.
- The seed argument is a `u32`, so JavaScript passes a plain number.
- A static browser playground in `playground/`.
- CI builds the crate for `wasm32-unknown-unknown`.
