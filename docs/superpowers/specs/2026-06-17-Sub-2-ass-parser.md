# Sub-2: ASS 解析器重构 (ASS Parser)

**Sprint**: S1
**周期**: 2-3 周
**依赖**: Sub-1
**阻塞**: Sub-3, Sub-4

## 目标

将 `ass-parser` crate 重写为完整支持 V4+ Styles 22 字段、Events 完整字段、override tag 全部语法（含 \t 动画），并提供 libass 兼容的错误恢复能力。

## 范围

### In Scope
- 完整 V4+ Styles 22 字段解析（Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding）
- 完整 Events 10 字段 + override tag（Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text）
- Override tag 完整语法（含嵌套、动画 `\t(\tag,t1,t2,accel)`）
- 错误恢复：跳过无效行、聚合错误、degraded mode
- SRT → ASS 自动升级路径

### Out of Scope
- 渲染（属于 Sub-4）
- 字体解析（属于 Sub-3）

## 架构决策

### AST 强类型
```rust
pub struct Style {
    pub name: String,
    pub font: FontSpec,           // 强类型 font name + bold/italic
    pub font_size: f32,
    pub primary_colour: Color,
    pub secondary_colour: Color,
    pub outline_colour: Color,
    pub back_colour: Color,
    pub bold: bool, pub italic: bool,
    pub underline: bool, pub strike_out: bool,
    pub scale_x: f32, pub scale_y: f32,
    pub spacing: f32, pub angle: f32,
    pub border_style: BorderStyle,    // 1=Outline+Shadow, 3=OpaqueBox
    pub outline: f32, pub shadow: f32,
    pub alignment: Alignment,         // numpad 1-9
    pub margins: Margins,
    pub encoding: Encoding,
}
```

### Override Tag 表达式求值
- 引入 `OverrideExpr` AST
- 支持 `Scalar(f64)` / `Color(Color)` / `Animated(start, end, accel)` / `Transform(...)` 等
- `Animator` trait：每个 tag 实现 `evaluate(time_ms) -> Value`

### 错误恢复
- 解析器产出 `Result<AssFile, Vec<ParseError>>`
- 单行错误不中断整体解析
- `AssFile.warnings: Vec<ParseWarning>` 暴露

## 已知 Bug 修复（从代码审计发现，必须在 Sprint 1 优先处理）

1. **统一 `parse_single_tag` 与 `parse_override_tag` 路径** — 消除 `crates/ass-parser/src/event.rs` 和 `crates/ass-parser/src/override_tag.rs` 之间的 ~760 行重复代码。`event.rs::parse_single_tag` 缺失 `parse_override_tag` 已有的标签：`\blur`（Gaussian）、`\fsp`（Spacing）、`\clip(@)`、`\iclip(@)`。让 `event.rs` 直接调用 `override_tag.rs::parse_override_tag`。这立即修复 4 个标签在对话覆盖块中被静默丢弃的 bug。
2. **修复 `\K`（大写 K 的卡拉 OK）** — 当前 `s.strip_prefix("k")` 是大小写敏感的，不会匹配 `\K`。`\K` 应与 `\kf` 完全相同（Aegisub 文档明确）。在两个解析器中都修复。
3. **`ScriptInfo` 缺失头部** — 添加 `LayoutResX`、`LayoutResY`、`Collisions`、`PlayDepth` 字段。
4. **强制定义字段验证** — `Encoding` 必须始终为 1；`Fontname` ≤ 31 字符；`Fontsize` 在 0..=511 范围内；样式名不得以空格开头/结尾。

## 任务清单

| # | 任务 | 验证 |
|---|------|------|
| 2.0 | 修复已知 bug（见上）| 新增/修改单元测试 |
| 2.1 | V4+ Styles 22 字段全部强类型 | 单元测试每个字段 round-trip |
| 2.2 | Events 10 字段 + Style 引用 | golden 测试 50+ ASS 文件 |
| 2.3 | Override tag 解析器（统一路径）| 50+ 单元测试覆盖每个 tag |
| 2.4 | `\t(\tag, t1, t2, accel)` 动画插值 | 视觉对比 libass |
| 2.5 | 错误恢复（已知 libass 行为：跳过错误行，*Default fallback）| 故意损坏输入测试 |
| 2.6 | SRT → ASS 升级 | round-trip SRT → ASS → SRT |
| 2.7 | SSA v4 对齐转换（1-11 → 1-9 numpad）| 单元测试 |
| 2.8 | libass 兼容性测试套件（如果可用）| 100+ libass 官方 fixtures |
| 2.9 | Fuzz 测试扩展到 `event.rs` 覆盖标签解析器 | 24h crash 报告 |

## libass 错误处理参考（设计依据）

| 条件 | libass / 规范行为 |
|-----------|---------------------|
| 样式缺失 | 使用 `*Default` 样式，如果也缺失则用 Arial 20pt |
| 字体缺失 | 回退到 Arial（或系统提供的任何字体） |
| 未知标签 | 静默忽略（`{}` 块内未识别文本）|
| 颜色格式错误 | libass 尝试解析，或默认白色 |
| 时间戳无效 | 跳过事件 |
| 未知章节 | 静默忽略 |
| 文件非 UTF-8 | 拒绝文件 |
| 样式/事件行字段不足 | 跳过该行 |
| 对齐超出范围 | 剪辑到有效范围（ASS 1-9，SSA 等效）|
| BOM | 支持，推荐但非必需 |

## 验证门 (Definition of Done)

- [ ] 4 个已知 bug 全部修复（统一路径、`\K`、缺失头部、强制验证）
- [ ] V4+ 22 字段全部解析 + 序列化
- [ ] Override tag 100% 覆盖（参考 libass tag 列表）
- [ ] 错误恢复：损坏输入产出可用 AST + 警告列表（libass 兼容）
- [ ] SSA v4 对齐转换正确
- [ ] libass 兼容测试 ≥ 95% pass
- [ ] 现有 parser 测试 0 失败
- [ ] `cargo clippy -D warnings` 零警告

## 参考实现

| 参考 | 用途 |
|-----------|------|
| [libass ass_parse.c](https://github.com/libass/libass/blob/master/libass/ass_parse.c) | 覆盖标签解析的权威实现 |
| [libass ass_render.c](https://github.com/libass/libass/blob/master/libass/ass_render.c) | 样式合并、\t 插值 |
| [Aegisub ASS 标签](https://aegisub.org/docs/latest/ass_tags/) | 最完整的标签描述 |
| [wiedymi/ass-rs](https://github.com/wiedymi/ass-rs) | 最先进的纯 Rust ASS 解析器（零拷贝）|
| [TCax ASS 规范](http://www.tcax.org/docs/ass-specs.htm) | 原始 SSA v4 文档 + ASS 红色标注 |
