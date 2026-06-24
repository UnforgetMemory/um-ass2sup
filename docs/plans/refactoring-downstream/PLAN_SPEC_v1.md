# PLAN SPEC — 下游重构 (pgs-encoder + ass2sup-cli)

> **版本**: v1.0
> **状态**: ⏳ 待批准
> **依赖**: color-quantizer 2.5 完成（新 ColorPipeline + 统一类型）
> **估算**: 3~5 天

---

## 1. 当前架构诊断

### 1.1 依赖链

```
color-quantizer (新管线 + 统一类型)
    ├── pgs-encoder    (4 源文件 + 6 测试/bench)
    └── ass2sup-cli    (1 源文件 + 6 测试)
```

### 1.2 当前问题

#### pgs-encoder/src/color.rs（165 行）

| 问题 | 详情 |
|------|------|
| YCbCr 转换独立维护 | `rgba_to_ycbcr_bt601` / `rgba_to_ycbcr_bt709` 公式重复于 `color-quantizer::color::space.rs` |
| `build_palette` 硬编码选择 | `display_height > 576 ? BT.709 : BT.601`，无法外部配置色彩空间 |
| `swap()` 函数功能单一 | 仅做 `0↔pivot` 交换，属于 PGS RLE 编码细节，不应在 `color.rs` |
| 测试耦合 | 10 个测试依赖具体的 BT.601/BT.709 值，重构后需重新验证 |

#### pgs-encoder/src/encoder.rs

| 问题 | 详情 |
|------|------|
| `build_palette` 调用点 | 3 处调用：`encode_frame`、`build_single_window_display_set`、`build_invisible_display_set` |
| `QuantizedFrame` 使用 | 仅读取 `.palette`、`.indices`、`.transparent_index`、`.width`、`.height` 字段——当前兼容 |

#### ass2sup-cli/src/lib.rs

| 问题 | 详情 |
|------|------|
| 使用旧 API | `Quantizer` / `quantize_with_palette` / `Rgba` |
| 手写 palette-reuse 逻辑 | 与 `ColorPipeline::quantize_with_prev` 功能重复 |
| 不支持 HDR 配置 | 无 CLI `--color-space` 或 `--tonemap` 参数 |
| 主循环耦合 | 量化逻辑与渲染/编码混合在同一循环 |

---

## 2. 目标架构

### 2.1 pgs-encoder 目标

```
pgs-encoder/src/
├── color.rs           ← 精简：只保留 ycbcr↔rgba + swap + palette_to_rgba
│                         移除：build_palette（升级到 color-quantizer 统一管线）
│                         保留：ycbcr_to_rgba（PGS 解码必需）
│                         保留：swap()（RLE 编码必需）
│                         保留：palette_to_rgba（PGS 解码必需）
│                         新增：使用 color-quantizer::color::space 验证函数
├── encoder.rs         ← build_palette 改为从 frame.palette 直接构造 PaletteEntry，
│                         或调用 color-quantizer::pipeline::ColorPipeline 辅助
│
```

### 2.2 ass2sup-cli 目标

```
ass2sup-cli/src/
├── lib.rs             ← 替换量化逻辑
│                         --quantizer → 移除（永远 median-cut）
│                         新增 --color-space (srgb|bt709|bt2020)
│                         新增 --tonemap (hable|reinhard|aces)
│                         quantizer: Quantizer → ColorPipeline
│                         prev_palette: Vec<Rgba> → Option<QuantizedFrame>
```

---

## 3. 原子级任务分解

### Wave 1: pgs-encoder 清理（1 天）

#### W1.1 — `color.rs` 精简

**目标**: 移除重复的 YCbCr 转换，`build_palette` 改为纯构造器

**操作**:
1. `rgba_to_ycbcr_bt601()` / `rgba_to_ycbcr_bt709()` → 标记 `#[inline]` 保持性能，**不移除**（PGS 编码需要精确的 `f64` 取整语义，与 `color-quantizer` 的 `f32` 线性代数空间不同）
2. `build_palette()` → 添加 `color_space: ColorSpace` 参数，委托到 `color-quantizer` 进行到 YCbCr 的转换决策
3. 添加 `#[cfg(test)]` 验证函数，与 `color-quantizer::color::space` 交叉验证 BT.601/BT.709 矩阵

**并行**: W1.1 可独立

#### W1.2 — `encoder.rs:build_palette` 调用统一

**目标**: 3 个调用点统一到一个辅助方法

**操作**:
1. 新建 `fn build_palette_entries(&self, frame: &QuantizedFrame) -> Vec<PaletteEntry>`
2. 读取 `frame.palette` + `self.display_height`（或未来 `frame.color_space`）
3. 替换 3 个调用点

**验收**: `cargo test -p pgs-encoder` 通过，`build_palette` 测试覆盖完整

---

### Wave 2: ass2sup-cli 量化替换（1~2 天）

#### W2.1 — `ColorPipeline` 替代 `Quantizer`

**操作**:
1. 替换 `use color_quantizer::{quantize_with_palette, DitherMethod, Quantizer, Rgba}` → `use color_quantizer::{ColorPipeline, DitherMethod, QuantizedFrame}`
2. 构建 `ColorPipeline` 替代 `Quantizer::new(max_colors).with_dither(method)`
3. 更新并行路径（line 954-968）：`quantizer.quantize()` → `pipeline.quantize()`
4. 更新时序路径（line 1000-1023）：`quantize_with_palette()` → `pipeline.quantize_with_prev()`
5. `prev_palette: Option<Vec<Rgba>>` → `prev_frame: Option<QuantizedFrame>`

**注意**: `QuantizedFrame` 现在包含 `color_space` 字段，可以在帧间传递色彩空间信息

#### W2.2 — 新增 CLI 色彩空间参数

**操作**:
1. 在 clap `Args` 结构体新增：
   ```rust
   /// Output colour space (affects quantizer gamut)
   #[arg(long, default_value = "srgb")]
   color_space: String,  // srgb | bt709 | bt2020
   
   /// HDR → SDR tone mapping operator
   #[arg(long)]
   tonemap: Option<String>,  // hable | reinhard | aces
   ```
2. 转换为 `ColorSpace` 枚举 + `ToneMapOperator`
3. 传递给 `ColorPipeline` builder

#### W2.3 — 测试文件更新

**操作**:
1. 6 个测试文件中的 `Quantizer::new()` → 酌情保留或改为 `ColorPipeline` 构建
2. 验证 CLI 测试覆盖 `--color-space` 和 `--tonemap`

---

### Wave 3: 清理与验证（1 天）

#### W3.1 — 遗留 API 标记

**操作**:
1. `color-quantizer` 的 `Quantizer` → 添加 `#[deprecated(since = "0.6.0", note = "use ColorPipeline instead")]`
2. `Quantizer::quantize()` 等 → 也标记 `#[deprecated]`
3. 不影响已有代码（ass2sup-cli 已迁移到新 API）

#### W3.2 — 测试

| 测试 | 场景 | 验证方式 |
|------|------|---------|
| pgs `test_ycbcr_roundtrip` | BT.601/BT.709 roundtrip 仍在 ±1 内 | 对比旧值 |
| ass2sup-cli `test_srt_conversion` | SRT→SUP 全链路 | cargo test |
| ass2sup-cli E2E OCR | 真实 ASS→SUP 圆通 | cargo test --ignored |
| `--color-space` | CLI 参数生效 | 指定 bt709/bt2020 |

---

## 4. 依赖图

```
W1.1 ──┐
W1.2 ──┤── 可并行 ──→ W2.1 ──→ W2.2 ──→ W2.3 ──→ W3.1
         └ W2.1 ──→ W3.2
```

W1.1 和 W1.2 可并行执行。W2.x 系列依赖 W1 完成后启动。W3.x 在所有变更后执行。

---

## 5. 不做的（Out of Scope）

- ❌ `pgs-encoder` `ycbcr_to_rgba` 不移除（解码器唯一依赖）
- ❌ `pgs-encoder` `swap()` 不移除（RLE 核心函数）
- ❌ ass2sup-cli 渲染/编码循环模块化（属于 2.6+）
- ❌ HDR bitstream 编码（PGS 标准不支持 HDR）
- ❌ `QuantizedFrame` 字段重命名（破坏性变更推迟到 v2.0）

---

## 6. 验证门

| 检查 | 条件 |
|------|------|
| `cargo clippy -p pgs-encoder -D warnings` | ✅ 零警告 |
| `cargo clippy -p ass2sup-cli -D warnings` | ✅ 零警告（或仅预先存在） |
| `cargo test -p pgs-encoder` | ✅ 全部通过 |
| `cargo test -p ass2sup-cli --lib` | ✅ 全部通过 |
| `cargo test -p color-quantizer` | ✅ 全部通过 |
| ass2sup-cli `--color-space` CLI | ✅ 参数解析正确 |
| 旧 `Quantizer` `#[deprecated]` | ✅ 标记存在 |
| `--no-check-fonts` 结合体验 | ✅ 无退化 |

---

## 7. 风险矩阵

| # | 风险 | 概率 | 影响 | 缓解 |
|---|------|------|------|------|
| R1 | YCbCr 精度差异 (f64 vs f32) | 低 | 高 | 保持 pgs-encoder 用 f64，不共享矩阵 |
| R2 | `prev_palette` 类型变化打破时序逻辑 | 中 | 中 | 使用 `QuantizedFrame` 包装，保留 `.palette` 访问 |
| R3 | CLI 新增参数与现有参数冲突 | 低 | 低 | clap 自动处理 |
| R4 | deprecated 标记产生 warning 影响 CI | 低 | 中 | 在 ass2sup-cli 中加 `#[allow(deprecated)]` 过渡 |

---

## 8. 时间线

| 日 | Wave | 累计 |
|----|------|------|
| 1 | W1: pgs-encoder 清理 (1d) | 1 |
| 2-3 | W2: ass2sup-cli 迁移 (2d) | 3 |
| 4 | W3: 清理 + 验证 (1d) | 4 |

**总计**: 3~5 天
