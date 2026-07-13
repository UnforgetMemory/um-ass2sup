# um-ass2sup 中文 Wiki

> ASS/SSA/SRT → Blu-ray SUP/PGS 字幕转换器  
> Rust 工作区 · **8 crates** · **v3.0.0** · **双渲染后端**

---

## 📖 快速导航

| 页面 | 内容 |
|------|------|
| 🏠 **[首页](index.md)** | 项目概览、特性速览、入门指引 |
| 🏗️ **[架构详解](architecture.md)** | 管线流程、crate 职责、数据流、内存模型 |
| ⚡ **[双渲染后端对比](rendering-backends.md)** | native-backend (swash) vs libass-backend 对比 |
| 🛠️ **[开发指南](development.md)** | 构建、测试、CI 工作流、贡献指南 |
| 🧩 **[PGS 编码器设计](pgs-encoder.md)** | DDD 架构：domain/ + encoding/ 分离 |
| 🎨 **[色彩量化管线](color-quantizer.md)** | 颜色科学、量化、抖动、帧抽象 |
| 🔤 **[字体子系统](font-system.md)** | FontRegistry、塑形、光栅化、8 级回退链 |

---

## 🎯 项目是什么

`ass2sup` 是一个用 Rust 编写的命令行工具，将开源字幕格式（ASS/SSA/SRT）转换为蓝光播放器原生支持的位图字幕流（PGS/SUP），同时支持 BDN XML + PNG 母版输出。

### 典型使用场景

- **自制 BDMV** — 替换或追加多语字幕轨
- **批量自动化** — 整季番剧的字幕转换流水线
- **特效保留** — ASS `\move`、`\fad`、`\t`、卡拉 OK 等全部特效
- **精确帧率** — 23.976/29.97 等非整数帧率逐帧 PTS 校准

---

## ⚡ 快速开始

### 安装

```bash
# 前置：Rust 1.85+
# Linux native-backend:
sudo apt install libfontconfig1-dev fonts-dejavu-core

# Linux libass-backend:
sudo apt install libass9

# 从源码构建
git clone https://github.com/UnforgetMemory/um-ass2sup.git
cd um-ass2sup
cargo build --release

# 产物：target/release/ass2sup
# 或安装到 PATH：
cargo install --path crates/ass2sup-cli --locked
```

### 基本使用

```bash
# 单文件转换
ass2sup input.ass -o output.sup

# 自定义分辨率与帧率
ass2sup input.ass -o output.sup -r 1280x720 -f 25.0

# 批量转换整季
ass2sup s01/*.ass -d ./sup_output/ --parallel

# 输出 BDN XML 母版
ass2sup input.ass --to-bdn -d ./bdn_out/
```

---

## ✨ 核心特性

### 输入与解析
- ASS v4+、SSA v4、SubRip (`.srt`) 自动识别
- 手写解析器，零外部解析依赖
- 完整 AST，保留 Style/Dialogue/Font 全部信息
- SRT 往返自检：`ass2sup in.srt --to-srt -o out.srt && diff`

### 渲染
- **native-backend**：swash 字形塑形 + tiny-skia 位图合成，零 C 依赖
- **libass-backend**：通过 FFI 调用系统 libass，完美规范兼容
- 全面 ASS 特效：卡拉 OK、`\move`、`\fad`/`\fade`、`\t`、3D 旋转、各向异性边框、矢量裁剪、滚动横幅

### 量化与编码
- Median-Cut 量化，k-d 树最近色查找（2.57× 加速）
- 三种抖动算法：None / Floyd-Steinberg / Ordered Bayer
- 相邻帧调色板复用，减少 PDS 开销
- PotPlayer `MAX_OBJECT_REFS=2` 兼容
- 淡入淡出 PDS-only 优化
- 并行量化（rayon，opt-in）

### 输出
- SUP (`.sup`) — 蓝光原盘字幕流
- BDN XML + PNG — 蓝光母版 XML 描述符
- SRT 降级 — ASS → SRT 调试输出

---

## 🏗️ 工作区结构（8 crates）

| Crate | 职责 | 关键依赖 |
|-------|------|----------|
| **`ass-core`** | ASS/SSA/SRT 解析 → 强类型 AST | thiserror, tracing |
| **`subtitle-validator`** | 语法校验、事件重叠检测 | ass-core |
| **`subtitle-renderer`** | [native] RGBA 位图渲染 | swash, tiny-skia, wide |
| **`libass-sys`** | [libass] libass v0.17 FFI 绑定 | — (header-only) |
| **`subtitle-renderer-libass`** | [libass] libass 渲染管线 | libass-sys |
| **`color-quantizer`** | RGBA → 索引色，k-d 树加速 | thiserror, tracing |
| **`pgs-encoder`** | 量化帧 → PGS/SUP (DDD 架构) | color-quantizer, png |
| **`bdn-xml`** | 蓝光母版 XML + PNG | quick-xml, png |
| **`ass2sup-cli`** | 二进制 CLI | clap, rayon, indicatif |

---

## 🎯 质量门禁

| 维度 | 标准 |
|------|------|
| MSRV | Rust 1.85 |
| 测试 | 700+ 单元/集成测试，2 ignored |
| 属性测试 | proptest：ass-core / color-quantizer / pgs-encoder / bdn-xml |
| 基准 | criterion，HTML 报告 |
| Clippy | `-D warnings` 零警告 |
| Fmt | `cargo fmt --all -- --check` 无漂移 |
| 模糊测试 | ass-core (3) / color-quantizer (1) / pgs-encoder (1) |
| 文档 | 4/8 crates `#![warn(missing_docs)]` |

---

## 🔗 相关链接

- [GitHub 仓库](https://github.com/UnforgetMemory/um-ass2sup)
- [CI](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml)
- [Release](https://github.com/UnforgetMemory/um-ass2sup/releases)
- 蓝光规范：[Blu-ray Disc Read-Only Format](https://www.blu-raydisc.info/)

---

<p align="center">
  <sub>用 <code>cargo</code> 构建 · 跟踪于 <code>master</code></sub>
</p>
