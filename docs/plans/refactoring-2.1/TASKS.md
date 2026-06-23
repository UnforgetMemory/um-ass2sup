# 重构 2.1 — 原子任务分解

> **新 crate `ass-core`**，从零构建，分支 `v2.1/rewrite`。
> 旧 `ass-parser` 保持不动直到合并。

## Wave 0: 初始化（~30min）

- `git checkout -b v2.1/rewrite`
- `cargo init crates/ass-core --lib`
- 编辑 `Cargo.toml`：`thiserror` + `tracing`（运行时）、`proptest` + `insta` + `criterion`（开发）
- 添加 `ass-core = { path = "crates/ass-core" }` 到 workspace `Cargo.toml`

## Wave 1: 核心数据模型（5任务并行）

| # | 任务 | 模块 | 产出 | 关键测试 |
|---|------|------|------|---------|
| 1.1 | time/ 模块 | `time/` | Fps 有理数 + Timestamp + ms_to_90khz + 5种时间格式 | `ms_to_90khz(1000)==90000` 属性测试 |
| 1.2 | Span + Error | `span.rs`, `error.rs` | Span, ParseError, Warning 类型 | 编译通过，clippy 零警告 |
| 1.3 | AssColor | `color.rs` | 从旧 crate 迁移 + 增强 | 10 个现有测试移植 |
| 1.4 | SubtitleDocument | `lib.rs` | 产品类型 + SubtitleFormat | 编译通过 |
| 1.5 | Effect + Karaoke | `effect.rs`, `karaoke.rs` | 从旧 crate 迁移 | 现有测试移植 |

## Wave 2: 解析器核心（3任务并行）

| # | 任务 | 模块 | 产出 | 关键测试 |
|---|------|------|------|---------|
| 2.1 | Lexer | `lexer.rs` | Token 流 + Section 识别 | 122 fixtures Token 计数或等价验证 |
| 2.2 | OverrideTagger | `override_tag.rs` | libass 语义等价标签解析（60+ 标签） | 逐项验证 TAG_MATRIX.md |
| 2.3 | SRT 解析 | `srt.rs` | 独立 SRT 解析（输出 SubtitleDocument） | 往返测试 |

## Wave 3: 完整管线（依赖 Wave 2）

| # | 任务 | 模块 | 产出 | 关键测试 |
|---|------|------|------|---------|
| 3.1 | Section 解析 | `section.rs` | ScriptInfo/Style/Event/Font | 基本 ASS 文件解析 |
| 3.2 | 解析入口 | `lib.rs` | parse()/parse_lenient/parse_with_recovery | 122 libass fixtures 全通过 |
| 3.3 | 消除 unwrap_or | 全部 | 零静默吞数据 | grep unwrap_or = 0 |

## Wave 4: 质量加固

| # | 任务 | 模块 | 产出 | 关键测试 |
|---|------|------|------|---------|
| 4.1 | Proptest | `tests/` | 确定性/往返/不 panic 属性 | 256×10 用例通过 |
| 4.2 | Fuzz | `fuzz/` | 4 targets | 各 5min 无崩溃 |
| 4.3 | 基准 | `benches/` | 与旧 ass-parser 对比 | 性能退化 ≤5% |

## 退出标准

```
□ ass-core 是全新 crate，不含 ass-parser 旧代码
□ 所有 60+ OverrideTag 变体正确解析（TAG_MATRIX.md）
□ libass 边缘情况全部覆盖
□ 零 unwrap_or(default) 在解析路径
□ 每个 ParseError/Warning 带 Span
□ margin_l/r/v 用 Option 表示「未设置」
□ text_raw 保留原始文本，text_display 提供 \N→\n 版本
□ time/ 模块：纯整数 ms_to_90khz + Fps + 5 种时间格式
□ 122 libass fixtures 解析通过（快照新基线）
□ 4 fuzz targets 各 5min 无崩溃
□ 旧 ass-parser 测试全部通过（回归护栏）
```
