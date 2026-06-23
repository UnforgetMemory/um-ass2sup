# Cosmic-Text 字体引擎 — 完整规格

> 基于 plan agent 输出，2026-06-23

## 1. 战略定位

```
旧的 (v0.5)                    新的 (v2.2+)
─────────────────              ─────────────────
fontdb (字体发现)              cosmic-text FontSystem
rustybuzz (HarfBuzz 塑形)      cosmic-text Buffer (internal HarfRust)
ttf-parser (TTF 解析)          skrifa (bundled via cosmic-text)
tiny-skia (字形光栅化)          swash (bundled via cosmic-text, optional)
手动 CJK 回退 (8级链)          Fallback trait (可编程)
查找→塑形→光栅化 三阶段分离     一站式：shape_and_rasterize()
parking_lot::Mutex<inner>     parking_lot::Mutex<FontSystem> (因 !Send)
```

## 2. DDD 模块设计

### 2.1 `cosmic/resolver.rs` — FontCosmicResolver

```rust
pub struct FontCosmicResolver {
    font_system: Mutex<cosmic_text::FontSystem>,
    swash_cache: Mutex<cosmic_text::SwashCache>,
    cjk_fallback_key: Mutex<Option<cosmic_text::FontKey>>,
}
```

**公开方法**：
- `new()` — 创建 FontSystem，加载系统字体，预扫描 CJK 回退
- `load_system_fonts()` — 重新扫描
- `load_font_file(path) / load_fonts_dir(path)`
- `resolve_font(family, bold, italic) -> Option<FontKey>`
- `get_image(key, glyph_id, font_size) -> Option<SwashImage>`
- `warmup_cjk_scan()` — 预计算 CJK 回退字体（仅在 `AssFallback` 中触发）

**关键设计**：
- `FontSystem` + `SwashCache` 各用独立 `Mutex`，粒度分离
- CJK pre-scan 使用 skrifa 检测 U+4E2D 字形
- `!Send + !Sync` 通过 Mutex 桥接

### 2.2 `cosmic/shaper.rs` — CosmicShaper

```rust
pub struct CosmicShapedText {
    pub glyphs: Vec<CosmicShapedGlyph>,
    pub total_advance: f32,
}

pub struct CosmicShapedGlyph {
    pub glyph_id: u16,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub font_key: cosmic_text::FontKey,  // per-glyph! 支持回退
}
```

**核心逻辑**：
1. 创建 `Buffer::new(font_system, Metrics { font_size, line_height })`
2. `buffer.set_text(text, &attrs, Shaping::Advanced, None)`
3. `buffer.shape_until_scroll(font_system, true)`（触发塑形，不做布局）
4. 遍历 `buffer.lines[0].runs[0].glyphs` 提取每个字形信息
5. 每字形记录 FontKey（cosmic-text 可能对拉丁/CJK 用不同字体）

### 2.3 `cosmic/rasterizer.rs` — 字形光栅化桥接

```rust
pub fn rasterize_cosmic_glyph(
    pixmap: &mut Pixmap,
    resolver: &FontCosmicResolver,
    glyph: &CosmicShapedGlyph,
    x: f32, y: f32,
    ctx: &RenderContext,
);
```

**流程**：
1. `resolver.get_image(glyph.font_key, glyph.glyph_id, ctx.font_size)` → SwashImage
2. SwashImage 是 alpha 蒙版 → 写入临时 Pixmap
3. 调用 `apply_anisotropic_outline()` 复用现有轮廓逻辑
4. 用 `ctx.primary_color` 着色后合成到目标 Pixmap

### 2.4 `renderer/cosmic.rs` — 渲染管线主入口

```rust
#[cfg(feature = "cosmic-text")]
fn render_event_cosmic(
    &self, pixmap, event, ctx, timestamp_ms, event_start_ms,
);
```

**流程**（与 `render_event` 平行）：
```
BEGIN
├── Handle Effect (Banner/Scroll) — 与现有相同
├── 字体解析 → FontKey (via resolver)
├── Karaoke → render_karaoke_cosmic() 分支
├── Drawing → 复用 render_drawing()
├── wrap_text() → 适配 CosmicShaper
├── 循环：逐行 → 逐字形
│   └── rasterize_cosmic_glyph() → 写入临时 layer
├── 复用后处理管线（完全不变）：
│   ├── effects::apply_gaussian_blur()
│   ├── effects::apply_shadow()
│   ├── compositing::composite_over()
│   ├── transform::AffineTransform + apply_with_perspective()
│   └── clip masks
└── 合成到最终 pixmap
END
```

## 3. API 合约（与现有管线边界）

| 接触点 | 现有 | cosmic-text |
|--------|------|-------------|
| `FontManager::query_with_fallback()` | → `Option<fontdb::ID>` | → `Option<FontKey>` |
| `get_font_data_with_index(id)` | → `Arc<Vec<u8>>` + index | **不再需要**（cosmic-text 内部管理字体数据） |
| `Shaper::shape(text, font_id, size)` | → `ShapedText` | → `CosmicShapedText`（含 per-glyph FontKey） |
| `Rasterizer::rasterize_glyph(pixmap, font_mgr, id, glyph, ...)` | → 写入 pixmap | → `rasterize_cosmic_glyph(pixmap, resolver, glyph, ...)` |

**关键差异**：现有管线需要 `fontdb::ID` + `Arc<Vec<u8>>` 供 shaper/rasterizer 创建 rustybuzz::Face/ttf_parser::Face。cosmic-text 路径不需要——塑形和光栅化由 FontSystem + SwashCache 内部完成。

## 4. 特效兼容策略

| 特效 | 兼容方式 | 工作方式 | 风险 |
|------|---------|----------|------|
| `\bord`/`\xbord`/`\ybord` | ✅ 复用 | SwashImage alpha → `apply_anisotropic_outline()` | 低 |
| `\shad`/`\xshad`/`\yshad` | ✅ 复用 | SwashImage → `apply_shadow()` | 低 |
| `\blur`/`\be` | ✅ 复用 | `apply_gaussian_blur()` | 低 |
| `\frz`/`\frx`/`\fry` | ✅ 复用 | `AffineTransform + apply_with_perspective()` | 低 |
| `\fax`/`\fay` shear | ✅ 复用 | `AffineTransform::shear()` | 低 |
| `\clip`/`\iclip` | ✅ 复用 | `apply_clip_mask()` | 低 |
| `\move` | ✅ 复用 | `build_context` 中插值 | 低 |
| `\fad`/`\fade` | ✅ 复用 | `alpha_multiplier` compositing | 低 |
| `\t` transform | ✅ 复用 | `animation::apply_transform_tag()` | 低 |
| Karaoke | ✅ 新实现 | CosmicShaper per-syllable + 复用 fill clip | 中 |
| Anisotropic outline | ✅ 复用 | `rasterizer::apply_anisotropic_outline()` | 低 |
| Emoji (彩色字形) | ✅ 支持 | SwashContent::Color → 直接 RGBA blit | 低 |

## 5. 风险矩阵

| # | 风险 | 可能性 | 影响 | 缓解 |
|---|------|--------|------|------|
| R1 | FontSystem !Send +!Sync 死锁 | 低 | 高 | 单线程 + Mutex 分离 |
| R2 | 字形渲染 AA 差异 | 高 | 中 | 接受 ≤5% 像素偏差 |
| R3 | CJK 回退不匹配 | 中 | 高 | Fallback trait + 预扫描 |
| R4 | SwashCache 内存膨胀 | 低 | 低 | 内部限制 ~10MB |
| R5 | wrap_text 需改适配器 | 中 | 中 | trait 泛型化 |
