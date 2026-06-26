# ass2sup

[![CI](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml/badge.svg)](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml)
[![Audit](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml/badge.svg)](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml)
[![Release](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/release.yml/badge.svg)](https://github.com/UnforgetMemory/um-ass2sup/releases)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE-APACHE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.5.0-blue.svg)](https://github.com/UnforgetMemory/um-ass2sup/releases)
[![Coverage](https://img.shields.io/badge/coverage-88.13%25-brightgreen.svg)](COVERAGE.md)

[English](README.en.md) | **简体中文**

> 一款用 Rust 编写的字幕转换器，将 **ASS / SSA / SRT** 字幕转换为蓝光 **SUP / PGS** 图文流，并支持 **BDN XML** 蓝光母版输出。

---

## 目录

- [这是什么](#这是什么)
- [核心特性](#核心特性)
- [快速开始](#快速开始)
- [架构总览](#架构总览)
- [工作区结构](#工作区结构)
- [安装](#安装)
- [使用方式](#使用方式)
  - [单文件转换](#单文件转换)
  - [批量转换](#批量转换)
  - [校验与降级](#校验与降级)
  - [蓝光母版（BDN XML）](#蓝光母版bdn-xml)
  - [多核加速](#多核加速)
- [命令行选项](#命令行选项)
- [作为 Rust 库使用](#作为-rust-库使用)
- [性能与基准](#性能与基准)
- [测试与质量保障](#测试与质量保障)
- [安全](#安全)
- [贡献](#贡献)
- [许可证](#许可证)
- [致谢](#致谢)

---

## 这是什么

`ass2sup` 是一个**模块化 Rust 工作区**，专注于把开源字幕（ASS / SSA / SRT）转换为蓝光播放所需的位图字幕流（PGS / SUP）以及蓝光母版（BDN XML）格式。

**典型应用场景：**
- 给家庭蓝光原盘（BDMV）烧录字模替换后的多语字幕轨
- 自动化流水线中处理成百上千集番剧字幕
- 对 ASS 特效（卡拉 OK、`\move`、`\fad`、`\t` 等）做精准的时间轴再现
- 需要 23.976 / 29.97 等非整数帧率下的逐帧 PTS 校准

**与同类工具的差异：**
- **真·Rust 原生**：无 Python / Node 依赖，**单二进制**即可部署
- **模块化工作区**：6 个独立 crate，渲染、量化、编码可单独复用
- **k-d 树加速**：1080p 量化从 908 ms 降至 353 ms（2.57×）
- **蓝光合规**：精确处理 NTSC 1001/1000 因子、多窗口分割、EPG 显示集拆分
- **测试与模糊一应俱全**：350+ 单元/集成测试、proptest、insta 快照、cargo-fuzz

---

## 核心特性

### 输入与解析
- **多格式**：ASS v4+、SSA v4、SubRip（`.srt`）自动识别（`SubtitleFormat::detect`）
- **完整 AST**：保留 Style、Dialogue、Font、Embedded Font 全部信息
- **SRT 自检**：`ass2sup in.srt --to-srt -o out.srt && diff in.srt out.srt` 即可验证解析器+序列化器无损

### 渲染
- **字形塑形**：基于 `fontdb` + `rustybuzz`（HarfBuzz 的 Rust 绑定），完整支持复杂文字（中日韩、阿拉伯、印度等）
- **6 级字体回退链**：用户指定 → ASS `[Fonts]` 嵌入字体 → 系统 fontconfig
- **ASS 特效**：卡拉 OK（`\k` / `\kf` / `\ko` / `\kt`）、运动（`\move`）、淡入淡出（`\fad` / `\fade`）、变换（`\t`）、3D 旋转（`\frx` / `\fry`）、各向异性边框、矢量裁剪（`\clip` / `\iclip`）、滚动横幅
- **小调色板去重**：`HashSet<u32>` 优化，O(n²) → O(n)

### 量化与编码
- **Median-Cut 量化器**：内建 k-d 树查找（`find_nearest_index`）
- **三种抖动**：None / Floyd-Steinberg / Ordered
- **调色板复用**：相邻帧使用同一调色板，减少 PGS 段头开销
- **PGS 编码**：完整 PCS / WDS / PDS / ODS 显示集，NTSC 1001/1000 因子精确处理
- **多窗口模式**：自动在透明行边界拆分大显示集
- **并行量化**（可选）：rayon 多核，1.36× 加速（30 事件 1080p 压测：366 ms → 270 ms）

### 输出与发布
- **SUP（`.sup`）**：蓝光原盘字幕流
- **BDN XML + PNG**：蓝光母版 XML 描述符（`<Event InTC="..." />` 等）
- **SRT 降级**：ASS → SRT，便于调试与无蓝光设备预览

### 工程化
- **CI / Audit / Release 三个工作流**（`ci.yml` / `audit.yml` / `release.yml`）
- **cargo-deny**：依赖白名单、许可证、来源审计
- **`#![warn(missing_docs)]`** 强制公开项文档
- **clippy `cast_lossless`** 强制无损类型转换
- **88.13% 行覆盖率**（tarpaulin xml 下界）

---

## 快速开始

```bash
# 1. 转换一个字幕文件
ass2sup input.ass -o output.sup

# 2. 转换时做语法校验
ass2sup input.ass -o output.sup --validate --overlap-warn

# 3. 批量转换整季
ass2sup s01/*.ass -d ./sup_output/ --parallel
```

> 想看更多 → [使用方式](#使用方式)

---

## 架构总览

```
            ┌────────────┐
            │  输入文件   │  ASS / SSA / SRT
            └─────┬──────┘
                  │ SubtitleFormat::detect
                  ▼
        ┌────────────────────┐
        │     ass-parser     │  → 强类型 AST（事件、样式、字体）
        └─────────┬──────────┘
                  │ （可选）
                  ▼
        ┌──────────────────────┐
        │  subtitle-validator  │  语法检查 / 事件重叠检测
        └─────────┬────────────┘
                  │
                  ▼
        ┌──────────────────────┐
        │   subtitle-renderer  │  fontdb + rustybuzz → 每帧 RGBA 位图
        └─────────┬────────────┘
                  │ （可选并行：rayon）
                  ▼
        ┌──────────────────────┐
        │    color-quantizer   │  RGBA → 索引色（≤255 色 + alpha）
        └─────────┬────────────┘
                  │ 调色板可复用
                  ▼
        ┌──────────────────────┐
        │     pgs-encoder      │  PGS / SUP 段（PCS / WDS / PDS / ODS）
        └─────────┬────────────┘
                  │
        ┌─────────┴──────────┐
        ▼                    ▼
  ┌──────────┐        ┌────────────┐
  │  .sup    │        │   BDN XML  │  + 0001.png、0002.png……
  └──────────┘        └────────────┘
```

---

## 工作区结构

| Crate                | 职责                                          | 关键依赖                          |
| -------------------- | --------------------------------------------- | --------------------------------- |
| **`ass-parser`**       | 解析 ASS / SSA / SRT，产出强类型 AST          | —                                 |
| **`subtitle-validator`** | 语法校验、样式检查、事件重叠检测              | `ass-parser`                      |
| **`subtitle-renderer`** | 把字幕渲染为 RGBA 位图（含字形塑形、特效）    | `fontdb`、`rustybuzz`、`tiny-skia` |
| **`color-quantizer`**   | RGBA → 索引色（k-d 树加速）                    | `tiny-skia`                       |
| **`pgs-encoder`**       | 量化帧 → PGS / SUP 二进制段                    | —                                 |
| **`bdn-xml`**           | 蓝光母版 XML + PNG 资源                        | `png`、`quick-xml`                |
| **`ass2sup-cli`**       | CLI 总线（`ass2sup` 二进制）                   | 上述所有 + `clap` + `rayon`       |

所有 crate 通过 `[workspace.dependencies]` 集中管理依赖版本，许可证统一为 `Apache-2.0`。

---

## 安装

### 前置依赖

- **Rust 1.75+**（[rustup](https://rustup.rs/)）
- **fontconfig**（Linux 系统库；macOS / Windows 已内建）
  - Debian / Ubuntu: `sudo apt install libfontconfig1-dev`
  - Fedora: `sudo dnf install fontconfig-devel`
  - macOS: `brew install fontconfig`（Homebrew 已默认带）

### 从源码构建

```bash
git clone https://github.com/UnforgetMemory/um-ass2sup.git
cd um-ass2sup
cargo build --release
```

产物：`target/release/ass2sup`（约 3.5 MB，strip 后更小）。

### 安装到 `$PATH`

```bash
cargo install --path crates/ass2sup-cli --locked
```

---

## 使用方式

### 单文件转换

```bash
ass2sup input.ass -o output.sup
```

默认 1920×1080 @ 23.976 fps；可自定义：

```bash
ass2sup input.ass -o output.sup -r 1280x720 -f 25.0
```

### 批量转换

```bash
# 显式列出（shell 通配）
ass2sup *.srt -d ./out/

# 使用 --glob（更安全，支持跨平台）
ass2sup --glob "subs/**/*.ass" --recursive -d ./out/

# 多核并发处理
ass2sup --glob "subs/**/*.ass" --recursive --parallel -d ./out/
```

### 校验与降级

```bash
# 仅校验，不写文件（CI 友好，退出码 0=OK / 1=错误）
ass2sup input.ass --check

# 校验并显示事件重叠警告
ass2sup input.ass --check --validate --overlap-warn --overlap-mode strict

# ASS → SRT 降级
ass2sup input.ass --to-srt -o output.srt

# SRT 自检：diff 应当为空
ass2sup input.srt --to-srt -o out.srt && diff input.srt out.srt
```

### 蓝光母版（BDN XML）

```bash
ass2sup input.srt --to-bdn -d ./bdn_out/
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

`BDN.xml` 形如：

```xml
<?xml version="1.0" encoding="utf-8"?>
<BDN Version="0.93">
  <Description>
    <Name>input</Name>
    <Language>eng</Language>
    <Format VideoFormat="NTSC">
      <Events>
        <Event InTC="00:00:01:01" OutTC="00:00:03:03" Forced="false">
          <Graphic File="0001.png" Area="0,0,1920,1080" />
        </Event>
        …
      </Events>
    </Format>
  </Description>
</BDN>
```

### 多核加速

```bash
# 单文件：并行量化（默认关闭，显式启用）
ass2sup input.ass -o output.sup --parallel-frames

# 批量：并行文件
ass2sup --glob "subs/**/*.srt" --parallel -d ./out/
```

两个加速独立可叠加：批量并行 + 单文件内并行量化。

---

## 命令行选项

| 选项                              | 说明                                       | 默认          |
| --------------------------------- | ------------------------------------------ | ------------- |
| `-o, --output <OUTPUT>`           | 输出 SUP 路径（单文件）                    | —             |
| `-d, --output-dir <DIR>`          | 输出目录（批量）                           | —             |
| `-r, --resolution <WxH>`          | 显示分辨率                                 | `1920x1080`   |
| `-f, --fps <FLOAT>`                | 帧率                                       | `23.976`      |
| `--validate`                       | 转换前运行校验                             | off           |
| `--overlap-warn`                   | 启用事件重叠检测                           | off           |
| `--overlap-mode <MODE>`            | 重叠模式 `strict` / `lenient`              | `lenient`     |
| `--quantizer <ALGO>`              | 量化算法（当前 `median-cut`）              | `median-cut`  |
| `--max-colors <1-255>`            | 调色板最大颜色数                           | `255`         |
| `--dither <METHOD>`               | 抖动 `none` / `floyd-steinberg` / `ordered`| `floyd-steinberg` |
| `--check`                          | 仅解析校验，不写文件（退出码 0/1）         | off           |
| `--to-srt`                         | 输出 SRT 格式（ASS→SRT 降级 / SRT 自检）   | off           |
| `--to-bdn`                         | 输出 BDN XML + PNG（蓝光母版）             | off           |
| `--parallel-frames`               | 并行量化（单文件，rayon）                  | off           |
| `--parallel`                      | 并行文件处理（批量）                       | off           |
| `--dry-run`                       | 仅解析校验，不写                           | off           |
| `--force`                          | 校验失败仍继续转换                         | off           |
| `--font <NAME>`                   | SRT 输入默认字体                           | `Arial`       |
| `--font-size <PT>`                | SRT 输入默认字号                           | `48.0`        |
| `--glob <PATTERN>`                | 输入文件通配模式                           | —             |
| `--recursive`                      | `--glob` 模式下递归目录                    | off           |
| `--max-files <N>`                  | `--glob` 模式最大处理文件数                 | 无限          |
| `--quiet`                          | 禁用进度条                                 | off           |
| `--color <MODE>`                  | 颜色输出 `auto` / `always` / `never`        | `auto`        |
| `-v, --verbose`                    | 启用详细日志                               | off           |
| `-h, --help`                       | 打印帮助                                   | —             |
| `-V, --version`                    | 打印版本                                   | —             |

输入文件**超过 100 MiB 会被拒绝**（`MAX_INPUT_SIZE_BYTES`），防止误传视频等大文件。如确需调整请改源码。

---

## 作为 Rust 库使用

工作区每个 crate 都是**独立可复用的库**。`Cargo.toml`：

```toml
[dependencies]
ass-parser        = "0.5"
subtitle-validator = "0.5"
subtitle-renderer = { version = "0.5", features = ["..."] }
color-quantizer   = "0.5"
pgs-encoder       = "0.5"
bdn-xml           = "0.5"
```

或 path 依赖：

```toml
[dependencies]
ass-parser = { path = "../ass2sup/crates/ass-parser" }
```

简单示例：解析 + 校验

```rust
use ass_parser::AssFile;
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

更多范例见 `crates/*/examples/`：

```bash
cargo run --example parse_ass       -p ass-parser
cargo run --example quantize_image  -p color-quantizer
cargo run --example encode_sup      -p pgs-encoder
```

---

## 性能与基准

详细数据见 [BENCHMARKS.md](BENCHMARKS.md)。代表性数据（Linux WSL2 / Rust 1.77）：

| 基准                       | 规模      | 中位耗时     | 备注                       |
| -------------------------- | --------- | ------------ | -------------------------- |
| `rle_small_64x32`            | 64×32     | 2.84 µs      | 单段 RLE                   |
| `rle_large_1920x1080`        | 1080p     | 2.45 ms      | 单段 RLE                   |
| `quantizer_medium_320x180`   | 320×180   | 13.1 ms      | 量化（抖动+调色板）        |
| `quantizer_large_1920x1080`  | 1080p     | 908 ms       | k-d 树加速后 353 ms（2.57×） |
| `pgs_encode_medium_320x180`  | 320×180   | 90.3 µs      | PGS 编码                   |
| `pgs_encode_ntsc_320x180`    | 320×180   | 91.1 µs      | NTSC 1001/1000 因子         |

复现：

```bash
cargo bench --workspace
```

---

## 测试与质量保障

- **350+ 单元/集成测试**（`cargo test --workspace`）
- **proptest** 属性测试（ass-parser 解析确定性、SRT 往返、ASS 宽松模式恢复等）
- **insta 快照**（`crates/ass2sup-cli/tests/snapshots/`）覆盖 CLI 输出
- **cargo-fuzz** 两个目标（`decode_pgs`、`quantize_rgba`）—— P26 通过 fuzz 找到 2 个 PGS 解码 OOB bug 已修
- **88.13% 行覆盖率**（cargo-tarpaulin）

运行全部：

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo fmt --all -- --check
```

> 详细见 [COVERAGE.md](COVERAGE.md)。架构决策见 [`docs/adr/`](docs/adr/)。

---

## 安全

- **`SECURITY.md`**：漏洞上报流程（请走 GitHub Security Advisories 而**非**公开 issue）
- **`deny.toml`**：cargo-deny 审计（advisories / bans / licenses / sources）
- **`.github/workflows/audit.yml`**：每周一 06:00 UTC + push/PR 自动审计

当前已知警告：忽略 `RUSTSEC-2025-0119`（`number_prefix` 无维护，间接经 `indicatif 0.17.11` 引入，等上游修）。

详见 [SECURITY.md](SECURITY.md)。

---

## 贡献

欢迎 PR 与 Issue。开发流程建议：

```bash
# 1. 克隆与构建
git clone https://github.com/UnforgetMemory/um-ass2sup.git
cd um-ass2sup
cargo build

# 2. 跑全部门
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo fmt --all -- --check

# 3. 新增 crate / 文件请同步更新根 README 与 CHANGELOG
```

提交前请确保：
- [ ] 所有测试通过
- [ ] clippy 零警告（`-- -D warnings`）
- [ ] `cargo doc` 零缺失文档警告
- [ ] `cargo fmt` 无漂移
- [ ] 新公开项有 `///` rustdoc
- [ ] `CHANGELOG.md` 已更新

---

## 许可证

**双许可**：[`Apache-2.0`](LICENSE-APACHE)

```
Copyright (c) 2024-2026 The um-ass2sup authors
```

详细条款见 `LICENSE-APACHE` 文件。

---

## 致谢

构建于以下优秀开源项目之上：

- [`rustybuzz`](https://github.com/RazrFalcon/rustybuzz) — HarfBuzz 的 Rust 绑定
- [`tiny-skia`](https://github.com/RazrFalcon/tiny-skia) — 纯 Rust Skia 绑定
- [`fontdb`](https://github.com/RazrFalcon/fontdb) — 字体数据库
- [`clap`](https://github.com/clap-rs/clap) — CLI 参数解析
- [`rayon`](https://github.com/rayon-rs/rayon) — 数据并行
- 所有 [依赖列表](Cargo.toml) 中的 crate

也感谢所有 [贡献者](https://github.com/UnforgetMemory/um-ass2sup/graphs/contributors)。

---

<p align="center">
  <sub>用 <code>cargo</code> 构建 · 提交于 <code>master</code></sub>
</p>
