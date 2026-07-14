<p align="center">
  <a href="https://github.com/UnforgetMemory/um-ass2sup">
    <img src=".github/logo.png" alt="ass2sup" width="300">
  </a>
</p>

<h1 align="center">ass2sup</h1>

<p align="center">
  <b>ASS / SSA / SRT → Blu-ray SUP / PGS 字幕转换器</b><br>
  <sub>附带 BDN XML 蓝光母版输出 · Rust 实现 · v2.7.1</sub>
</p>

<p align="center">
  <a href="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml/badge.svg" alt="Audit"></a>
  <a href="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/release.yml"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <a href="LICENSE-APACHE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License: Apache-2.0"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.89%2B-orange.svg" alt="Rust 1.89+"></a>
  <a href="https://github.com/UnforgetMemory/um-ass2sup/releases"><img src="https://img.shields.io/badge/version-2.7.1-blue.svg" alt="Version"></a>
</p>

<p align="center">
  <a href="README.en.md">English</a> · <b>简体中文</b>
</p>

---

## 📋 目录

- [🚀 项目介绍](#-项目介绍)
- [🔬 与传统工具链的差异](#-与传统工具链的差异)
- [🎯 双渲染后端](#-双渲染后端)
- [⚡ 核心特性](#-核心特性)
- [🏃 快速开始](#-快速开始)
- [🏗️ 架构总览](#️-架构总览)
- [📂 工作区结构](#-工作区结构)
- [🔧 安装](#-安装)
- [📖 使用方式](#-使用方式)
- [📝 CLI 参考](#-cli-参考)
- [📦 作为 Rust 库使用](#-作为-rust-库使用)
- [📊 性能与基准](#-性能与基准)
- [🧪 测试与质量](#-测试与质量)
- [🛡️ 安全](#️-安全)
- [🤝 贡献](#-贡献)
- [📄 许可证](#-许可证)
- [🙏 致谢](#-致谢)

---

## 🚀 项目介绍

`ass2sup` 将开源字幕格式（ASS/SSA/SRT）转换为蓝光播放器原生支持的位图字幕流（PGS/SUP），同时支持 BDN XML 母版输出。

**典型场景：**

- 自制 BDMV 时替换或追加多语字幕轨
- 批量处理整季番剧的字幕自动化流水线
- 保留 ASS 特效（`\move`、`\fad`、`\t`、卡拉 OK）的时序精度
- 23.976/29.97 等非整数帧率下的逐帧 PTS 校准

---

## 🔬 与传统工具链的差异

### 背景

传统的 ASS→SUP 转换链路通常为：

```
ASS → AviSynth (avs2pipe) → easyavs2bdnxml/easyavs2sup → SUP
```

这类工具依赖 **libass**（通过 VSFilter 兼容层）进行字幕渲染，生成的 SUP 字幕尺寸、字形外观以 libass 的输出为基准。**`um-ass2sup` 并非上述链路中任一工具的 Rust 替代品，而是一条根本不同的技术路径。**

### 根本差异

| 维度 | 传统链路（easyavs2bnxml 等） | um-ass2sup native-backend |
|---|---|---|
| 渲染引擎 | libass（通过 VSFilter/AviSynth） | **swash**（纯 Rust 字形引擎） |
| 字形度量 | FreeType hinted advance | **swash 无 hinting 的原始字形度量** |
| 渲染结果 | 较小的字形尺寸、较窄的字距 | **字形更大更宽**（实测 +18% 宽 / +27% 高）¹ |
| 部署形态 | Python / AviSynth / VSFilter 依赖链 | **单二进制，零运行时依赖** |
| 着色 | 依赖系统的 FreeType + fontconfig | **自建 FontRegistry，纯 Rust 光栅化** |
| 合成粗体 | `FT_Outline_Embolden()` VSFilter 语义 | **swash 内建 embolden 合成**（参数不兼容） |
| 目标 | VSFilter 兼容性至上 | 蓝光合规 + 性能至上 |

¹ 实测数据：DejaVu Sans 60px / Outline=2 / Shadow=2，native-backend 渲染边界框 274×42 px，libass 渲染边界框 232×33 px。

### 含义

- **native-backend 输出的 SUP 字幕在播放时看起来比 libass 渲染的字更大、更粗**。这是 swash 字形引擎与 FreeType hinting 之间的固有差异，并非 bug。
- 如果追求与 libass（ffmpeg、mpv、VLC）完全一致的渲染结果，应使用 **libass-backend** 构建模式（`--no-default-features -F libass-backend`）。
- 项目提供一个实验性的 `--compat-vsfilter` 标志，对字号施加约 0.764× 缩放因子，使 swash 输出在尺寸上更接近 VSFilter 传统值——但字形轮廓和间距的差异依然存在。
- **`um-ass2sup` 与 easyavs2bnxml 之间没有直接的 SUP 兼容性承诺**：两者使用不同的量化器、不同的调色板策略、不同的显示集分段逻辑。同一个 ASS 输入产生的 SUP 在字节级别必然不同。

---

## 🎯 双渲染后端

`ass2sup` 提供两种渲染路径，编译时通过 Cargo features 选择。

### native-backend（默认）

纯 Rust 实现，零 C/C++ 依赖：

```
swash（字形塑形 + 光栅化）→ tiny-skia（位图合成）
```

- `FontRegistry` + `SimpleShaper` + `GlyphRasterizer`，基于 swash
- 8 级字体回退链（精确匹配 → 后缀剥离 → 别名 → 硬编码 CJK → 跨平台扫描 → 泛型 → SansSerif → 任意）
- SIMD 加速（`wide` crate）：Porter-Duff 合成、仿射变换双线性插值
- 适合不需要 libass 兼容性的轻量部署

### libass-backend

通过 FFI 调用系统 libass（v0.17+）：

```
libass.so（字形塑形 + 光栅化）→ 量化 → PGS 编码
```

- 完美的 ASS 规范兼容性
- 渲染结果与其他 libass 工具（ffmpeg、mpv、VLC）一致
- 适合需要 ASS 精确匹配的场景

### 构建方式

```bash
# 默认（native 后端）
cargo build --release

# 仅 libass 后端
cargo build --release --no-default-features -F libass-backend

# 双后端（运行时通过 --backend 切换）
cargo build --release --no-default-features -F native-backend,libass-backend
```

---

## ⚡ 核心特性

### 输入与解析

- ASS v4+、SSA v4、SubRip（`.srt`）自动识别（`SubtitleFormat::detect`）
- 手写解析器，零外部解析依赖
- 完整 AST，保留 Style/Dialogue/Font 全部信息
- SRT 自检：`ass2sup in.srt --to-srt -o out.srt && diff in.srt out.srt`

### 渲染

- **native-backend**：swash 字形塑形，8 级字体回退，全面 ASS 特效支持
- **libass-backend**：libass 原生渲染，完美规范兼容
- ASS 特效：卡拉 OK、`\move`、`\fad`/`\fade`、`\t`、3D 旋转、各向异性边框、矢量裁剪、滚动横幅

### 量化与编码

- Median-Cut 量化，k-d 树最近色查找（加速比 2.57×）
- 三种抖动算法：None / Floyd-Steinberg / Ordered
- 相邻帧调色板复用，减少 PDS 开销
- 完整 PGS 显示集（PCS/WDS/PDS/ODS），NTSC 1001/1000 因子
- PotPlayer `MAX_OBJECT_REFS=2` 兼容：chunks(2) 自动拆分多对象显示集
- 淡入淡出 PDS-only 优化（无需 ODS 重绘）
- 并行量化（rayon，opt-in）

### 输出

- SUP（`.sup`）：蓝光原盘字幕流
- BDN XML + PNG：蓝光母版 XML 描述符
- SRT 降级：ASS → SRT 调试输出

---

## 🏃 快速开始

```bash
# 单文件转换
ass2sup input.ass -o output.sup

# 转换时校验
ass2sup input.ass -o output.sup --validate --overlap-warn

# 批量转换整季
ass2sup s01/*.ass -d ./sup_output/ --parallel
```

---

## 🏗️ 架构总览

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
         │  调色板复用 · Floyd-Steinberg/Ordered/None 抖动           │
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

## 📂 工作区结构

### 主要工作区（8 crates）

| Crate | 职责 | 关键依赖 | 文档检查 |
|---|---|---|---|
| **`ass-core`** | ASS/SSA/SRT 解析，强类型 AST | thiserror, tracing | `unsafe_code = "deny"` |
| **`subtitle-validator`** | 语法校验、事件重叠检测 | ass-core, thiserror | `#![warn(missing_docs)]` |
| **`subtitle-renderer`** | [native] RGBA 位图渲染 | swash, tiny-skia, wide, parking_lot | — |
| **`libass-sys`** | [libass] libass v0.17 FFI 绑定（纯头文件） | — | — |
| **`subtitle-renderer-libass`** | [libass] libass 渲染管线 | libass-sys, color-quantizer, pgs-encoder, bdn-xml | `#![warn(missing_docs)]` |
| **`color-quantizer`** | RGBA → 索引色，k-d 树加速 | thiserror, tracing | `#![warn(missing_docs)]` |
| **`pgs-encoder`** | 量化帧 → PGS/SUP（DDD：domain/ + encoding/） | color-quantizer, png | — |
| **`bdn-xml`** | 蓝光母版 XML + PNG | quick-xml, png | — |
| **`ass2sup-cli`** | `ass2sup` 二进制，feature-gated 后端分发 | clap, rayon, indicatif, serde | `#![warn(missing_docs)]` |

### 独立工作区

- `ass2sup-libass/` — libass-only 构建的独立 Cargo 工作区（不与主工作区共享）

---

## 🔧 安装

### 前置依赖

- **Rust 1.89+**（[rustup](https://rustup.rs/)）
- Linux native-backend：`sudo apt install libfontconfig1-dev fonts-dejavu-core`
- Linux libass-backend：`sudo apt install libass9`
- macOS：`brew install libass`

### 从源码构建

```bash
git clone https://github.com/UnforgetMemory/um-ass2sup.git
cd um-ass2sup
cargo build --release
```

产物位于 `target/release/ass2sup`。

### 安装到 `$PATH`

```bash
cargo install --path crates/ass2sup-cli --locked
```

---

## 📖 使用方式

### 单文件转换

```bash
ass2sup input.ass -o output.sup
# 自定义分辨率和帧率
ass2sup input.ass -o output.sup -r 1280x720 -f 25.0
# 指定渲染后端（双后端构建时）
ass2sup input.ass -o output.sup --backend libass
```

### 批量转换

```bash
ass2sup *.srt -d ./out/
ass2sup --glob "subs/**/*.ass" --recursive -d ./out/
ass2sup --glob "subs/**/*.ass" --recursive --parallel -d ./out/
```

### 校验与降级

```bash
# 仅校验（CI 友好，退出码 0/1）
ass2sup input.ass --check
# 校验 + 重叠警告
ass2sup input.ass --check --validate --overlap-warn --overlap-mode strict
# ASS → SRT
ass2sup input.ass --to-srt -o output.srt
# SRT 自检
ass2sup input.srt --to-srt -o out.srt && diff input.srt out.srt
```

### 蓝光母版（BDN XML）

```bash
ass2sup input.ass --to-bdn -d ./bdn_out/
```

产出：

```
bdn_out/
└── input/
    ├── BDN.xml
    ├── 0001.png
    ├── 0002.png
    └── ...
```

### 多核加速

```bash
# 单文件内并行量化
ass2sup input.ass -o output.sup --parallel-frames
# 批量文件并行
ass2sup --glob "subs/**/*.ass" --parallel -d ./out/
```

---

## 📝 CLI 参考

| 选项 | 说明 | 默认值 |
|---|---|---|
| `-o, --output <OUTPUT>` | 输出 SUP 路径（单文件） | — |
| `-d, --output-dir <DIR>` | 输出目录（批量） | — |
| `-r, --resolution <WxH>` | 显示分辨率 | `1920x1080` |
| `-f, --fps <FLOAT>` | 帧率 | `23.976` |
| `--backend <BACKEND>` | 渲染后端 `native` / `libass`（双后端构建时） | `native` |
| `--validate` | 转换前校验 | off |
| `--overlap-warn` | 事件重叠检测 | off |
| `--overlap-mode <MODE>` | 重叠模式 `strict` / `lenient` | `lenient` |
| `--quantizer <ALGO>` | 量化算法 | `median-cut` |
| `--max-colors <1-255>` | 调色板最大颜色数 | `255` |
| `--dither <METHOD>` | 抖动算法 | `floyd-steinberg` |
| `--check` | 仅校验，不写文件（退出码 0/1） | off |
| `--to-srt` | 输出 SRT | off |
| `--to-bdn` | 输出 BDN XML + PNG | off |
| `--parallel-frames` | 单文件并行量化 | off |
| `--parallel` | 批量文件并行 | off |
| `--dry-run` | 仅校验，不写入 | off |
| `--force` | 校验失败仍继续转换 | off |
| `--font <NAME>` | SRT 输入默认字体 | `Arial` |
| `--font-size <PT>` | SRT 输入默认字号 | `48.0` |
| `--glob <PATTERN>` | 输入通配符模式 | — |
| `--recursive` | `--glob` 模式下递归搜索 | off |
| `--max-files <N>` | glob 模式最大文件数 | 不限 |
| `--quiet` | 禁用进度条 | off |
| `--color <MODE>` | 颜色输出 `auto` / `always` / `never` | `auto` |
| `-v, --verbose` | 详细日志输出 | off |
| `-h, --help` | 显示帮助信息 | — |
| `-V, --version` | 显示版本号 | — |

> 输入文件超过 **100 MiB** 会被拒绝（`MAX_INPUT_SIZE_BYTES`），防止误传视频文件。

---

## 📦 作为 Rust 库使用

每个 crate 均可独立复用。在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
ass-core            = "2.7"
subtitle-validator  = "2.7"
subtitle-renderer   = { version = "2.7", features = ["..."] }
color-quantizer     = "2.7"
pgs-encoder         = "2.7"
bdn-xml             = "2.7"
```

或使用 path 依赖：

```toml
[dependencies]
ass-core = { path = "../ass2sup/crates/ass-core" }
```

### 解析 + 校验示例

```rust
use ass_core::AssFile;
use subtitle_validator::{validate, ValidationStage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string("input.ass")?;
    let ass  = AssFile::parse(&text)?;
    let report = validate(&ass, ValidationStage::Full);
    if report.has_errors() {
        eprintln!("校验失败: {}", report);
        std::process::exit(1);
    }
    println!("OK: {} 个事件", ass.events.len());
    Ok(())
}
```

### 更多运行示例

```bash
cargo run --example parse_ass       -p ass-core
cargo run --example quantize_image  -p color-quantizer
cargo run --example encode_sup      -p pgs-encoder
```

---

## 📊 性能与基准

完整数据见 [BENCHMARKS.md](BENCHMARKS.md)。代表值（Linux / Rust 1.89）：

| 基准 | 规模 | 中位耗时 | 备注 |
|---|---|---|---|
| `rle_small_64x32` | 64×32 | 2.84 µs | 单段 RLE |
| `rle_large_1920x1080` | 1080p | 2.45 ms | 单段 RLE |
| `quantizer_medium_320x180` | 320×180 | 13.1 ms | 量化 + 抖动 + 调色板 |
| `quantizer_large_1920x1080` | 1080p | 353 ms | k-d 树加速后（2.57×） |
| `pgs_encode_medium_320x180` | 320×180 | 90.3 µs | PGS 编码 |
| `pgs_encode_ntsc_320x180` | 320×180 | 91.1 µs | NTSC 1001/1000 因子 |

```bash
cargo bench --workspace
```

---

## 🧪 测试与质量

- **700+ 单元/集成测试**（`cargo test --workspace`，全部通过）
- **proptest**：ass-core（解析确定性、SRT 往返、ASS 宽松恢复）、color-quantizer、pgs-encoder、bdn-xml
- **insta 快照测试**：`crates/ass2sup-cli/tests/snapshots/`
- **cargo-fuzz**：ass-core（3 目标）、color-quantizer（1）、pgs-encoder（1）
- **criterion 基准**：`cargo bench --workspace`（HTML 报告）
- **clippy `-D warnings`** — 零警告
- **`cargo fmt --all -- --check`** — 无漂移
- **`#[expect(clippy::*)]`** — 优先于 `#[allow(clippy::*)]`

### 完备验证命令

```bash
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo bench --workspace --no-run
cargo doc --workspace --no-deps
```

---

## 🛡️ 安全

- **SECURITY.md**：漏洞上报请通过 GitHub Security Advisories，**勿**开公开 issue
- **deny.toml**：cargo-deny 规则（advisories / bans / licenses / sources）
- **audit.yml**：每周一 06:00 UTC + push/PR 自动安全审计
- 已知忽略：`RUSTSEC-2025-0119`（`number_prefix` 无人维护，通过 `indicatif` 间接引入）

详见 [SECURITY.md](SECURITY.md)。

---

## 🤝 贡献

欢迎提交 PR 和 Issue。提交前请确认：

- [ ] `cargo test --workspace` 全部通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 零警告
- [ ] `cargo doc --workspace --no-deps` 零缺失文档
- [ ] `cargo fmt --all -- --check` 无漂移
- [ ] 新公开 API 包含 `///` rustdoc
- [ ] `CHANGELOG.md` 已更新

---

## 📄 许可证

本项目基于 [`Apache-2.0`](LICENSE-APACHE) 许可。

```
Copyright (c) 2024-2026 The um-ass2sup authors
```

详见 `LICENSE-APACHE`。

---

## 🙏 致谢

构建于以下优秀项目之上：

### Rust 生态

| 项目 | 用途 |
|---|---|
| [`swash`](https://github.com/dfrg/swash) | 字形塑形与光栅化 |
| [`tiny-skia`](https://github.com/RazrFalcon/tiny-skia) | 纯 Rust Skia 位图合成 |
| [`clap`](https://github.com/clap-rs/clap) | CLI 参数解析 |
| [`rayon`](https://github.com/rayon-rs/rayon) | 数据并行 |
| [`wide`](https://github.com/lokathor/wide) | SIMD 加速 |
| [`parking_lot`](https://github.com/Amanieu/parking_lot) | 高效互斥锁 |
| [`quick-xml`](https://github.com/tafia/quick-xml) | XML 序列化 |
| [`png`](https://github.com/image-rs/image-png) | PNG 编码 |
| [`criterion`](https://github.com/bheisler/criterion.rs) | 性能基准 |
| [`proptest`](https://github.com/proptest-rs/proptest) | 属性测试 |
| [`indicatif`](https://github.com/console-rs/indicatif) | 进度条 |

### 外部库

| 项目 | 用途 |
|---|---|
| [`libass`](https://github.com/libass/libass) | ASS 字幕渲染器（v0.17+，可选后端） |
| [`fontconfig`](https://www.freedesktop.org/wiki/Software/fontconfig/) | 字体发现（Linux） |

### 蓝光标准参考

- [Blu-ray Disc Read-Only Format](https://www.blu-raydisc.info/) — PGS/SUP 规范

感谢所有 [贡献者](https://github.com/UnforgetMemory/um-ass2sup/graphs/contributors)。

---

<p align="center">
  <sub>用 <code>cargo</code> 构建 · 基于 <code>main</code> 分支维护</sub>
</p>
