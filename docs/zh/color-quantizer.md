# 🎨 色彩量化管线

> RGBA → 索引色（≤255 色 + 8 位 alpha），k-d 树加速

---

## 📋 目录

- [设计概览](#设计概览)
- [模块结构](#模块结构)
- [颜色科学（color/）](#颜色科学color)
- [抖动（dither/）](#抖动dither)
- [量化（quantize/）](#量化quantize)
- [帧抽象（frame/）](#帧抽象frame)
- [量化管线编排（pipeline.rs）](#量化管线编排pipelinerers)
- [性能优化](#性能优化)
- [代码示例](#代码示例)
- [基准性能](#基准性能)
- [与 PGS 编码器的衔接](#与-pgs-编码器的衔接)

---

## 设计概览

颜色量化器是渲染后端与 PGS 编码器之间的关键桥梁：渲染后端产生全彩 RGBA 位图，但 PGS 只接受索引色（最多 255 色 + 8 位 alpha 的调色板）。量化器负责将完整色彩空间压缩到受限的调色板中，同时保持视觉质量。

```
RGBA 位图（渲染输出）
    │
    ▼
┌─────────────────────────────┐
│   颜色科学（color/）         │
│   · 颜色空间转换（RGB ↔ XYZ）│
│   · 色调映射                  │
│   · 色差计算（ΔE）           │
└──────────┬──────────────────┘
           ▼
┌─────────────────────────────┐
│   调色板生成（quantize/）    │
│   · Median-Cut 划分          │
│   · k-d 树最近色查找         │
│   · 调色板复用                │
└──────────┬──────────────────┘
           ▼
┌─────────────────────────────┐
│   抖动（dither/）            │
│   · Floyd-Steinberg 误差扩散 │
│   · Ordered Bayer 抖动       │
│   · Adaptive 自适应抖动      │
└──────────┬──────────────────┘
           ▼
    索引帧（IndexedFrame）
    → PGS 编码器
```

---

## 模块结构

```
crates/color-quantizer/src/
│
├── color/                          # 颜色科学
│   ├── space.rs                    # 颜色空间定义（RGB, XYZ, Lab, LCH）
│   ├── transfer.rs                 # 传递函数（sRGB, Linear, Rec.709）
│   ├── delta_e.rs                  # 感知色差（CIE76, CIE94, CIEDE2000）
│   ├── tonemap.rs                  # 色调映射（HDR → SDR）
│   └── mod.rs
│
├── dither/                         # 抖动方法
│   ├── floyd_steinberg.rs          # Floyd-Steinberg 误差扩散
│   ├── ordered.rs                  # Ordered Bayer 抖动
│   ├── adaptive.rs                 # 自适应抖动（内容感知）
│   └── mod.rs
│
├── quantize/                       # 调色板生成
│   ├── median_cut.rs               # Median-Cut 调色板
│   ├── nearest.rs                  # K-D 树最近色查找
│   ├── palette.rs                  # 调色板管理（排序、裁剪、合并）
│   ├── temporal.rs                 # 帧间调色板复用
│   ├── naarahara.rs                # 奈良原调色板映射（实验性）
│   └── mod.rs
│
├── frame/                          # 帧抽象
│   ├── mod.rs
│   ├── owned.rs                    # 自有帧（OwnedIndexedFrame）
│   ├── view.rs                     # 帧视图（IndexedFrameView）
│   └── iter.rs                     # 帧迭代器
│
├── pipeline.rs                     # 量化管线编排
├── error.rs                        # 领域错误类型
├── types.rs                        # 公共类型定义
└── lib.rs                          # crate 根
```

---

## 颜色科学（color/）

### color/space.rs

颜色空间定义和转换函数：

| 空间 | 描述 |
|------|------|
| **RGB** | 线性 / 非线性 RGB |
| **XYZ** | CIE 1931 XYZ |
| **Lab** | CIE L\*a\*b\*（感知均匀） |
| **LCH** | 圆柱坐标表示 |

转换路径：`RGB → XYZ → Lab → ΔE`

### color/transfer.rs

传递函数（gamma/电光转换函数）：

| 函数 | 适用 |
|------|------|
| **sRGB** | 标准 Web/显示内容 |
| **Linear** | 线性光空间 |
| **Rec.709** | 蓝光 / HDTV 标准 |

### color/delta_e.rs

感知色差计算，用于判断颜色是否在视觉上可区分：

| 标准 | 精度 | 性能 |
|------|------|------|
| **CIE76** | 基础 | 最快 |
| **CIE94** | 中等 | 快 |
| **CIEDE2000** | 最高 | 较慢 |

### color/tonemap.rs

色调映射：在输入素材包含高动态范围（HDR）颜色值时，将其映射到 SDR 范围内。

---

## 抖动（dither/）

抖动通过分布量化误差来减少色带（banding）伪影，用空间模式补偿颜色精度损失。

### Floyd-Steinberg 误差扩散

| 属性 | 值 |
|------|-----|
| 算法 | Serpentine（蛇行）扫描 |
| 误差分配 | 右 7/16, 下左 3/16, 下 5/16, 下右 1/16 |
| 效果 | 优秀的视觉结果，细腻的颜色过渡 |
| 性能 | 中等 |

```
像素映射  误差分配
  [X]   →  7/16
  3/16  5/16  1/16
```

### Ordered Bayer 抖动

| 属性 | 值 |
|------|-----|
| 矩阵 | Bayer 4×4 / 8×8 |
| 效果 | 结构化棋盘图案 |
| 性能 | 最快 |

### Adaptive 抖动

根据图像局部内容自适应选择抖动强度和模式，在平坦区域使用更重的抖动、在纹理区域减少抖动。

---

## 量化（quantize/）

### Median-Cut 调色板（quantize/median_cut.rs）

核心调色板生成算法：

```
1. 将所有像素放入一个"桶"
2. 对每个桶：
   a. 找出颜色范围最大的分量（R/G/B）
   b. 沿该分量的中位数将桶切为两个
3. 重复直到桶数达到目标调色板大小
4. 每个桶的平均色 = 调色板条目
```

- 复杂度：O(n log n)
- 控制参数：`max_colors`（最大颜色数，1-255）

### K-D 树最近色查找（quantize/nearest.rs）

调色板建立后，每个像素需要映射到最近的调色板颜色：

```
朴素方法：每个像素 O(k)（k = 调色板大小）
k-d 树方法：每个像素 O(log k)
加速比：约 2.57×
```

**`find_nearest_index`** 是核心函数，使用 k-d 树（k=3，RGB 三维空间）加速最近色查找。

### 调色板管理（quantize/palette.rs）

| 操作 | 描述 |
|------|------|
| `sort` | 按亮度/色相排序调色板 |
| `deduplicate` | 合并相似条目（`HashSet<u32>` 加速，O(n²)→O(n)） |
| `trim` | 裁剪到指定的最大颜色数 |

### 帧间调色板复用（quantize/temporal.rs）

相邻帧通常内容相似，直接复用上一帧的调色板可以：

- **减少 PDS 段大小**：只发送变化的部分
- **加速量化**：跳过调色板生成步骤
- **缓解闪烁**：避免帧间调色板条目重新排序导致的视觉闪烁

**策略：**
1. 尝试复用上一帧调色板
2. 计算颜色映射误差
3. 若误差超出阈值 → 重新生成调色板

### 奈良原映射（quantize/naarahara.rs）

实验性的调色板映射算法，基于奈良原（Narahara）的色彩分区方法。在特定的艺术内容上可能比 k-d 树更优。

---

## 帧抽象（frame/）

帧抽象层提供了操作索引帧的统一接口：

### frame/owned.rs — 自有帧

`OwnedIndexedFrame` 拥有自己的像素数据和调色板：

```rust
pub struct OwnedIndexedFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,         // 索引值（每个字节一个像素索引）
    pub palette: Vec<[u8; 4]>,   // RGBA 调色板
}
```

### frame/view.rs — 帧视图

`IndexedFrameView` 是帧的借用视图，类似 `&[u8]` 之于 `Vec<u8>`。

### frame/iter.rs — 帧迭代器

高效的行/像素迭代器，用于：

- 遍历所有像素进行映射
- 按行扫描进行抖动
- 统计颜色直方图

---

## 量化管线编排（pipeline.rs）

`QuantizationPipeline` 是整个量化过程的主编排器：

```rust
pub struct QuantizationPipeline {
    max_colors: u8,
    dither_method: DitherMethod,
    reuse_palette: bool,
    // ...
}

impl QuantizationPipeline {
    pub fn run(&self, frame: &RgbaFrame) -> Result<IndexedFrame>;
}
```

### 完整管线流程

```
1.  输入验证（空帧、尺寸检查）
2.  可选色调映射（HDR → SDR）
3.  颜色直方图统计
4.  调色板生成（Median-Cut）
    或 调色板复用（temporal 决策）
5.  像素映射（k-d 树最近色查找）
6.  可选抖动（Floyd-Steinberg / Ordered / Adaptive）
7.  IndexedFrame 组装（像素索引 + 调色板）
```

---

## 性能优化

| 优化 | 方法 | 加速比 |
|------|------|--------|
| k-d 树最近色 | `find_nearest_index` | 2.57× vs 线性查找 |
| 调色板去重 | `HashSet<u32>` | O(n²) → O(n) |
| 帧间复用 | temporal palette reuse | 减少 PDS 开销 |
| 并行量化 | rayon `par_iter()` | opt-in |
| SIMD 像素拷贝 | wide crate | 批量像素处理 |

---

## 代码示例

### 作为库使用

```toml
# Cargo.toml
[dependencies]
color-quantizer = "2.7"
```

```rust
use color_quantizer::pipeline::QuantizationPipeline;
use color_quantizer::frame::owned::OwnedIndexedFrame;

// 创建管线（最多 128 色，Floyd-Steinberg 抖动）
let pipeline = QuantizationPipeline::new()
    .max_colors(128)
    .dither(DitherMethod::FloydSteinberg);

// 运行量化
let indexed: OwnedIndexedFrame = pipeline.run(&rgba_frame)?;

// 获取结果
println!("调色板大小: {}", indexed.palette.len());
println!("像素数据: {} 字节", indexed.pixels.len());
```

### 运行示例

```bash
cargo run --release --example quantize_image -p color-quantizer
```

---

## 基准性能

| 基准 | 规模 | 中位耗时 | 备注 |
|------|------|---------|------|
| `quantizer_medium_320x180` | 320×180 | 13.1 ms | 量化 + 抖动 + 调色板 |
| `quantizer_large_1920x1080` | 1080p | 353 ms | k-d 树加速后（2.57×） |

```
cargo bench -p color-quantizer
```

1080p 量化大约 353 ms 完成——这对批量处理管线是可行的，但对于 23.976 fps 的实时播放（每帧约 41.7 ms）仍然太慢。因此量化器设计为离线处理模式。

---

## 与 PGS 编码器的衔接

量化器的输出直接供给 PGS 编码器：

```
量化器输出: IndexedFrame { pixels: Vec<u8>, palette: Vec<[u8;4]> }
    │
    ▼
PGS 编码器:
  ├─ PDS: palette → YCbCr 条目
  ├─ ODS: pixels → RLE 压缩
  └─ PCS: 尺寸、窗口、组合信息
```

两者通过共享的 `IndexedFrame` 接口衔接——在 crate 边界上，`pgs-encoder` 依赖 `color-quantizer` 的输出类型。

---

<p align="center">
  <sub>← [PGS 编码器设计](pgs-encoder.md) | [返回首页](index.md) | 下一篇：[字体子系统](font-system.md) →</sub>
</p>
