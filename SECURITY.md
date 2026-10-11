# Security Policy

## Supported versions

Only the latest published release receives security fixes. Before the first
tagged release, that means the `master` branch.

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |
| < 0.1   | No        |

## Reporting a vulnerability

Please report suspected vulnerabilities privately, not through public issues
or pull requests.

Use GitHub's private vulnerability reporting for this repository:
**Security → Report a vulnerability** on the repository page
(<https://github.com/tpt-solutions/tpt-des/security/advisories/new>).

Include, where you can:

- the crate and version (or commit) affected,
- a minimal reproduction, ideally a short Rust program or test,
- what you expected to happen and what actually happened,
- the impact you believe it has.

The maintainer will acknowledge reports as soon as practical. If the report is accepted,
the fix will be developed on a private branch, released with a changelog entry
that credits the reporter (unless you ask otherwise), and disclosed once users
have had a reasonable window to upgrade.

## Scope

tpt-des is a simulation library. The areas most likely to matter are:

- **Memory safety and panics on untrusted input.** The workspace forbids
  `unsafe`. A panic reachable from safe input that the docs do not describe is
  a bug worth reporting.
- **Dependency vulnerabilities.** `cargo deny` runs advisory checks in CI. If
  you find a vulnerable transitive dependency that CI missed, report it.

Simulation results that are numerically wrong but not security-relevant
should go to the public issue tracker instead.
