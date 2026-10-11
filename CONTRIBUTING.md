# Contributing to tpt-des

Thanks for considering a contribution. This guide covers how to build and test
the workspace, what a change needs before it is ready, and how the project
tracks its work.

## Getting set up

The toolchain is pinned in `rust-toolchain.toml` (stable, with `rustfmt` and
`clippy`). The minimum supported Rust version is 1.84, set as `rust-version`
in the workspace `Cargo.toml`.

```sh
cargo build --workspace
cargo test --workspace
```

The `justfile` wraps the common checks:

| Recipe       | What it runs                                              |
|--------------|-----------------------------------------------------------|
| `just fmt`   | `cargo fmt --all --check`                                 |
| `just clippy`| clippy over all targets, warnings denied                  |
| `just test`  | the workspace test suite                                  |
| `just deny`  | `cargo deny check` (licenses, advisories, bans, sources)  |
| `just no-std`| the `no_std` build of `tpt-des-core`                      |
| `just kani`  | Kani bounded model checks (needs `cargo kani` installed)  |

Run `just` with no arguments to list them.

## What a change needs

Before opening a pull request, check that:

1. **Formatting and lints pass.** `cargo fmt --all` and
   `cargo clippy --workspace --all-targets -- -D warnings`.
2. **Tests pass in debug and release.** Some bugs only appear in release
   builds, where `debug_assert!` is compiled out. Run
   `cargo test --workspace` and `cargo test --workspace --release`.
3. **Bug fixes come with a regression test** that fails on the old code. Where
   the old behaviour cannot be reached through the public API, say so in the
   pull request.
4. **Docs build cleanly.** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`.
5. **`CHANGELOG.md` is updated** under `[Unreleased]` for user-visible changes.
   Mark breaking changes under **Breaking**.
6. **`todo.md` is updated** if the change closes or changes a tracked item.

## Scope and style

- **Correctness first.** Numerical routines are checked against closed-form
  results (Erlang, Kleinrock, Kac, gambler's ruin) wherever one exists.
  Simulation tests compare against those results with a stated tolerance.
- **No new dependencies without discussion.** The event calendar and the
  statistical numerics (Student-t, incomplete beta, erfc) are implemented in
  this workspace. Linear algebra and probability sampling come from the
  published `tpt-math` crates. `cargo deny` enforces the license and source
  policy in `deny.toml`.
- **Match the surrounding code.** Use the existing naming, comment density,
  and error-handling patterns in the crate you are editing. Public functions
  that can fail return `Result` with a crate error type. Math helpers that
  are only defined on part of their domain return `NaN` outside it, and say
  so in their docs.
- **Determinism.** Simulations are seeded and must replay bit-for-bit. Do not
  introduce wall-clock time, hash-map iteration order, or unseeded randomness
  into simulation paths.

## Project tracking

`todo.md` is the phased build tracker. Phase 6 holds correctness fixes from
the 2026-10-11 review; Phases 7 to 10 cover CI hygiene, performance,
adoption material, and proposed features. Phase 10 items need confirmation
before they are built, so please open an issue before starting one.

## Reporting security issues

Please do not file public issues for security vulnerabilities. See
[SECURITY.md](SECURITY.md).

## Licensing

tpt-des is dual-licensed under MIT OR Apache-2.0. By contributing, you agree
that your contribution is licensed under the same terms.
