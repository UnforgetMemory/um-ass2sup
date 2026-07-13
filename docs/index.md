# 📖 um-ass2sup Technical Wiki

> **ASS/SSA/SRT → Blu-ray SUP/PGS subtitle converter**  
> v3.0.0 · Rust workspace · 8 crates · Dual rendering backends

---

## 👋 Welcome

This wiki provides comprehensive technical documentation for `um-ass2sup`, a Rust-based subtitle converter that transforms open subtitle formats (ASS/SSA/SRT) into Blu-ray SUP/PGS bitmap subtitle streams, with BDN XML mastering as a secondary output.

Whether you are contributing to the codebase, integrating a crate into your own project, or debugging a rendering issue, these pages cover the architecture, design decisions, and implementation details that the README only touches on.

---

## 📚 Wiki Pages

| Page | Description |
|------|-------------|
| [🏛️ Architecture](architecture.md) | Full pipeline diagram, 8-crate breakdown, data flow from ASS to SUP |
| [🎨 Rendering Backends](rendering-backends.md) | Native (swash/tiny-skia) vs libass FFI — build modes, differences, trade-offs |
| [🛠️ Development Guide](development.md) | Build commands, testing, quality gates, CI workflows, contribution guidelines |
| [📦 PGS Encoder Design](pgs-encoder.md) | DDD architecture (domain/ + encoding/), segment types, display sets, PotPlayer compat |
| [🎯 Color Quantizer](color-quantizer.md) | Median-cut, k-d tree, dithering (Floyd-Steinberg/Ordered/None), palette reuse, pipeline |
| [🔤 Font System](font-system.md) | FontRegistry, FontDatabase, FontDiscovery, 8-level fallback, SimpleShaper, GlyphRasterizer |

---

## ✨ Project Highlights

- **Two rendering backends**: native (pure Rust, swash + tiny-skia) and libass FFI
- **Hand-written ASS parser** in `ass-core` — zero external parsing dependencies
- **Domain-Driven Design** for the PGS encoder (`domain/` + `encoding/` separation)
- **Color science**: color spaces, transfer functions, perceptual delta-E, tone mapping
- **SIMD acceleration** via the `wide` crate: Porter-Duff compositing, affine transforms
- **700+ tests**, property-based testing (proptest), fuzz targets, criterion benchmarks
- **PotPlayer compatibility**: `MAX_OBJECT_REFS=2` splitting, palette_update requirements

---

## 🏗️ Quick Navigation

```
crates/
  ass-core/                       # ASS/SSA/SRT parser → strong AST
  subtitle-validator/             # Syntax/overlap checks
  subtitle-renderer/              # [feature=native-backend] RGBA bitmap rendering
  libass-sys/                     # [feature=libass-backend] libass v0.17 FFI bindings
  subtitle-renderer-libass/       # [feature=libass-backend] libass rendering pipeline
  color-quantizer/                # RGBA → indexed color
  pgs-encoder/                    # Indexed frames → PGS/SUP binary segments
  bdn-xml/                        # Blu-ray mastering XML + PNG output
  ass2sup-cli/                    # CLI binary (ass2sup)
```

---

## 🔗 Related Resources

- [README.en.md](../README.en.md) — Project overview and quick start
- [AGENTS.md](../AGENTS.md) — AI agent instructions (also a good technical summary)
- [BENCHMARKS.md](../BENCHMARKS.md) — Performance benchmark data
- [CHANGELOG.md](../CHANGELOG.md) — Release history
- [SECURITY.md](../SECURITY.md) — Security policy and vulnerability reporting

---

<p align="center">
  <sub>Built with <code>cargo</code> · Licensed under Apache-2.0</sub>
</p>
