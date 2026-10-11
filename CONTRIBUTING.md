# Contributing to tpt-des

Contributions to tpt-des are made through **issues only**. Please do not open
pull requests. Report bugs, ask questions, and suggest features by opening an
issue on the [GitHub issue tracker](https://github.com/tpt-solutions/tpt-des/issues).

Maintainers will decide what to change and how. An issue that reproduces a bug
or describes a clear use case is the most useful kind of report.

## Reporting a bug

Include:

1. The crate and version (for example, `tpt-des-queue 0.1.0`), and whether you
   use the `tpt-des` umbrella or a member crate directly.
2. The Rust toolchain (`rustc --version`) and your operating system.
3. The features you enabled.
4. A minimal program or model file that shows the problem, and what you expected
   to happen instead.
5. Whether the problem appears in debug builds, release builds, or both. Some
   bugs only appear in one of them.

If a result looks wrong against a closed-form value, include the formula you
compared with and its inputs.

## Suggesting a feature

Describe the modelling problem you are trying to solve before describing the
API you would like. Say which crate the feature belongs in, if you know.

## Security issues

Please do not file public issues for security vulnerabilities. See
[SECURITY.md](SECURITY.md).

## Project tracking

The maintainers track work in `todo.md`. Issues are linked to its phases where
they apply, so reference the phase if you can.

## Licensing

tpt-des is dual-licensed under MIT OR Apache-2.0. By filing an issue, you agree
that any text or code you include in it may be used under the same terms.
