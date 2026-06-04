# Code Coverage Baseline

Last generated: 2026-06-04 (Phase 26 W5)

## Result

**Line rate: 88.13%** (cargo-tarpaulin, workspace, xml output)

## How to regenerate

```bash
cargo install cargo-tarpaulin --locked
cargo tarpaulin --workspace --out Html --output-dir coverage/
cargo tarpaulin --workspace --out Xml --output-dir coverage/
# Open coverage/index.html in browser, or inspect coverage/cobertura.xml
```

The `coverage/` directory is **gitignored**; regenerate locally to inspect.
The artifact in this repo was a one-off export and has been removed.

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

## Tests added since baseline (P26-P29)

- ass-parser proptest +8 (Phase 26 W1)
- insta CLI snapshots: 5 cases (Phase 26 W3)
- pgs-encoder OOB fuzz regression tests (Phase 26 W2 + W2.5)
- 2 SRT→SRT self-check CLI tests (commit 8fdf28f)
- 2 input-size-guard CLI tests (commit 763385f)

The actual current line rate is likely higher than 88.13% baseline; rerun
the command above to get a fresh number.
