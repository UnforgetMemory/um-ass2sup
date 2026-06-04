# ADR 0002: Parallelize per-frame quantize, not encode

## Status
Accepted (Phase 27, June 2026).

## Context
Profiling of `ass2sup-cli convert` on a 30-event 1920x1080 stress asset showed total wall time dominated by quantize + encode, split roughly 60/40. Sequential encode of 30 frames averaged 0.366s end-to-end.

Two natural parallelism axes exist:
1. **Across frames** — each frame is independent in terms of input data, so the quantize + encode pipeline could in principle run N frames in parallel.
2. **Within frame** — the kd-tree nearest-color search is a per-pixel operation, embarrassingly parallel.

## Decision
Parallelize the **quantize step** across frames using `rayon::par_iter`, leaving the **encode step** strictly sequential.

## Rationale
- `PgsEncoder::encode_frame` takes `&mut self` and mutates `composition_number`, `object_id`, `object_version`, and `frame_count` on every call (encoder.rs lines 92–95). These counters are part of the PGS spec — the decoder uses them to detect dropped or reordered segments. Splitting state across threads would produce non-monotonic IDs and break compatibility with standard players.
- `Quantizer::quantize` takes `&self` — the kd-tree is rebuilt per call and is purely a function of the input. Safe to share across threads.
- Quantize is the bottleneck within a frame (2.6× speedup from kd-tree in Phase 24); encode is mostly RLE + segment assembly, already fast.

## Consequences
- Wall time on the stress asset: 0.366s → 0.270s (1.36×) when `--parallel-frames` is passed.
- Output is byte-identical to the sequential path (verified via two runs of the stress asset compared with `cmp`).
- Default behavior is unchanged — `--parallel-frames` is opt-in. Rationale: determinism-by-default is preferred for a tool that may be used in rendering pipelines.
- The 0.270s floor is now mostly render (already parallel) + sequential encode + I/O. Further gains would require a different encoder architecture (e.g. producing all segments then assigning IDs in a post-pass) — deferred per ADR scope.

## Alternatives considered
- **Parallel encode with mutex-protected counters**: rejected — would serialize encode calls behind the mutex, eliminating the speedup.
- **Post-hoc ID assignment**: rejected — requires knowing total frame count upfront; the current CLI processes frames streaming-style from the ASS parser.
- **Lock-free ID generation via atomics**: rejected — would change ID semantics (non-monotonic across frames, only monotonic within a frame's segments) and risk breaking decoder compatibility.
