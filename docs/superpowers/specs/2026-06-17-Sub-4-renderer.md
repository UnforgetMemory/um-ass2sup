# Sub-4: 渲染器核心 (Renderer Core)

**Sprint**: S3
**周期**: 2-3 周
**依赖**: Sub-2, Sub-3, Sub-1
**阻塞**: Sub-5, Sub-6

## 目标

将渲染器重写为 cosmic-text Buffer 驱动，支持完整 ASS 特效系统（fade、pos、move、clip、org、frz、fax、fay、blur、\t 动画、karaoke、drawing），像素级精度。

## 范围

### In Scope
- cosmic-text `Buffer` 编排
- 完整 ASS 特效：
  - `\fad(t1, t2)` 淡入淡出
  - `\fade(a1, a2, a3, t1, t2, t3, t4)` 复杂 alpha
  - `\pos(x, y)` 绝对定位
  - `\move(x1, y1, x2, y2, t1, t2)` 移动
  - `\clip(x1, y1, x2, y2)` 矩形裁剪
  - `\clip(@)` / `\iclip(@)` 路径裁剪
  - `\org(x, y)` 旋转中心
  - `\frz`, `\frx`, `\fry`, `\fax`, `\fay` 旋转
  - `\blur(n)` 模糊
  - `\t(\tag, t1, t2, accel)` 动画
  - `\k`, `\K`, `\kf`, `\ko` karaoke
  - `\p` drawing mode
- Layer ordering（z-depth）
- 像素精度（subpixel positioning, hinting options）

### Out of Scope
- GPU 加速（属于 Sub-6）
- 颜色空间（属于 Sub-5）

## 架构决策

### 渲染管线
```rust
pub struct Renderer {
    font_system: FontSystem,
    backend: Box<dyn RendererBackend>,  // CPU (tiny-skia) 或 GPU (Sub-6)
    color_pipeline: ColorPipeline,       // Sub-5
}

impl Renderer {
    pub fn render_ass(&self, ass: &AssFile, pts_ms: u64) -> Option<RenderedFrame> {
        // 1. 构建 cosmic-text Buffer（width, height, scale）
        // 2. 解析事件 Text 为 spans（per-style, per-override-tag-block）
        // 3. 设置 span 样式（font, color, transform）
        // 4. buffer.shape_until_scroll() 触发布局
        // 5. 提取 glyph runs → backend.draw_glyphs(...)
        // 6. 应用 effects（fade, blur, clip）→ backend
    }
}
```

### 特效系统
- 引入 `Effect` enum：`Fade`, `Move`, `Clip`, `Blur`, `Transform`
- 每帧重新求值 override tag 的 effect（处理动画）
- `EffectStack`：每行文本可能嵌套多个 effect（clip + fade + pos）

### Karaoke
- 引入 `KaraokeState` 跟踪已唱/未唱
- 维护 fill color sweep（线性扫描）
- 与 fade 交互测试

## 任务清单

| # | 任务 | 验证 |
|---|------|------|
| 4.1 | cosmic-text Buffer 编排 | unit test |
| 4.2 | Spans 解析（per-style + override tag） | unit test |
| 4.3 | `\fad`, `\fade` 实现 | 视觉对比 libass |
| 4.4 | `\pos`, `\move` 实现 | 视觉对比 |
| 4.5 | `\clip`, `\iclip` 实现 | 视觉对比 |
| 4.6 | `\frz`, `\frx`, `\fry` 实现 | 视觉对比 |
| 4.7 | `\blur` 实现 | 视觉对比 |
| 4.8 | `\t(\tag, ...)` 动画 | 视觉对比 |
| 4.9 | Karaoke `\k`, `\K`, `\kf`, `\ko` | 视觉对比 |
| 4.10 | Drawing mode `\p` | 视觉对比 |
| 4.11 | Layer ordering | 视觉对比 |
| 4.12 | 像素精度调优 | 与 libass 像素匹配 ≥ 90% |

## 验证门 (Definition of Done)

- [ ] 11 个特效全部实现
- [ ] Karaoke 4 模式全部实现
- [ ] Drawing mode 基础支持
- [ ] 与 libass 输出像素匹配 ≥ 90%
- [ ] 单事件渲染 ≤ 50ms（debug）/ ≤ 10ms（release）
- [ ] 现有 440+ 测试通过
