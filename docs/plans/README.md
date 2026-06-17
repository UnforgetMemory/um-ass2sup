# ass2sup v2.0 实施路线图

本目录是 ass2sup v2.0 全面重构的**任务级**实施计划。

## 文档结构

```
docs/
├── superpowers/specs/          ← 设计 spec（已完成）
│   ├── 2026-06-17-top-level-architecture.md
│   ├── 2026-06-17-Sub-1-infrastructure.md
│   ├── 2026-06-17-Sub-2-ass-parser.md
│   ├── 2026-06-17-Sub-3-font-engine.md
│   ├── 2026-06-17-Sub-4-renderer.md
│   ├── 2026-06-17-Sub-5-color-pipeline.md
│   ├── 2026-06-17-Sub-6-gpu.md
│   ├── 2026-06-17-Sub-7-output-formats.md
│   ├── 2026-06-17-Sub-8-cli.md
│   └── 2026-06-17-Sub-9-testing.md
└── plans/                       ← 本目录：任务级 MD + 路线图
    ├── README.md                ← 本文件
    ├── roadmap.md               ← 主路线图（所有 Sprint 任务）
    ├── 01-infrastructure/       ← Sub-1 的任务级 MD
    ├── 02-ass-parser/           ← Sub-2 的任务级 MD（占位）
    ├── 03-font-engine/          ← Sub-3 的任务级 MD（占位）
    ├── 04-renderer/             ← Sub-4 的任务级 MD（占位）
    ├── 05-color-pipeline/       ← Sub-5 的任务级 MD（占位）
    ├── 06-gpu/                  ← Sub-6 的任务级 MD（占位）
    ├── 07-output-formats/       ← Sub-7 的任务级 MD（占位）
    ├── 08-cli/                  ← Sub-8 的任务级 MD（占位）
    └── 09-testing/              ← Sub-9 的任务级 MD（占位）
```

## 使用方式

1. **先读** `docs/superpowers/specs/2026-06-17-top-level-architecture.md` 了解全局
2. **看路线图** `docs/plans/roadmap.md` 了解 Sprint 划分与任务依赖
3. **进入实施**时，按 Sprint 顺序展开 `docs/plans/SN-name/` 的 task-*.md
4. **每个 task MD** 包含：目标、依赖、详细步骤、测试要求、验证门

## 当前状态

- ✅ v0.5.5 已 commit + push (`59472a5`)
- ✅ 顶层架构 spec 已写
- ✅ 9 份子 spec 已写
- ✅ 路线图 roadmap.md 已写
- ✅ Sub-1 任务级 MD（基础设施，已就绪可开始）
- 🔄 Sub-2 ~ Sub-9 任务级 MD（Sprint 启动时填充）

## 开始实施

按 roadmap.md 的 Sprint 顺序，从 **Sprint 0 (Sub-1 基础设施)** 开始：

```bash
# 阅读 Sub-1 spec
cat docs/superpowers/specs/2026-06-17-Sub-1-infrastructure.md

# 阅读 Sub-1 任务 MD
ls docs/plans/01-infrastructure/

# 启动 Sub-1
git checkout -b v2.0/sub-1-infrastructure
# 按 task-01, task-02, task-03 顺序实施
```
