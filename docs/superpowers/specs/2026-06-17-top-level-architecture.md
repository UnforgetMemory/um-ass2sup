# ass2sup v2.0 全面架构重构 — 顶层架构 Spec

**日期**: 2026-06-17
**状态**: Draft (Brainstorming Phase)
**作者**: Sisyphus (with user collaboration)
**目标版本**: v1.0.0 (从 v0.5.5 跃升主版本号以反映架构断裂)

---

## 1. 背景与动机

v0.5.x 的字体/解析/渲染管线在 Windows 上因 fontdb 模糊匹配 + ttf-parser 慢路径产生 CJK tofu 回归。v0.5.5 移除了 `font_has_cjk_glyphs` 解决了 hang，但引入了 CJK 字体选择回退到拉丁字体的回归。

更深层的问题：
- 4 个字体相关库（fontdb / ttf-parser / rustybuzz / tiny-skia）接口碎片化
- ASS 解析对 V4+ Styles 22 个字段、override tag 完整语法支持不完整
- 缺少 HDR 输出（BT.2020 + PQ）
- 缺少 GPU 加速路径
- CLI 错误信息不可操作

本 spec 定义 v2.0 的**完整架构**与**子项目拆分**，作为后续 9 份子 spec 的总纲。

## 2. 设计原则

| 原则 | 含义 |
|------|------|
| **数据流单向性** | Input → Core → Output，无回环 |
| **可插拔性** | 字体系统、渲染后端、输出格式都是 trait 抽象 |
| **错误可恢复** | parser 容错，渲染器降级，输出器隔离 |
| **配置驱动** | 全部行为可通过 `ass2sup.toml` 配置 |
| **跨平台一致** | 同一份输入在三平台产生像素级接近的输出 |
| **依赖最小化** | 仅引入必要 crate，pin 稳定版本 |

## 3. 顶层架构

```
ass2sup-cli (v2)
├── CLI Frontend Layer (clap + 自研 REPL)
│   ├── 命令行模式 (现有)
│   ├── 交互模式 (REPL)        [Sub-8]
│   ├── 配置文件 (TOML)        [Sub-1]
│   └── 智能错误诊断           [Sub-8]
└── Processing Pipeline
    ├── Input Layer
    │   ├── ASS/SSA Parser    [Sub-2]
    │   ├── SRT Parser
    │   ├── Format Detection
    │   └── Validation + Error Recovery
    ├── Core Engine
    │   ├── Font Resolver     [Sub-3]  (cosmic-text Fallback impl)
    │   ├── Text Layout       [Sub-4]  (cosmic-text Buffer)
    │   ├── Renderer Backend  [Sub-4 + Sub-6]
    │   │   ├── CPU (tiny-skia)
    │   │   └── GPU (wgpu/vello)
    │   ├── Effects/Animation [Sub-4]
    │   └── Color Pipeline    [Sub-5]  (SDR↔HDR)
    └── Output Layer
        ├── PGS / SUP
        ├── BDN XML + PNG
        ├── TTML (HDR/SDR)     [Sub-7]
        ├── WebVTT             [Sub-7]
        └── Plugin Sinks       [Sub-7]
```

## 4. 子项目依赖图

```
Sub-1 基础 ──┬──▶ Sub-2 ASS 解析 ──┬──▶ Sub-3 字体 ──┬──▶ Sub-4 渲染 ──┬──▶ Sub-5 色彩 ──┬──▶ Sub-7 输出
             │                     │                 │                 │                 │
             │                     │                 │                 │                 ▼
             │                     │                 │                 │              Sub-8 CLI
             │                     │                 │                 │
             │                     │                 └──▶ Sub-6 GPU ◀──┘
             │                     │                       │
             │                     │                       │
             ▼                     ▼                       ▼
                          Sub-9 测试 (横向，全程)
```

## 5. 实施路线图（Sprint 级）

| Sprint | Sub-Project | 周 | 累计 | 关键交付 + 验证门 |
|--------|-------------|-----|------|------------------|
| **S0** | Sub-1 基础设施 | 1-2 | 2 | `ass2sup::Error`, `Config` 系统, telemetry — 现有 440+ 测试通过 |
| **S1** | Sub-2 ASS 解析器 | 2-3 | 5 | V4+ 22 字段完整支持, libass 兼容 — golden 测试 ≥ 95% |
| **S2** | Sub-3 字体引擎 | 1-2 | 7 | CJK 正确 + 跨平台一致 — CJK 视觉回归 0 tofu |
| **S3** | Sub-4 渲染器 | 2-3 | 10 | 像素精度 + 特效完整 — libass 输出像素匹配 ≥ 90% |
| **S4** | Sub-5 色彩管线 | 1-2 | 12 | HDR BT.2020+PQ + HLG — HDR roundtrip 测试 |
| **S5** | Sub-6 GPU | 2-3 | 15 | wgpu 集成 — 10x 加速（> 5000 事件） |
| **S6** | Sub-7 输出格式 | 1-2 | 17 | TTML + WebVTT + 插件 — 格式合规测试 |
| **S7** | Sub-8 CLI | 1-2 | 19 | 交互 REPL + 智能错误 + 配置 — UX 测试 |

**总工期**: 19 周（5 个月单人或 3 个月双人）

## 6. 跨子项目约束

| 约束 | 适用范围 |
|------|----------|
| **MSRV**: 1.88 (跟 parley 一致) | 全部 |
| **零 C 依赖** | 全部（除系统级 font discovery API 间接调用） |
| **`#![warn(missing_docs)]`** | 全部 public item |
| **`cargo clippy -D warnings`** | 全部 |
| **测试覆盖**: 单元 ≥ 80% 行 | Sub-2 起的核心模块 |
| **跨平台 CI**: Win/macOS/Linux | 全部 |
| **依赖更新**: dep-update bot 每月 PR | 全部 |

## 7. 风险与缓解

| 风险 | 影响 | 概率 | 缓解 |
|------|------|------|------|
| cosmic-text API 不稳定 | 高 | 中 | pin 稳定版本，封装 trait 抽象层 |
| GPU ABI 跨平台兼容 | 高 | 中 | 条件编译 + CPU fallback 完整 |
| HDR 标准更新 | 中 | 低 | 跟随 ITU-R BT.2408 / SMPTE ST 2086 现行 |
| 跨平台字体行为差异 | 中 | 高 | 三平台 CI 强制测试 |
| 工期超 6 个月 | 中 | 高 | 每 Sprint 独立可发布 |
| 旧用户配置不兼容 | 中 | 高 | 兼容 `~/.ass2sup.toml` 旧格式 |

## 8. Spec 拆分（本仓库文档树）

```
docs/
├── superpowers/
│   └── specs/
│       ├── 2026-06-17-top-level-architecture.md  ← 本文档
│       ├── 2026-06-17-Sub-1-infrastructure.md
│       ├── 2026-06-17-Sub-2-ass-parser.md
│       ├── 2026-06-17-Sub-3-font-engine.md
│       ├── 2026-06-17-Sub-4-renderer.md
│       ├── 2026-06-17-Sub-5-color-pipeline.md
│       ├── 2026-06-17-Sub-6-gpu.md
│       ├── 2026-06-17-Sub-7-output-formats.md
│       ├── 2026-06-17-Sub-8-cli.md
│       └── 2026-06-17-Sub-9-testing.md
└── plans/
    ├── README.md              ← 路线图总览（任务级 TODO）
    ├── 01-infrastructure/     ← Sub-1 的 task MD 文件
    │   ├── README.md
    │   ├── task-01-error-types.md
    │   ├── task-02-config-system.md
    │   └── task-03-telemetry.md
    ├── 02-ass-parser/         ← Sub-2 的 task MD 文件
    │   ├── ...
    │   (后续 Sprint 落地时填充)
    └── ...
```

## 9. 当前状态

- ✅ v0.5.5 已 commit + push (`59472a5`)，hang 链修复
- ✅ Sub-3 字体引擎研究完成（librarian bg_d80cff50）
- ⏳ Sub-2 ASS 规范研究进行中（librarian bg_563018c9）
- 🔄 本 spec 撰写中

## 10. 下一步

1. 用户 review 本顶层 spec
2. 完成 Sub-2 规范研究后写 Sub-2 spec
3. 用户批准后从 Sub-1 (基础设施) 开始第一个 Sprint
4. 每个 Sprint 走 writing-plans → implementation → review 循环
