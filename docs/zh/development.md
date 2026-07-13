# 🛠️ 开发指南

> 构建、测试、CI 工作流与贡献指南

---

## 📋 目录

- [前置依赖](#前置依赖)
- [构建命令](#构建命令)
- [完整验证流程](#完整验证流程)
- [单 crate 操作](#单-crate-操作)
- [测试体系](#测试体系)
- [基准测试](#基准测试)
- [模糊测试](#模糊测试)
- [CI 工作流](#ci-工作流)
- [质量门禁](#质量门禁)
- [代码规范](#代码规范)
- [贡献指南](#贡献指南)
- [常见问题](#常见问题)

---

## 前置依赖

### Rust 工具链

- **Rust 1.85+**（[rustup](https://rustup.rs/) 安装）
- **Edition 2021**

```bash
rustup update stable
rustc --version  # 确认 ≥ 1.85
```

### 系统依赖

**Linux native-backend：**

```bash
sudo apt install -y libfontconfig1-dev fonts-dejavu-core
```

`libfontconfig1-dev` 提供 fontconfig 头文件（编译 fontconfig-sys）。`fonts-dejavu-core` 提供测试用的 DejaVu 字体。

**Linux libass-backend：**

```bash
sudo apt install libass9
```

`links/` 目录下包含 CI 用预构建 libass 副本；生产使用通过系统包管理器安装。

**macOS libass-backend：**

```bash
brew install libass
```

---

## 构建命令

```bash
# 默认构建（native-backend）
cargo build --release

# libass-backend 仅
cargo build --release --no-default-features -F libass-backend

# 双后端（运行时 --backend 切换）
cargo build --release --no-default-features -F native-backend,libass-backend

# 调试构建
cargo build

# 仅检查编译（不生成二进制）
cargo check --workspace --all-targets
```

产物：`target/release/ass2sup`。

### 安装到 PATH

```bash
cargo install --path crates/ass2sup-cli --locked
```

---

## 完整验证流程

CI 中按此顺序运行全套验证：

```bash
# 1. 检查编译
cargo check --workspace --all-targets

# 2. 格式化检查
cargo fmt --all -- --check

# 3. Clippy 检查（零警告）
cargo clippy --workspace --all-targets -- -D warnings

# 4. 运行测试
cargo test --workspace --all-targets

# 5. 文档测试
cargo test --workspace --doc

# 6. 编译基准（仅编译，不运行）
cargo bench --workspace --no-run

# 7. 生成文档
cargo doc --workspace --no-deps
```

> **注意**：项目没有 Makefile 或 task runner。所有命令直接使用 `cargo` 运行。

---

## 单 crate 操作

在开发特定 crate 时，可以只处理该 crate 以减少编译时间：

```bash
# 运行 ass-core 测试
cargo test -p ass-core

# 运行定量化器 Clippy
cargo clippy -p color-quantizer --all-targets -- -D warnings

# 运行 pgs-encoder 的单个测试
cargo test -p pgs-encoder -- test_rle

# 运行特定示例
cargo run --release --example parse_ass -p ass-core
cargo run --release --example quantize_image -p color-quantizer
cargo run --release --example encode_sup -p pgs-encoder
```

---

## 测试体系

### 概览

- **700+ 单元/集成测试** 跨工作区（全部通过，2 个 ignored）
- **属性测试**（proptest）在 4 个 crate 中
- **快照测试**（insta）在 CLI 测试中
- **模糊测试**（cargo-fuzz）3 个 crate

### proptest 覆盖范围

| crate | 测试范围 |
|-------|---------|
| **ass-core** | 解析确定性、SRT 往返、ASS 宽松恢复 |
| **color-quantizer** | 量化稳定性、调色板边界条件 |
| **pgs-encoder** | 编码往返、RLE 正确性 |
| **bdn-xml** | XML 序列化往返 |

### 快照测试

```bash
# 更新快照（代码变更导致预期输出变化时）
cargo insta review
```

快照存储在 `crates/ass2sup-cli/tests/snapshots/`。

### 运行测试

```bash
# 全部测试
cargo test --workspace

# 全部测试（含 benchmark 编译）
cargo test --workspace --all-targets

# 文档测试
cargo test --workspace --doc

# 过滤测试名
cargo test -p color-quantizer -- quantize

# 显示测试输出
cargo test -- --nocapture
```

---

## 基准测试

[criterion](https://github.com/bheisler/criterion.rs) 基准生成 HTML 报告：

```bash
# 运行所有基准
cargo bench --workspace

# 运行特定基准
cargo bench -p color-quantizer
cargo bench -p pgs-encoder -- rle
```

代表性数据（Linux / Rust 1.85）：

| 基准 | 规模 | 中位耗时 | 备注 |
|------|------|---------|------|
| `rle_small_64x32` | 64×32 | 2.84 µs | 单段 RLE |
| `rle_large_1920x1080` | 1080p | 2.45 ms | 单段 RLE |
| `quantizer_medium_320x180` | 320×180 | 13.1 ms | 量化 + 抖动 + 调色板 |
| `quantizer_large_1920x1080` | 1080p | 353 ms | k-d 树加速后（2.57×） |
| `pgs_encode_medium_320x180` | 320×180 | 90.3 µs | PGS 编码 |
| `pgs_encode_ntsc_320x180` | 320×180 | 91.1 µs | NTSC 1001/1000 因子 |

基准报告位于 `target/criterion/`（HTML）。

---

## 模糊测试

使用 `cargo-fuzz`，模糊目标独立于主工作区（`exclude = ["crates/*/fuzz"]`）：

| crate | 模糊目标数 |
|-------|-----------|
| **ass-core** | 3 |
| **color-quantizer** | 1 |
| **pgs-encoder** | 1 |

```bash
# 运行模糊测试（安装 cargo-fuzz 后）
cd crates/ass-core/fuzz
cargo fuzz run parse_ass
```

---

## CI 工作流

项目使用 GitHub Actions，三个核心工作流：

### ci.yml（每次 push/PR 到 master）

| Job | 内容 |
|-----|------|
| check | `cargo fmt --check` |
| clippy | `cargo clippy -- -D warnings` |
| test | `cargo test --all-targets` + `cargo bench --no-run` |
| msrv | 验证 MSRV 1.85 |

### audit.yml（每周一 06:00 UTC + push/PR）

- `cargo-audit`：已知漏洞扫描，`--deny warnings`
- `cargo-deny`：license 白名单、ban 规则、源码来源检查

已知忽略：`RUSTSEC-2025-0119`（`number_prefix` 无人维护，通过 `indicatif` 间接引入）。

### release.yml（tag push）

- 交叉平台构建矩阵（Linux x86_64/aarch64, macOS ARM, Windows）
- dry-run publish
- GitHub Release

---

## 质量门禁

| 门禁 | 标准 |
|------|------|
| **MSRV** | Rust 1.85（CI 强制） |
| **Edition** | 2021 |
| **clippy** | `-D warnings`（工作区零警告） |
| **fmt** | `cargo fmt --all -- --check`（不允许漂移） |
| **文档** | 4/8 crates `#![warn(missing_docs)]` |
| **unsafe** | ass-core `unsafe_code = "deny"` |
| **Profile** | `opt-level = 3`, `lto = "thin"`, `codegen-units = 1` |
| **依赖审计** | cargo-deny（license whitelist） |
| **漏洞扫描** | cargo-audit（每周 + push/PR） |

---

## 代码规范

### 通用规则

- **禁止** `unwrap()`/`expect()` 在生产代码中（仅允许在测试和 CLI main）
- **优先** `#[expect(clippy::*)]` 而非 `#[allow(clippy::*)]`，并附带理由
- **工作区依赖** 管理在根 `Cargo.toml` 的 `[workspace.dependencies]` 中
- **Fuzz crates** 从工作区排除：`exclude = ["crates/*/fuzz"]`

### 文档要求

4 个 crate 要求 `#![warn(missing_docs)]`：

- **subtitle-validator**
- **subtitle-renderer-libass**
- **color-quantizer**
- **ass2sup-cli**

ass-core 额外禁止 `unsafe_code`。

新公开 API 必须包含 `///` rustdoc 注释。

### 许可证

- Apache-2.0
- 每个源文件头部包含版权声明

---

## 贡献指南

PR 和 Issue 欢迎。提交 PR 前请确认：

### 提交前检查清单

- [ ] `cargo test --workspace` 全部通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 零警告
- [ ] `cargo doc --workspace --no-deps` 零缺失文档
- [ ] `cargo fmt --all -- --check` 无漂移
- [ ] 新公开 API 有 `///` rustdoc
- [ ] `CHANGELOG.md` 已更新

### Issues

- Bug 报告：请包含输入示例、预期行为、实际行为
- 功能请求：清晰描述用例和期望

### 安全漏洞

请通过 **GitHub Security Advisories** 上报，勿开公开 Issue。

详见 [SECURITY.md](https://github.com/UnforgetMemory/um-ass2sup/SECURITY.md)。

---

## 常见问题

### Q: 如何减少增量编译时间？

```bash
# 只 check 不 build
cargo check -p ass-core

# 单 crate 测试
cargo test -p color-quantizer

# 使用 sccache
cargo install sccache
export RUSTC_WRAPPER=sccache
```

### Q: 如何调试渲染问题？

使用 `--to-bdn` 输出 PNG 序列进行像素级检查：

```bash
cargo run --release -p ass2sup-cli -- input.ass --to-bdn -d ./frames/
# 查看 ./frames/input/0001.png ...
```

### Q: 如何测试输出 SUP？

```bash
# 转换
ass2sup input.ass -o test.sup

# 用 ffmpeg 查看（将 SUP 封装为 mkv 测试）
ffmpeg -i video.mkv -i test.sup -c copy -map 0 -map 1 output.mkv
```

### Q: `cargo test` 报 fontconfig 错误？

确认安装了 `libfontconfig1-dev`：

```bash
sudo apt install -y libfontconfig1-dev fonts-dejavu-core
```

---

<p align="center">
  <sub>← [双渲染后端对比](rendering-backends.md) | [返回首页](index.md) | 下一篇：[PGS 编码器设计](pgs-encoder.md) →</sub>
</p>
