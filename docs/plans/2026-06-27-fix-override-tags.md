# Override Tags 修复计划 — Scale/Rotation/Shear/Outline/Spacing

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 修复 ASS 覆盖标签 `\fscx`(ScaleX)、`\fscy`(ScaleY)、`\frz`/`\fr`(Angle/Rotation)、`\fax`/`\fay`(Shear)、`\bord`(Outline)、`\fsp`(Spacing) 在渲染管线中不生效或错误的问题。

**架构：** 三个独立手术——(A) `transform_layer` 构建真实仿射变换矩阵，(B) 添加 outline 描边渲染通道，(C) spacing 计入换行宽度。

**技术栈：** Rust, tiny-skia, swash, subtitle-renderer crate

---

## 文件清单

| 文件 | 职责 | 变更类型 |
|------|------|----------|
| `crates/subtitle-renderer/src/renderer/font_registry_renderer.rs` | 渲染主循环、glyph合成、变换、合成输出 | **修改** |
| `crates/subtitle-renderer/src/renderer/layout_font_registry.rs` | 文本换行布局 | **修改** |
| `crates/subtitle-renderer/src/renderer/build_context.rs` | 从 override tags 构建 RenderContext | **验证（无修改）** |
| `crates/subtitle-renderer/src/transform.rs` | AffineTransform 实现 | **验证（无修改）** |
| `crates/subtitle-renderer/src/effects/mod.rs` | 特效模块（blur、shadow、composite、clip） | **验证（无修改）** |
| `crates/subtitle-renderer/src/effects/shadow.rs` | 阴影实现 | **验证（无修改）** |

### 不受影响的文件

这些文件的 parse 层正确——不需要修改：
- `crates/ass-core/src/override_tag/geometry.rs`（Scale/Rotation/Shear 解析 ✓）
- `crates/ass-core/src/override_tag/border.rs`（Border/Shadow 解析 ✓）
- `crates/ass-core/src/override_tag/position.rs`（Pos/Move 解析 ✓）
- `crates/ass-core/src/override_tag/font.rs`（Strikeout/Alignment/Charset 解析 ✓）
- `crates/ass-core/src/lib.rs`（OverrideTag enum ✓）

---

## 场景合同

### S1: ScaleX=50%, ScaleY=50% 使文本缩小一半
- **ASS**: `{\fscx50\fscy50}Hello`
- **Surface**: 生成的 SUP 中文本宽度/高度缩小 50%
- **Verify**: 生成的 PNG/像素数据中文字为正常大小的一半

### S2: Rotation=45deg 使文本旋转 45 度
- **ASS**: `{\frz45}Hello`
- **Surface**: 文本倾斜 45 度

### S3: Border=5px 使文本有明显的 5px 描边
- **ASS**: `{\bord5}Hello`
- **Surface**: 文字周围有 5px 宽度的黑色边框（当前没有边框）

### S4: Spacing=10px 使字符间距拉开
- **ASS**: `{\fsp10}Hello`
- **Surface**: 字符之间有额外 10px 间距

### S5: 多个标签叠加生效
- **ASS**: `{\fscx150\fscy150\frz30\bord5}Hello`
- **Surface**: 文本放大 50%、旋转 30 度、且有 5px 边框——全部同时生效

---

## 手术 B（高复杂度）：Outline 描边渲染

### 设计思路

当前渲染流程只做一次 glyph 渲染（填充色），没有 outline pass。需要在 `render_event_font_registry` 中增加一个独立的 outline 渲染阶段：

**渲染顺序（libass 兼容）：**
1. （无 shadow 时跳过）用 `shadow_color` 渲染偏移后模糊的文本→shadow layer
2. 用 `outline_color` 在放大的字体尺寸下渲染所有 glyph→outline layer
3. 对 outline layer 做高斯模糊（模拟描边宽度）
4. 在 outline layer 上用 `primary_color` 覆盖渲染所有 glyph（正常尺寸）
5. 合成到输出

### 简化方案（兼容当前架构）

由于当前 renderer 的 glyph 渲染是逐个进行的（在 layer 上合成），outline 需要生成一个独立的 bitmap 层：

1. **outline 通道**: 遍历所有 shaped_lines 和 glyphs，用 `outline_color` + 膨胀尺寸渲染到独立的 outline Pixmap
2. **模糊 outline**: 对 outline Pixmap 做 gaussian blur（半径 = outline_width）
3. **填充通道**: 用 `primary_color` 在 outline 之上正常渲染 glyphs
4. **合成**: outline layer + fill layer → 最终 layer

### 约束
- `border_style == 3`（OpaqueBox）时不执行 outline 渲染
- `outline_width == 0` 时跳过 outline 通道
- 要确保 alpha compositing 正确（outline 的半透明边缘和 fill 正确叠加）

---

## 任务分解

### 任务 A：修复 `transform_layer` — 构建真实仿射变换

**文件：** `crates/subtitle-renderer/src/renderer/font_registry_renderer.rs`

#### 步骤 A1：重构 `transform_layer` 以构建复合变换

当前（错误）代码：
```rust
fn transform_layer(data: &[u8], lw: u32, lh: u32, w: u32, h: u32, ctx: &RenderContext) -> Vec<u8> {
    if ctx.perspective_x != 0.0 || ctx.perspective_y != 0.0 {
        AffineTransform::identity().apply_with_perspective(...)
    } else if ctx.rotation != 0.0 || ctx.shear_x != 0.0 || ctx.shear_y != 0.0 {
        AffineTransform::identity().apply_to_pixmap(...)
    } else {
        data.to_vec()
    }
}
```

替换为：
```rust
fn transform_layer(data: &[u8], lw: u32, lh: u32, w: u32, h: u32, ctx: &RenderContext) -> Vec<u8> {
    if ctx.perspective_x != 0.0 || ctx.perspective_y != 0.0 {
        // Build affine: scale then shear then rotate around centre
        let cx = lw as f32 / 2.0;
        let cy = lh as f32 / 2.0;
        let t = AffineTransform::translate(cx, cy)
            .then(&AffineTransform::scale(
                ctx.scale_x / 100.0,
                ctx.scale_y / 100.0,
            ))
            .then(&AffineTransform::shear(ctx.shear_x, ctx.shear_y))
            .then(&AffineTransform::rotate(ctx.rotation))
            .then(&AffineTransform::translate(-cx, -cy));
        t.apply_with_perspective(data, lw, lh, w, h, ctx.perspective_x, ctx.perspective_y, ctx.origin_x, ctx.origin_y)
    } else if ctx.rotation != 0.0
        || ctx.shear_x != 0.0
        || ctx.shear_y != 0.0
        || (ctx.scale_x - 100.0).abs() > 0.01
        || (ctx.scale_y - 100.0).abs() > 0.01
    {
        let cx = lw as f32 / 2.0;
        let cy = lh as f32 / 2.0;
        let t = AffineTransform::translate(cx, cy)
            .then(&AffineTransform::scale(
                ctx.scale_x / 100.0,
                ctx.scale_y / 100.0,
            ))
            .then(&AffineTransform::shear(ctx.shear_x, ctx.shear_y))
            .then(&AffineTransform::rotate(ctx.rotation))
            .then(&AffineTransform::translate(-cx, -cy));
        t.apply_to_pixmap(data, lw, lh, w, h)
    } else {
        data.to_vec()
    }
}
```

**关键注意点：**
- `scale_x` / `scale_y` 以百分比存储（100.0 = 100% = 不变），所以除 100.0
- `rotation` 以度为单位（`AffineTransform::rotate` 内部调用 `to_radians`）
- `shear_x` / `shear_y` 直接使用
- 变换中心 `(cx, cy)` 为 layer 中心，因为 `\org`（origin）没有在 `transform_layer` 中考虑——对于简单非透视变换，中心缩放/旋转是合理的 libass 近似

#### 步骤 A2：更新 `render_event_font_registry` 中 `simple` 判定条件

当前（line 331-337）：
```rust
let simple = ctx.rotation == 0.0
    && ctx.shear_x == 0.0
    && ctx.shear_y == 0.0
    && ctx.perspective_x == 0.0
    && ctx.perspective_y == 0.0
    && !ctx.clip_enabled
    && ctx.clip_drawing_commands.is_none();
```

新增对 scale 的检查：
```rust
let simple = ctx.rotation == 0.0
    && ctx.shear_x == 0.0
    && ctx.shear_y == 0.0
    && ctx.perspective_x == 0.0
    && ctx.perspective_y == 0.0
    && (ctx.scale_x - 100.0).abs() < 0.01
    && (ctx.scale_y - 100.0).abs() < 0.01
    && !ctx.clip_enabled
    && ctx.clip_drawing_commands.is_none();
```

否则 scale != 100% 且其他 transform 都为 0 时也会走恒等路径（`else { data.to_vec() }`）。

---

### 任务 B：添加 Outline 描边渲染通道

**文件：** `crates/subtitle-renderer/src/renderer/font_registry_renderer.rs`

#### 步骤 B1：提取 glyph 光栅化+合成到独立 buffer 的方法

当前 glyph 渲染循环（lines 225-303）直接渲染到 `layer` Pixmap。需要将 fill 和 outline 分离。

设计思路：

1. 在 glyph 渲染循环之前，如果 `border_style != 3 && outline_width > 0.0`，创建 outline Pixmap
2. 在 outline Pixmap 上以膨胀后的字体尺寸 + `outline_color` 渲染所有 glyph
3. 对 outline 做高斯模糊（半径 = outline_width）
4. 在 outline 之上以正常尺寸 + `primary_color` 渲染 glyphs
5. 合成到最终 layer

**重构方案：**

将 glyph 渲染提取为可复用的函数：
```rust
fn render_glyphs_to_pixmap(
    pixmap: &mut Pixmap,
    shaped_lines: &[ShapedLine],
    ctx: &RenderContext,
    oxf: f32,
    oyf: f32,
    font_map: &HashMap<String, Vec<String>>,
    style_name: &str,
    registry: &FontRegistry,
    color: [u8; 4],
    font_size_multiplier: f32,
) {
    for sl in shaped_lines {
        let mut cx = sl.x_start - oxf;
        for g in &sl.glyphs {
            let font_data = resolve_glyph_font_data(...);
            let size = ctx.font_size * font_size_multiplier;
            match GlyphRasterizer::rasterize(&font_data, g.glyph_id, size) {
                Ok(rasterized) => {
                    composite_glyph(pixmap, &rasterized, cx + g.x_offset,
                        sl.line_y + g.y_offset - oyf, color);
                }
                ...
            }
            cx += g.x_advance + ctx.spacing;
        }
    }
}
```

#### 步骤 B2：实现 outline 渲染流程

在原 glyph 渲染位置，插入：

```rust
// Outline pass
let has_outline = ctx.border_style != 3 && ctx.outline_width > 0.1;
if has_outline {
    // TODO: Determine outline expansion factor
    let expansion = 1.0 + (ctx.outline_width * 2.0) / ctx.font_size;
    let mut outline_layer = match resources.pool_get(lw, lh) {
        Some(p) => p,
        None => return,
    };
    render_glyphs_to_pixmap(&mut outline_layer, &shaped_lines, &ctx,
        oxf, oyf, &resources.font_map, event.style.as_str(),
        &registry, ctx.outline_color, expansion);
    apply_gaussian_blur(&mut outline_layer, ctx.outline_width);
    // Composite outline under fill
    composite_over(layer.data_mut(), outline_layer.data(), lw, lh);
    resources.pool_put(outline_layer);
}

// Fill pass
render_glyphs_to_pixmap(&mut layer, &shaped_lines, &ctx,
    oxf, oyf, &resources.font_map, event.style.as_str(),
    &registry, ctx.primary_color, 1.0);
```

**注意点：**
- 因为 layer 已经由前面的循环填充了 fill glyphs，outline pass 需要在 **fill pass 之前**完成
- 膨胀比例 `expansion = 1.0 + 2*outline_width/font_size` 估计值，用于 glyph 光栅化时放大以适应描边宽度
- outline 模糊使用 `effects::apply_gaussian_blur`
- 合成的顺序：outline 先渲染到自己的 buffer，模糊，然后通过 `composite_over` 放在 fill 下面

**但是**，这个方案有个问题——layer 当前已经渲染了 fill glyphs，outline 需要先做。更好的方案：

1. 分配 outline_layer（新 buffer）
2. 在 outline_layer 上以膨胀尺寸+outline_color 渲染
3. 模糊 outline_layer
4. 清空 layer，以正常尺寸+primary_color 渲染 fill
5. composite_over(layer, outline) — 把 outline 放到底层

或者更简单的方案：
1. 渲染所有 glyph 到 temp_layer（正常尺寸，primary_color）
2. 如果有 outline：渲染 outline 到单独 buffer → 模糊 → 合成到 temp_layer 下面
3. 继续当前流程

**最终选定的简化方案（更低侵入）：**

保留现有 glyph 渲染循环不变（它渲染 fill 到 layer）。在此循环之前，如果 `has_outline`：
1. 用 outline_color + 膨胀尺寸渲染 glyph 到 outline_pixmap
2. 模糊 outline_pixmap
3. 留待后续合成

然后在 shadow 处理之前：
1. composite_over(layer, outline) — outline 合成到 layer 下方
2. 继续正常的 shadow/blur/transform 流程

实际上，正确的顺序是：outline 应该在 fill 之下、shadow 之上。所以：

```
shadow → outline → fill → transform → composite to screen
```

但为了最小化代码变更，可以使用：
1. 渲染 fill glyphs 到 layer（已有）
2. 渲染 outline glyphs 到 outline_layer
3. 模糊 outline_layer
4. 把 outline 合成到 layer 下方（通过 composite_over 把 layer 叠在 outline 上）
5. 正常的 shadow/blur/transform 流程

**关键问题：** `composite_over(dst, src)` 是 Porter-Duff Over，即 src 叠在 dst 之上。要把 outline 放在 fill 下方，需要：
```
composite_over(outline_buffer, layer) // outline_buffer = outline + fill on top
```

或者在 outline 渲染后用 swap 方式：
```
let result = composite_over(empty, outline); // result = outline
composite_over(result, layer); // result = outline + fill
layer = result;
```

---

### 任务 C：修复 Spacing 在换行宽度计算

**文件：** `crates/subtitle-renderer/src/renderer/layout_font_registry.rs`

#### 步骤 C1：在 `wrap_text_lines_simple` 中使用 spacing 参数

当前函数签名和参数使用：
```rust
fn wrap_text_lines_simple(text: &str, font_data: &[u8], fz: f32, _sp: f32, mw: f32) -> Vec<String> {
```

将 `_sp` 改为 `sp` 并在换行宽度计算时计入间距。

每行宽度 = Σ(g.x_advance) + (n_glyphs - 1) * spacing

需要在 `SimpleShaper::shape` 返回 glyph 列表后，计算宽度时加上间距：

```rust
// 在 cw += ww 之前，ww 需要包含 spacing
// 当前 ww 计算: .map(|g| g.x_advance).sum()
// 改为: .map(|g| g.x_advance).sum::<f32>() + (glyphs.len().saturating_sub(1) as f32) * sp
```

---

## 执行顺序

1. **任务 C（Spacing）** — 最低风险，最快见效
2. **任务 A（transform_layer）** — 影响 Scale/Rotation/Shear 三个标签族，中风险
3. **任务 B（Outline）** — 最高风险，依赖于正确理解合成顺序

## 验证

每个任务完成后：
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-targets`
4. 手动验证：`cargo run -- input.ass -o output.sup`

## 变更日志

任务全部完成后更新 `CHANGELOG.md`，注明修复的标签：
- `\fscx`/`\fscy` (ScaleX/Y)
- `\frz`/`\fr` (Rotation)
- `\fax`/`\fay` (Shear)
- `\bord` (Outline)
- `\fsp` (Spacing)