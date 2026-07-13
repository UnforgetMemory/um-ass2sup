# ⚡ 双渲染后端对比

> native-backend（swash + tiny-skia） vs libass-backend（libass FFI）

---

## 📋 目录

- [概述](#概述)
- [native-backend（默认）](#native-backend默认)
- [libass-backend](#libass-backend)
- [对比详表](#对比详表)
- [与传统工具链的差异](#与传统工具链的差异)
- [构建方式](#构建方式)
- [运行切换](#运行切换)
- [如何选择](#如何选择)

---

## 概述

`ass2sup` 提供两种完全独立的渲染路径，通过 Cargo features 在编译时选择：

```
                         ┌──────────────────────┐
                         │     ass-core AST      │
                         └──────────┬───────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
                    ▼                               ▼
        ┌───────────────────┐            ┌──────────────────────┐
        │   native-backend  │            │    libass-backend    │
        │   swash 字形引擎  │            │  libass.so v0.17+   │
        │   tiny-skia 合成  │            │  C FFI 绑定           │
        └────────┬──────────┘            └──────────┬───────────┘
                 │                                  │
                 ▼                                  ▼
        ┌──────────────────────────────────────────────┐
        │          color-quantizer + pgs-encoder       │
        │         （双后端共享，完全一致的编码链路）      │
        └──────────────────────────────────────────────┘
```

**双后端共享量化与编码** —— 两条路径在渲染完成后汇入完全一致的 `color-quantizer` → `pgs-encoder` 管线。这意味着同一输入渲染出的 RGBA 位图在量化后可能因渲染差异导致不同的 PGS，但编码逻辑本身完全一致。

---

## native-backend（默认）

纯 Rust 实现，**零 C/C++ 运行时依赖**。

### 技术栈

```
swash（字形塑形 + 光栅化）→ tiny-skia（位图合成）
```

- **swash**：纯 Rust 字形引擎，提供字符→字形映射、字形度量、光栅化
- **tiny-skia**：纯 Rust Skia 子集，负责 Porter-Duff alpha 合成、模糊、位图操作
- **wide**：SIMD 加速（Porter-Duff 合成 `u32x4`、仿射变换双线性插值 `f32x4`）
- **parking_lot**：高效 `Mutex` 包装共享资源

### 字体系统

自建 `FontRegistry` 替代 fontdb：

- 跨平台字体发现（Linux / macOS / Windows）
- 8 级字体回退链
- `SimpleShaper` 基于 swash 的字符→字形映射
- `GlyphRasterizer` 基于 swash 的字形→alpha 位图

### 字形度量差异

native-backend 使用 swash **无 hinting 的原始字形度量**，这与 FreeType hinted advance 存在固有差异：

- 字形更大更宽（实测 +18% 宽 / +27% 高）
- `--compat-vsfilter` 标志提供约 0.764× 缩放因子实验性补偿

### 适用场景

- 不需要 libass 兼容性的独立部署
- 希望零 C 依赖的单二进制交付
- 自建 BDMV 流程中的批量自动化

---

## libass-backend

通过 FFI 调用系统安装的 `libass.so`（v0.17+），将形状计算和光栅化委托给 libass。

### 技术栈

```
libass.so（字形塑形 + 光栅化）→ 量化 → PGS 编码
```

- **libass-sys**：纯头文件 FFI 绑定，手动生成
- **subtitle-renderer-libass**：libass 渲染管线适配层

### 兼容性

libass 是 ASS 字幕的事实标准渲染器：

- 与其他 libass 工具（ffmpeg、mpv、VLC）渲染结果一致
- 完美的 ASS 规范兼容性
- FreeType hinted 字形度量，字形尺寸相对紧凑

### 适用场景

- 需要与 mpv/VLC/ffmpeg 渲染结果精确一致
- 已有的 libass 部署环境
- VSFilter 兼容性要求

---

## 对比详表

| 维度 | native-backend | libass-backend |
|------|---------------|----------------|
| **语言** | 纯 Rust | Rust + C FFI |
| **运行时依赖** | 零 | libass.so (v0.17+) |
| **字体引擎** | swash（无 hinting） | FreeType（hinted） |
| **字形度量** | 原始字形度量，偏大偏宽 | FreeType hinted 度量，紧凑 |
| **字形尺寸** | 大（实测 +18% 宽 / +27% 高） | 小（VSFilter 传统尺寸） |
| **字体发现** | 自建 FontRegistry | libass 内部 fontconfig |
| **合成** | tiny-skia + SIMD (wide) | libass 内部 |
| **特效支持** | 全部 ASS 特效 | 全部 ASS 特效 |
| **兼容性** | swash 原生行为 | 与 ffmpeg/mpv/VLC 一致 |
| **部署** | 单二进制，零外部 | 需要系统 libass |
| **跨平台** | Linux / macOS / Windows | Linux / macOS（需安装 libass） |
| **compiler 分支** | `native-backend` | `libass-backend` |

### 实测数据对比

DejaVu Sans 60px / Outline=2 / Shadow=2：

| 指标 | native-backend | libass-backend | 差异 |
|------|---------------|---------------|------|
| 边界框 | 274×42 px | 232×33 px | +18% 宽 / +27% 高 |

---

## 与传统工具链的差异

### 传统链路

```
ASS → AviSynth (avs2pipe) → easyavs2bdnxml / easyavs2sup → SUP
```

这类工具依赖 **libass**（通过 VSFilter 兼容层）进行字幕渲染。

### 根本差异

| 维度 | 传统链路（easyavs2bnxml 等） | um-ass2sup native-backend |
|------|------|------|
| 渲染引擎 | libass（via VSFilter/AviSynth） | **swash**（纯 Rust 字形引擎） |
| 字形度量 | FreeType hinted advance | **swash 无 hinting 的原始字形度量** |
| 部署形态 | Python / AviSynth / VSFilter 依赖链 | **单二进制，零运行时依赖** |
| 着色 | 依赖系统 FreeType + fontconfig | **自建 FontRegistry，纯 Rust 光栅化** |
| 合成粗体 | `FT_Outline_Embolden()` VSFilter 语义 | **swash 内建 embolden 合成**（参数不兼容） |
| 目标 | VSFilter 兼容性至上 | 蓝光合规 + 性能至上 |

### 含义

- **native-backend 输出的 SUP 字幕在播放时看起来比 libass 渲染的字更大、更粗**。这是 swash 与 FreeType hinting 之间的固有差异，并非 bug。
- `--compat-vsfilter` 实验性标志对字号施加约 0.764× 缩放因子，使 swash 输出在尺寸上更接近 VSFilter 传统值——但字形轮廓和间距的差异依然存在。
- **`um-ass2sup` 与 easyavs2bnxml 之间没有直接的 SUP 兼容性承诺**：两者使用不同的量化器、调色板策略、显示集分段逻辑。同一 ASS 输入产生的 SUP 在字节级别必然不同。

---

## 构建方式

### 默认（native-backend 仅）

```bash
cargo build --release
```

### libass-backend 仅

```bash
cargo build --release --no-default-features -F libass-backend
```

### 双后端（运行时切换）

```bash
cargo build --release --no-default-features -F native-backend,libass-backend
```

---

## 运行切换

双后端构建后，通过 `--backend` 标志选择：

```bash
# 使用 native 后端（默认）
ass2sup input.ass -o output.sup --backend native

# 使用 libass 后端
ass2sup input.ass -o output.sup --backend libass
```

---

## 如何选择

| 如果你的情况 | 推荐后端 |
|-------------|---------|
| 想要单二进制部署，零外部依赖 | **native-backend** |
| 要跟 mpv/VLC 渲染效果一致 | **libass-backend** |
| 做蓝光母盘，需要最精确的 ASS 合规 | **libass-backend** |
| CI/CD 流水线中批量自动化 | **native-backend**（无外部 dep） |
| 开发自己工具链的一部分 | **native-backend**（纯 Rust） |
| 原生支持跨平台一致性 | **native-backend**（swash 跨平台一致） |

---

<p align="center">
  <sub>← [架构详解](architecture.md) | [返回首页](index.md) | 下一篇：[开发指南](development.md) →</sub>
</p>
