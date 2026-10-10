# tpt-des workspace tasks.
#
# Mirrors the `tpt-math` / `tpt-control` conventions: thin wrappers so the same
# commands work locally and in CI. Run `just` (or `just --list`) to see all.

default:
    @just --list

# Format the whole workspace.
fmt:
    cargo fmt --all

# Run clippy with -D warnings across all targets and features.
clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run the workspace test suite (all features).
test:
    cargo test --workspace --all-features

# Run `cargo-deny` checks (licenses, advisories, bans, sources).
deny:
    cargo deny check

# Build the no_std-capable crates for a bare-metal target.
no-std:
    cargo build -p tpt-des-core --no-default-features --target thumbv6m-none-eabi
    cargo build -p tpt-des-core --no-default-features

# Run Kani bounded model checks (requires `cargo kani`; see https://github.com/model-checking/kani).
kani:
    cargo kani -p tpt-des-verify

# Full verification gate.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features
    cargo deny check
