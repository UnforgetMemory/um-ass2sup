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

## 任务清单

| # | 任务 | 验证 |
|---|------|------|
| 2.1 | V4+ Styles 22 字段全部强类型 | 单元测试每个字段 round-trip |
| 2.2 | Events 10 字段 + Style 引用 | golden 测试 50+ ASS 文件 |
| 2.3 | Override tag 解析器 | 50+ 单元测试覆盖每个 tag |
| 2.4 | `\t(\tag, t1, t2, accel)` 动画 | 视觉对比 libass |
| 2.5 | 错误恢复 | 故意损坏输入测试 |
| 2.6 | SRT → ASS 升级 | round-trip SRT → ASS → SRT |
| 2.7 | libass 兼容性测试套件（如果可用） | 100+ libass 官方 fixtures |

## 验证门 (Definition of Done)

- [ ] V4+ 22 字段全部解析 + 序列化
- [ ] Override tag 100% 覆盖（参考 libass tag 列表）
- [ ] 错误恢复：损坏输入产出可用 AST + 警告列表
- [ ] libass 兼容测试 ≥ 95% pass
- [ ] 现有 parser 测试 0 失败
- [ ] `cargo clippy -D warnings` 零警告
