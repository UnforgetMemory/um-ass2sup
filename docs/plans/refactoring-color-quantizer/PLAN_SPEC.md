# PLAN SPEC — Sub-5 色彩管线 (color-quantizer 重构)

> **版本**: v1.0
> **状态**: ⏳ 待批准
> **依赖**: Sub-4 (subtitle-renderer 2.4) 完成
> **估算**: 5~7 天

---

## 1. 背景与现有架构诊断

### 1.1 当前 crate 现状

```text
crates/color-quantizer/src/
├── lib.rs          (225行)  Quantizer struct + quantize + quantize_with_palette + tests
├── types.rs        ( 97行)  Rgba, QuantizedFrame, DitherMethod
├── median_cut.rs   (356行)  median_cut + k-d tree + find_nearest_index + tests
├── dithering.rs    (133行)  floyd_steinberg_dither + ordered_dither
```

**不足**:
1. **无色彩空间管理** — 仅在原生 sRGB 空间做欧几里得距离，无视感知权重
2. **无 HDR** — 不支持 PQ/HLG transfer function 或 BT.2020 色域
3. **内存低效** — `median_cut` 全量 `pixels.to_vec()` 复制 x2，1080p=~18MB 堆分配
4. **无 SIMD** — Floyd-Steinberg 用 `f64` 全量 err 缓存（~33MB）无向量化
5. **k-d tree 偶现精度偏移** — `find_nearest_index` 与线性扫描在边界 case 有偏差（已有 hash 测试）
6. **单文件过大** — `median_cut.rs:356`、`lib.rs:225`（含测试混合）
7. **透明像素处理原始** — 仅 `a == 0` 判断，不支持预乘 alpha 或 alpha 感知量化
8. **无结构性优化** — palette 填充为随机顺序，无 temporal 优化

### 1.2 PGS 色彩约束

| 约束 | 值 |
|------|-----|
| 最大调色板颜色 | 255 不透明 + 1 透明 |
| 颜色深度 | 8-bit per channel (R,G,B,A) |
| 色域 | BT.709 (HD)，BT.2020 (UHD) 为扩展 |
| Transfer | sRGB γ ≈ 2.2 (SDR)，PQ ST 2084 / HLG (HDR) |
| 分辨率 | 1920×1080 (HD) / 3840×2160 (UHD) |
| 帧率 | 23.976/24/25/29.97/50/59.94 fps |

---

## 2. 目标架构

### 2.1 模块图

```text
crates/color-quantizer/src/
├── lib.rs                        ~100行  ── Public API (ColorPipeline builder)
├── error.rs                      ~ 50行  ── ColorError
│
├── color/                         ← 色彩科学 (DDD Bounded Context)
│   ├── mod.rs                    ~ 30行  ── re-exports
│   ├── space.rs                  ~120行  ── ColorSpace enum + primaries + white point
│   ├── transfer.rs               ~150行  ── TransferFunction (Linear, sRGB γ, PQ, HLG)
│   └── delta_e.rs                ~200行  ── CIE76/CIE94/CIEDE2000 + weighted-Euclidean
│
├── quantize/                      ← 量化 (DDD Bounded Context)
│   ├── mod.rs                    ~ 30行  ── re-exports
│   ├── palette.rs                 ~120行  ── PaletteBuilder (dedup, sort, merge, compress)
│   ├── median_cut.rs              ~200行  ── Improved median cut (SIMD sort candidate)
│   ├── naarahara.rs               ~150行  ── Naarahara (M0783) — 渐进式八叉树
│   ├── nearest.rs                 ~250行  ── k-d tree + SIMD linear scan (4×u32x4)
│   └── temporal.rs                ~100行  ── 帧间 palette 重用分析
│
├── dither/                        ← 抖色 (DDD Bounded Context)
│   ├── mod.rs                    ~ 30行  ── re-exports
│   ├── floyd_steinberg.rs         ~150行  ── SSE/AVX2 `i16x8` 弗洛伊德-斯坦伯格
│   ├── ordered.rs                 ~ 80行  ── 4×4/8×8 Bayer 有序抖动 (SIMD)
│   └── adaptive.rs                ~100行  ── 自适应抖动选择 (gradient detector)
│
├── frame/                         ← 帧 (DDD Bounded Context)
│   ├── mod.rs                    ~ 30行  ── re-exports
│   ├── view.rs                    ~100行  ── RgbaRef (零拷贝切片视图)
│   ├── owned.rs                   ~ 80行  ── QuantizedFrame (输出)
│   └── iter.rs                    ~ 80行  ── 逐块迭代器 (chunk-based)
│
└── pipeline.rs                    ~200行  ── Pipeline Orchestrator (构建 + 运行)
```

**文件大小限制**: 每文件 ≤250 行，超过必须拆分。

### 2.2 数据流

```
RGBA bytes (subtitle-renderer output)
    │
    ▼
[Frame View]        ── 零拷贝切片包装 (RgbaRef)
    │
    ▼
[Color Space Convert]  ── sRGB↔Linear, BT.709↔BT.2020, Gamma↔Linear
    │
    ├── [Tonemap]  ── (选择性) HDR→SDR tonemapping (Hable, Reinhard)
    │
    ▼
[Transfer Encode]  ── 可选: 应用 PQ/HLG 编码曲线
    │
    ▼
[Palette Builder]  ── Dedup → MedianCut / Naarahara → Palette (max 255)
    │
    ├── [Temporal Merge]  ── 与上一帧 palette 合并 (可选)
    │
    ▼
[Nearest Neighbor]  ── k-d tree / SIMD linear / hybrid → indices
    │
    ├── [Dither]  ── Floyd-Steinberg / Bayer / Adaptive (可选)
    │
    ▼
[QuantizedFrame]  ── palette + indices + transparent_index
    │
    ▼
→ pgs-encoder
```

---

## 3. 原子级任务分解 (10 Waves)

### W1: 基础设施 + 模块化骨架 (0.5天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 1.1 | `error.rs` | `ColorError` 枚举 (6-8 变体) | 50 |
| 1.2 | `frame/view.rs` | `RgbaRef<'a>` — 4× `u8` 行的零拷贝切片视图 | 100 |
| 1.3 | `frame/owned.rs` | `QuantizedFrame` — 输出类型，增加 `color_space` 元数据 | 80 |
| 1.4 | `frame/iter.rs` | `ChunkIter` — 按行/区域分割的迭代器 | 80 |
| 1.5 | `lib.rs` | `ColorPipeline` builder: `.with_max_colors()`, `.with_dither()`, `.with_color_space()`, `.quantize()` | 100 |
| 1.6 | `pipeline.rs` | Pipeline Orchestrator 骨架, stage chain builder | 200 |

**并行**: 1.1~1.4 可四路并行。1.5+1.6 依赖前置。

### W2: 色彩科学引擎 (1天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 2.1 | `color/mod.rs` | `ColorSpace` + `TransferFunction` 枚举，re-export | 30 |
| 2.2 | `color/space.rs` | BT.709↔XYZ↔BT.2020 矩阵，D65/D50 白点，primaries→RGB 矩阵 | 120 |
| 2.3 | `color/transfer.rs` | Linear: γ=1.0, sRGB: γ≈2.4 分段, PQ: ST 2084 (EOTF+OETF), HLG: ARIB STD-B67 | 150 |
| 2.4 | `color/delta_e.rs` | CIE76 `ΔE*ab`, CIE94 `ΔE*94` (graphic arts), CMC l:c, Weighted-Euclidean `wRGB` (|3,4,2| 权重) | 200 |

**注**: 2.4 delta_e 为可选高阶距离，默认仍使用加权欧几里得（速度快）。CIEDE2000 因复杂度高标记为 `#[cfg(feature = "cie2000")]`。

**并行**: 2.2+2.3 可并行。2.4 可独立。

### W3: 量化 — PaletteBuilder (0.5天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 3.1 | `quantize/palette.rs` | `PaletteBuilder` — dedup (fxhash 替代 HashSet), sort (luminance/usage), compress (≤255) | 120 |

### W4: 量化 — MedianCut 重写 (0.5天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 4.1 | `quantize/median_cut.rs` | 重写 median_cut: ① `SortByKey` 替代 `sort_by` ② 通道选择用 SSE `_mm_max_epu8` ③ 避免 `to_vec()` 中间复制 ④ 支持 alpha 通道分割 | 200 |

### W5: 量化 — Naarahara 八叉树 (0.5天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 5.1 | `quantize/naarahara.rs` | Naarahara (M0783) 渐进八叉树量化器: 逐像素插入八叉树 → 按像素频次裁剪至 max_colors → 叶节点平均 → palette | 150 |

### W6: 量化 — Nearest + SIMD (1天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 6.1 | `quantize/nearest.rs` | k-d tree (从现有迁移) + SIMD linear scan: `u32x4` 加载 4 个像素，并行计算 `sum(diff*diff)`；auto-dispatch SSE4.1/AVX2 | 250 |

**关键创新**: SIMD nearest — 4 路并行欧几里得距离

```rust
// 伪代码: SIMD 4-wide nearest
let px = i32x4::new(r as i32, g as i32, b as i32, a as i32);
for chunk in palette.chunks(4) {
    let p = i32x4::from_slice(…);  // 4 palette entries × 4 channels
    let d = (px - p) * (px - p);    // 4×4 = 16 ops, 1 SIMD instruction
    let dist = d.horizontal_sum();   // 4 reductions → 4 distances
    best = min(best, dist);          // 并行最小值
}
```

### W7: 量化 — Temporal 帧间优化 (0.5天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 7.1 | `quantize/temporal.rs` | 帧间 palette 差异分析 + delta palette 编码 + 重用决策引擎 | 100 |

### W8: 抖动 (1天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 8.1 | `dither/floyd_steinberg.rs` | SIMD Floyd-Steinberg: `i16x8` 误差缓存替代 `f64`；4 像素并行；减少 ~5× 内存 (33MB→6.6MB) | 150 |
| 8.2 | `dither/ordered.rs` | 4×4/8×8 Bayer + SIMD threshold 叠加 | 80 |
| 8.3 | `dither/adaptive.rs` | Sobel 梯度检测 → 平坦区域 Floyd-Steinberg, 纹理区域 Ordered, 边缘 None | 100 |

### W9: Tonemapping + HDR 管线 (1天)

| ID | 文件 | 内容 | 行数上限 |
|----|------|------|---------|
| 9.1 | `pipeline/tonemap.rs` | Hable (Uncharted2) + Reinhard 色调映射算子，逐像素 SIMD | 100 |
| 9.2 | `pipeline/convert.rs` | BT.2020 PQ/HLG → BT.709 sRGB 下行转换管线 | 120 |

### W10: 验证 + 性能基准 (1天)

| ID | 文件 | 内容 |
|----|------|------|
| 10.1 | `tests/color_spaces.rs` | 色彩空间 round-trip: sRGB↔Linear↔sRGB 误差 < 1/256; BT.2020↔XYZ↔BT.709 |
| 10.2 | `tests/delta_e.rs` | ΔE 计算与 reference 值对比 |
| 10.3 | `tests/quantize_parity.rs` | 新 quantize 与旧版 hash 一致性 (proptest 随机像素) |
| 10.4 | `tests/tonemap.rs` | HDR→SDR 色调映射视觉合理 |
| 10.5 | `benches/quantizer.rs` | Criterion 基准: 1080p 全帧, 4K 子区域, SIMD vs scalar |
| 10.6 | `benches/delta_e.rs` | ΔE 各算法吞吐 |

---

## 4. 依赖树

```
W1 ──────────────────────────────────────────────────────────
 ├─ 1.1 error.rs ─┐
 ├─ 1.2 view.rs ──┤
 ├─ 1.3 owned.rs ─┤
 ├─ 1.4 iter.rs ──┤
 └─ 1.5 lib.rs ◄──┘   ← depends on 1.1~1.4
     └─ 1.6 pipeline.rs  ← depends on 1.5

W2 ──────────────────────────────────────────────────────────
 ├─ 2.1 color/mod.rs ───┐
 ├─ 2.2 space.rs ◄──────┤
 ├─ 2.3 transfer.rs ◄───┤
 └─ 2.4 delta_e.rs ◄────┘  ← can be 2.1

W3 ── 3.1 palette.rs  ← can be parallel with W2

W4 ── 4.1 median_cut.rs  ← depends on 3.1
W5 ── 5.1 naarahara.rs   ← depends on 3.1 (parallel with W4)
W6 ── 6.1 nearest.rs     ← depends on 3.1 (parallel with W4+W5)

W7 ── 7.1 temporal.rs    ← depends on 3.1

W8 ──────────────────────────────────────────────────────────
 ├─ 8.1 floyd_steinberg.rs ← depends on 6.1
 ├─ 8.2 ordered.rs         ← depends on 6.1
 └─ 8.3 adaptive.rs        ← depends on 8.1+8.2

W9 ──────────────────────────────────────────────────────────
 ├─ 9.1 tonemap.rs         ← depends on 2.2, 2.3
 └─ 9.2 convert.rs         ← depends on 2.2, 2.3

W10 ─── tests + benches   ← depends on ALL
```

**并行执行机会**:
- W1.1~W1.4: 4 路并行
- W2.2+W2.3: 2 路并行
- W4+W5+W6: 3 路并行
- W8.1+W8.2: 2 路并行

---

## 5. 研究关键发现（来自 PGS 规范 + OSS 调研）

### 5.1 PGS 原生色彩空间是 YCbCr (BT.709)

PDS (Palette Definition Segment) 存储格式：

| 偏移 | 大小 | 字段 |
|------|------|------|
| 0x00 | 1 | Palette Entry ID (0–255) |
| 0x01 | 1 | Y (Luminance) |
| 0x02 | 1 | Cr (Red diff) |
| 0x03 | 1 | Cb (Blue diff) |
| 0x04 | 1 | Alpha |

BT.709 YCbCr → RGB:
```
R = Y                    + 1.5748 × (Cr - 128)
G = Y - 0.1873 × (Cb - 128) - 0.4681 × (Cr - 128)
B = Y + 1.8556 × (Cb - 128)
```

**架构决策**: quantizer 工作在 **Linear Light sRGB** 空间（感知均匀），YCbCr 转换在 pgs-encoder 边界做，不侵入 quantizer。

### 5.2 抖动必须在 Linear Light 空间

当前 Floyd-Steinberg 直接对 sRGB 字节做误差扩散——技术错误。
> "For correct results, all values should be linearized first, rather than operating directly on sRGB values."

**修复**: dither 流程增加 sRGB→Linear 前处理 + Linear→sRGB 后处理。

### 5.3 MMCQ + Voronoi 迭代 = 黄金标准

libimagequant v4 (Rust) 使用 Modified Median Cut + Voronoi refinement：
- **MMCQ**: 方差分割（非像素计数），JPEG 库使用半 median-cut + 半最大体积分割
- **Voronoi 迭代**: 1-3 轮 palette 中心微调，修复轴对齐偏差
- **Octree** 作为快速兜底（0.089s 1080p vs median-cut 0.15s）

**推荐**: MMCQ 作为默认，1 轮 Voronoi 迭代可选，Octree 为快速路径。

### 5.4 PGS 无 HDR 扩展（标准不存在）

- **PGS 规格中不存在 HDR** — 8-bit YCbCr BT.709，SDR only
- UHD Blu-ray 字幕仍是 SDR（播放器在 HDR 视频上复合 SDR 图形层）
- **W9 简化**: PQ/HLG bitstream 编码去除，仅保留 HDR→SDR tonemapping

### 5.5 每 epoch 最多 8 个 palette

FFmpeg `MAX_EPOCH_PALETTES = 8`。超过后需新 epoch（fresh acquisition point）。
帧间 palette 重用策略需在 8 个 palette 内循环。

---

## 6. 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| SIMD `u32x4` 在非 x86 平台降级 | 低 | 中 | 用 `cfg(target_arch)` + portable scalar fallback |
| CIEDE2000 `cos`/`atan` 溢出 | 低 | 低 | feature-gate, 精度上限测试 |
| Naarahara 与 median_cut 输出差异大 | 中 | 中 | 设 `COLOR_QUANTIZER_METHOD` 可选，默认 median_cut |
| HDR→SDR 色调映射导致影像变更 | 中 | 高 | 参数化 tone map 强度，默认保守值 |
| 旧 quantize hash 一致性测试不通过 | 低 | 低 | 仅在默认 config 下要求 hash 一致 |
| 过量 SIMD 导致编译占用 | 中 | 低 | `#[inline(never)]`, `cfg` 门控 |

---

## 7. 验证门

| 检查 | 通过条件 | 涉及 W |
|------|---------|--------|
| `cargo clippy -D warnings` | 零警告 | ALL |
| `cargo test --lib` | 100% PASS | ALL |
| 旧 quantize hash 一致性 | 默认 config hash 匹配 | W10.3 |
| 色彩空间 round-trip | sRGB↔Linear 误差 < 1/256 | W10.1 |
| 1080p 全帧 quantize | ≤ 50ms (debug) | W10.5 |
| ΔE 精度 | CIE94 与参考值 ±0.5 | W10.2 |
| 峰值内存 | < 20MB (debug 1080p) | — |
| Cargo deny / audit | 零 CVE / license 合规 | — |

---

## 8. 时间线

| 日 | Wave | 累计 |
|----|------|------|
| 1 | W1: 骨架 (0.5d) + W2: 色彩科学 (0.5d) | 1 |
| 2 | W3+W4+W5: 量化 (1.5d) | 2.5 |
| 3 | W6: SIMD nearest (1d) + W7: temporal (0.5d) | 4 |
| 4 | W8: 抖动 (1d) | 5 |
| 5 | W9: HDR/tonemap (1d) + W10: 验证 (1d) | 7 |

**总计**: 5~7 天

---

## 8. 未来扩展 (Sprint 后)

- **GPU 加速**: 将 quantize 部署到 WGSL 着色器 (vello 集成)
- **CIEDE2000 完全体**: 非 feature-gate 默认支持
- **CAM16 色貌模型**: 感知均匀色彩空间
- **ML 调色板预测**: 轻量模型预测最佳 palette
- **WebAssembly**: 在浏览器端运行 quantize
