# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.3.x   | :white_check_mark: |
| 0.2.x   | :white_check_mark: (until 2026-09-01) |
| < 0.2   | :x:                |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report privately via one of:

- **GitHub Security Advisories**: <https://github.com/UnforgetMemory/um-ass2sup/security/advisories/new>
- **Email**: <security@your-domain.example> (replace with actual address)

Include:

1. Description of the vulnerability and impact
2. Reproduction steps / proof-of-concept
3. Affected version(s)
4. Suggested fix (if any)

We will acknowledge receipt within **72 hours** and aim to release a patch within **30 days** for critical issues.

## Scope

`ass2sup` is a **local CLI tool** that processes user-provided subtitle files. The primary attack surface is:

- Untrusted subtitle file input (ASS/SSA/SRT/BDN XML) — see [fuzz targets](crates/pgs-encoder/fuzz/, crates/color-quantizer/fuzz/) for our hardening work
- Untrusted font files via `fontdb`

## Dependencies

We track known vulnerabilities via:

- [`cargo-audit`](https://github.com/rustsec/rust-audit) — RustSec advisory database
- [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) — license / source / duplicate version checks

CI runs both on every push, pull request, and weekly. See [`.github/workflows/audit.yml`](.github/workflows/audit.yml) and [`deny.toml`](deny.toml).

## Known Issues

| ID             | Description                                                  | Status                  |
| -------------- | ------------------------------------------------------------ | ----------------------- |
| RUSTSEC-2025-0119 | `number_prefix` is unmaintained (transitive via `indicatif` 0.17) | Tracked for Phase 27 |
