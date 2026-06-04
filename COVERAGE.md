# Code Coverage Baseline

Last generated: 2026-06-04 (Phase 26 W5)

## Result

**Line rate: 88.13%** (cargo-tarpaulin, workspace, xml output)

## How to regenerate

```bash
cargo install cargo-tarpaulin --locked
cargo tarpaulin --workspace --out Html --output-dir coverage/
# Open coverage/index.html in browser
```

## What was measured

- `ass-parser`, `subtitle-validator`, `subtitle-renderer`,
  `color-quantizer`, `bdn-xml`, `pgs-encoder`, `ass2sup-cli`
- Lib + integration tests, no doc tests, no benches

## Caveats

- The run that produced the 88.13% baseline was killed mid-test-suite
  (test_integration interrupted). The line rate is therefore a
  lower bound — uncovered code includes both genuine gaps and
  code that would have been hit by the remaining tests.
- The fuzz targets in `crates/*/fuzz/` are excluded from coverage
  measurement; their job is to surface crashes, not measure lines.
- Binary crates (`ass2sup`) and proc-macro crates are excluded by
  tarpaulin defaults.

## Action items

- [ ] Re-run after Phase 28 (Architecture) when error types unify —
      new error paths may temporarily drop coverage.
- [ ] Re-run after Phase 29 (Robustness) when resource limits land —
      the limit-enforcement branches are new code that needs tests.
- [ ] Target 90% by end of Phase 29.
