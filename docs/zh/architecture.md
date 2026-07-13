# 🏗️ 架构详解

> 完整管线流程、crate 职责、数据流与内存模型

---

## 📋 目录

- [整体管线](#整体管线)
- [crate 职责详述](#crate-职责详述)
- [数据流](#数据流)
- [渲染堆栈（native-backend）](#渲染堆栈native-backend)
- [性能约束](#性能约束)
- [内存模型](#内存模型)
- [关键架构决策](#关键架构决策)

---

## 整体管线

```
            ┌────────────┐
            │  输入文件   │  ASS / SSA / SRT
            └─────┬──────┘
                  │
                  ▼
         ┌─────────────────┐
         │    ass-core     │  → 强类型 AST
         └────────┬───────-┘
                  │ 可选
                  ▼
         ┌──────────────────────[ 渲染后端 ]──────────────────────┐
         │                                                       │
         ▼                                                       ▼
   ┌──────────────────┐                               ┌──────────────────────┐
   │  native-backend  │                               │   libass-backend     │
   │  swash +         │                               │   libass FFI         │
   │  tiny-skia       │                               │   (libass-sys)       │
   └────────┬─────────┘                               └──────────┬───────────┘
            │                                                     │
            ▼                                                     ▼
         ┌───────────────────────────────────────────────────────────┐
         │                color-quantizer                           │
         │  RGBA → 索引色（≤255 + alpha），k-d 树加速                │
         └────────────────────────┬─────────────────────────────────-┘
                                  │
                                  ▼
         ┌───────────────────────────────────────────────────────────┐
         │                    pgs-encoder                           │
         │  量化帧 → PGS 段（PCS/WDS/PDS/ODS）                      │
         │  DDD 架构：domain/（纯模型）+ encoding/（序列化）          │
         └──────────────────┬──────────────────────────────────────-┘
                            │
                  ┌─────────┴──────────┐
                  ▼                    ▼
            ┌──────────┐        ┌────────────┐
            │  .sup    │        │  BDN XML   │
            │  SUP/PGS │        │  + PNG 序列 │
            └──────────┘        └────────────┘
```

---

## crate 职责详述

### ass-core

ASS/SSA/SRT 的解析层，纯 Rust 手写解析器，**零外部解析依赖**。输出强类型 AST，保留 Style/Dialogue/Font 全部信息。

```
输入文本 → Lexer → 事件流 → 格式检测 → AST（AssFile）
```

- `SubtitleFormat::detect` 自动识别 ASS/SSA/SRT
- 完整的 override tag 解析：`\fn`、`\fs`、`\b`、`\i`、`\move`、`\fad`、`\fade`、`\t`、`\clip`、`\an`、`\pos`、`\org`、`\frx`/`\fry`/`\frz`、`\bord`、`\shad`、`\be`、`\blur`、卡拉 OK (`\k`/`\K`/`\ko`)
- `unsafe_code = "deny"` 保证
- 依赖：thiserror, tracing

关键模块：

| 模块 | 职责 |
|------|------|
| `lexer.rs` | 词法分析 → 标记流 |
| `types.rs` | ASS 核心类型（AssFile, Style, Dialogue） |
| `event.rs` | 事件模型与解析 |
| `style.rs` | 样式解析与管理 |
| `color.rs` | ASS 颜色解析（`&HAABBGGRR`） |
| `span.rs` | 文本段与 override tag 交织 |
| `override_tag/` | 全部 override tag 解析器 |
| `time/` | 时间戳、帧率、FPS 转换 |
| `srt.rs` | SRT 解析器 + 序列化器 |
| `karaoke.rs` | 卡拉 OK 时序解析 |
| `section.rs` | ASS 章节结构（Script Info / V4+ Styles / Events / Fonts） |
| `error.rs` | 解析器错误类型 |

### subtitle-validator

在渲染前对解析后的 AST 进行校验。校验是**可选的**——管线可以不经过校验直接进入渲染。

- 语法校验
- 事件重叠检测（`strict` / `lenient` 模式）
- 依赖：ass-core

### subtitle-renderer（native-backend）

双后端渲染器中 native-backend 的实现。基于 swash 字形引擎 + tiny-skia 位图合成。

```
ass-core AST → RenderContext → 字形塑形 (SimpleShaper) →
字形光栅化 (GlyphRasterizer) → 合成 (composite_glyph) →
特效处理 (blur/shadow/outline) → 仿射变换 (transform_layer) →
裁剪 → 位图输出
```

关键模块：

| 模块 | 职责 |
|------|------|
| `font/` | 整个字体子系统（FontRegistry + SimpleShaper + GlyphRasterizer） |
| `renderer/` | 渲染管线核心（build_context, 逐帧渲染） |
| `effects/` | 特效（blur, shadow, clip, composite） |
| `transform.rs` | SIMD 仿射变换（wide::f32x4） |
| `karaoke.rs` | 卡拉 OK 渲染 |
| `context.rs` | RenderContext 构建 |

### libass-sys

libass v0.17 的纯头文件 FFI 绑定。手写绑定，无 `build.rs` 编译时依赖，仅提供 `libass.so` 运行时链接。

### subtitle-renderer-libass（libass-backend）

双后端中 libass 后端的实现，通过 `libass-sys` 调用系统 libass。

- libass 原生渲染，完美 ASS 规范兼容
- 渲染结果与 ffmpeg、mpv、VLC 一致
- DDD 架构：`domain/` (renderer, pipeline, timeline, frame) + `infra/` (vendor, pgs_adapter)

### color-quantizer

将 RGBA 位图量化为索引色（≤255 色 + 8 位 alpha）。完整颜色科学管线。

```
输入 RGBA → 可选色调映射 → 颜色空间转换 → 量化（Median-Cut）→
调色板映射（k-d 树加速）→ 可选抖动 → 输出索引帧
```

模块结构：

| 模块 | 职责 |
|------|------|
| `color/` | 颜色科学：space, transfer, delta_e, tonemap |
| `dither/` | 抖动：floyd_steinberg, ordered, adaptive |
| `quantize/` | 量化：median_cut, nearest (k-d tree), palette, temporal |
| `frame/` | 帧抽象：owned, view, iter |
| `pipeline.rs` | 量化管线编排 |

### pgs-encoder

将量化帧编码为 PGS/SUP 二进制流。领域驱动设计（DDD），严格分离纯领域模型与编码逻辑。

```
模块化框架 → 显示集构建（PCS/WDS/PDS/ODS）→
段序列化 → SUP 文件写入
```

架构详见 [PGS 编码器设计](pgs-encoder.md)。

### bdn-xml

蓝光母版 XML 描述符 + PNG 序列写入器。`quick-xml` 序列化 BDN XML，`png` crate 写入 PNG。

### ass2sup-cli

CLI 二进制入口。通过 Cargo features 编译时分发渲染后端：

- `native-backend`（默认）：使用 subtitle-renderer
- `libass-backend`：使用 subtitle-renderer-libass

支持 `--backend native|libass` 运行时切换（双后端构建时）。

---

## 数据流

### 帧处理管线（native-backend）

```
ass-core parse
    │
    ▼
subtitle-validator.validate()  [可选]
    │
    ▼
subtitle-renderer (native-backend):
  build_context() → RenderContext per event per timestamp
    │
    ├─ shape_horizontal() / shape_vertical() → Vec<ShapedGlyph>
    ├─ rasterize_glyph() → RasterizedGlyph
    ├─ composite_glyph() → RGBA layer
    ├─ apply_effects() → blur / shadow / outline
    ├─ transform_layer() → AffineTransform
    └─ composite_subregion() → full RGBA frame
    │
    ▼
color-quantizer:
  QuantizationPipeline::run() → IndexedFrame
    │
    ▼
pgs-encoder:
  PgsEncoder::encode_frame() → DisplaySet
  DisplaySet::to_bytes() → Segment::serialize()
    │
    ▼
SUP file write / BDN XML + PNG sequence
```

### 帧处理管线（libass-backend）

```
ass-core parse
    │
    ▼
subtitle-renderer-libass:
  libass_ass_render_frame() → ASS_Image list
    │
    ▼
color-quantizer → pgs-encoder → SUP / BDN XML
```

---

## 渲染堆栈（native-backend）

> ⚠️ 强调：无 fontdb / 无 cosmic-text / 无 rustybuzz

```
Trace: ass-core parse
  → RenderContext (build_context)
  → shape_horizontal/vertical (SimpleShaper/swash)
  → glyph rasterization (GlyphRasterizer/swash)
  → composite_glyph
  → effects (blur/shadow/outline)
  → transform_layer (AffineTransform for scale/rotate/shear/perspective)
  → composite_subregion
```

**字体管线：**

```
shape:    SimpleShaper::shape(text, font_data, font_size) → Vec<ShapedGlyph>
          Maps chars→glyph_id via swash FontRef.charmap(), records advance width

resolve:  FontRegistry.query() → FontId → get_font_data() → Vec<u8>
          Uses name-parsed weight/style fallback + font_map per-style fallback chain

rasterize: GlyphRasterizer::rasterize(font_data, glyph_id, font_size) → RasterizedGlyph
           Uses swash CacheKey for glyph cache lookup

composite: composite_glyph(layer, rasterized, x, y, color) — Porter-Duff over per pixel
```

---

## 性能约束

此项目对渲染性能有严格要求，特别是在热路径上：

- **无堆分配** — 字形循环、合成、变换等热路径禁止堆分配
- **PixmapPool** — 复用 Pixmap 缓冲：`pool_get`/`pool_put`，8 缓存条目，`Mutex` 包装
- **AffineTransform** — SIMD (`wide::f32x4`) 双线性插值实现 `apply_to_pixmap`
- **composite_over** — SIMD (`wide::u32x4`) Porter-Duff over，4 像素块处理
- **并行渲染** — `rayon` 的 `par_iter()` 在 `build_display_set` 中应用，每个 worker 一次持有 1 帧（~8.3 MB @ 1080p），无需中间 `Vec<RenderedFrame>`
- **调色板去重** — `HashSet<u32>` 将 O(n²) 降至 O(n)
- **k-d 树量化** — `find_nearest_index` 调色板映射加速（2.57×）

---

## 内存模型

- **渲染器持有** — `FontRegistryRenderResources`（registry + pool + font_map，全部包在 `Mutex` 中）
- **build_context** — 每个 event 每个 timestamp 产生一个 `RenderContext`
- **render_event_font_registry** — 每个 event 分配一个 `layer: Pixmap`（pool_get → fill/outline/shadow → composite → pool_put）
- **transform_layer** — 分配输出缓冲（变换比例通常为 1:1 或更小）
- **峰值内存** — `max_events_per_timestamp × layer_size + output_buffer`，1080p 下 < 50 MB

---

## 关键架构决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 字体渲染 | swash（无 FreeType hinting） | 纯 Rust、零 C 依赖、跨平台一致 |
| 位图合成 | tiny-skia | 纯 Rust Skia 子集，API 简洁 |
| 像素格式 | RGBA 预处理 → 索引色 | PGS 需要索引色 + alpha |
| 量化算法 | Median-Cut + k-d 树映射 | 兼顾质量与性能 |
| 编码架构 | DDD（domain/ + encoding/） | 分离纯模型与序列化关注点 |
| 后端选择 | Cargo features 编译时决定 | 零运行时开销、可选 libass 依赖 |
| SIMD | wide crate（f32x4 / u32x4） | 平台无关 SIMD（支持 x86/x64/ARM） |
| 并发 | rayon | 数据并行、无中间缓冲 |

---

<p align="center">
  <sub>← [返回首页](index.md) | 下一篇：[双渲染后端对比](rendering-backends.md) →</sub>
</p>
