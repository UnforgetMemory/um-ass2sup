# 重构 2.1 — ass-parser 全面重写

> ⚠️ 这不是「重构」，这是「用正确的方式重新实现 ASS 解析」。
> 当前下游（渲染器/编码器）成功率只有 10%——用残废的下游做约束没有意义。
> **2.1 的约束条件只有一个：与 libass 语义完全等价。**

## 核心目标

| 工匠 | 模块 | 变化 |
|------|------|------|
| 工匠1 | `lexer.rs` | 新生：Token 流 + Section 识别 |
| 工匠2 | Section parsers | 从 lib.rs 提取，消除 3 个 parse 模式的代码重复 |
| 工匠3 | `override_tag.rs` | 统一解析路径，libass 语义等价（60+ 标签完全覆盖） |
| 工匠4 | `time/` | 新生：Fps 有理数帧率 + 纯整数 ms_to_90khz |
| 工匠5 | `error.rs` + `span.rs` | Span 跟踪，消除 50 处 unwrap_or |

## 不做的

- ❌ 保持 122 snapshots 不变（期望值会变，AST 更完整了）
- ❌ 保持下游 crate 编译通过（下游需要重建）
- ❌ 保持 Event/Style 字段布局不变（新数据模型优先）

## 文档导航

| 文档 | 内容 |
|------|------|
| `PLAN_SPEC.md` | 完整规格：设计原则、工匠模块、解析管线、测试策略、任务分解 |
| `TAG_MATRIX.md` | 50+ OverrideTag 的 libass 语义对照表 |
| `TASKS.md` | 4波次原子任务 + 执行顺序 + 退出标准 |
