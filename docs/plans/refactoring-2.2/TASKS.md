# 2.2 原子任务列表

> 执行严格依此顺序。每波次内并行，波次间串行阻塞。

## Wave 1（并行 3 任务）

| # | 文件 | 动作 | 验证 |
|---|------|------|------|
| 1 | `Cargo.toml`, `lib.rs` | ass-parser → ass-core 依赖替换 | `cargo check` |
| 2 | `context.rs` | RenderContext 补全 53 标签字段 | `cargo test` |
| 3 | `error.rs` | EventError 类型 + EventResult | `cargo test` |

## Wave 2（并行 12 任务，等 T2/T3）

### build_context 处理器（并行 10，等 T2）

| # | 文件 | 动作 |
|---|------|------|
| 4 | `renderer/context/position.rs` | Pos, Move, Origin 处理器 |
| 5 | `renderer/context/font.rs` | FontName, FontSize, Bold 等 8 标签 |
| 6 | `renderer/context/color.rs` | 1c-4c, alpha, 1a-4a 共 9 标签 |
| 7 | `renderer/context/border.rs` | bord, shad + X/Y 变体 共 6 |
| 8 | `renderer/context/geometry.rs` | Scale, Rotation, Shear 等 7 |
| 9 | `renderer/context/clip.rs` | clip, iclip + @ 共 6 变体 |
| 10 | `renderer/context/karaoke.rs` | karaoke 标志位 |
| 11 | `renderer/context/reset.rs` | Reset, ResetAll 样式回退 |
| 12 | `renderer/context/transform.rs` | Transform 动画插值委托 |
| 13 | `renderer/context/misc.rs` | an, a, q, fe, !, p, pbo 等 9 |

### support 模块（并行 2，等 T1）

| # | 文件 | 动作 |
|---|------|------|
| 15 | animation.rs, compositing.rs, drawing.rs, text_layout.rs | ass_parser→ass_core 类型更新 |
| 16 | font.rs, shaper.rs, rasterizer.rs, transform.rs, effects.rs, karaoke.rs | 仅 import 更新 |

## Wave 3（2 任务串行）

| # | 文件 | 动作 | 等 |
|---|------|------|-----|
| 14 | `renderer/context/mod.rs` | build_context orchestrator 编排 10 处理器 | T4-13 |
| 17 | `renderer/mod.rs` | render_ass 管线：event 过滤→排序→catch_unwind 渲染 | T3, T14, T15, T16 |

## Wave 4（2 任务串行）

| # | 文件 | 动作 | 等 |
|---|------|------|-----|
| 18 | `tests/` 全部 | 测试更新到 ass_core 类型 | T17 |
| 19 | `renderer/context/tests.rs` | 53 标签覆盖测试 (85+ cases) | T17 |
