# 重构 2.4 — 渲染器核⼼精调 (Renderer Core Perfection)

> 接续 2.3（cosmic-text 字体引擎迁移完成）。聚焦像素级精度、边界情况覆盖、文件模块化。

## 全链路定位

```
2.1 (ass-core)   2.2 (renderer)    2.3 (font)         2.4 (perfection)
  AST 解析        53-tag 覆盖       cosmic-text 迁移 →   像素精度回归
                                                        文件模块化
                                                        边界覆盖
```

## 当前架构风险

### 文件膨胀（突破可维护性阈值）

| 文件 | 当前行数 | 安全阈值 | 超标倍数 | 根因 |
|------|----------|----------|----------|------|
| `renderer/cosmic.rs` | **918** | 250 | **3.7×** | 渲染循环 + karaoke + build_context 混在⼀个文件 |
| `renderer/animation.rs` | **834** | 300 | **2.8×** | fade/move/transform 全部在⼀个文件 |
| `renderer/compositing.rs` | **488** | 300 | **1.6×** | clip/shadow/composite 未分离 |
| `effects.rs` | **446** | 300 | **1.5×** | blur/shadow 未分离 |

### 功能缺口

| # | 功能 | 现状 | 症状 | 修复方案 |
|---|------|------|------|----------|
| F1 | Spans 解析 | `Attrs::new()` 全局统⼀ | CJK 和拉丁⽤同⼀字体 | 拆分为 per-style AttrsList |
| F2 | 像素回归 | ⽆ | ⽆法量化 libass 差距 | 搭建 libass 参考 + diff pipeline |
| F3 | \p4 clip mask | 未处理 | `\p4` 绘制 clip 不⼯作 | 检测 drawing_level==4 → clip mask |
| F4 | perspective 退步 | test_perspective 被 ignore | \frx45 输出空帧 | 修复 AffineTransform 链 |
| F5 | 边界测试 | 零散 | \fad(0,0), \move 超出边界 等 | 每个特效 3+ 边界 case |

## 模块化设计

### ⽬标模块树

```
crates/subtitle-renderer/src/
├── cosmic/                    # ← 当前 cosmic/ 只含 resolver/shaper/rasterizer
│   ├── mod.rs                 # 模块声明 + pub use
│   ├── resolver.rs            # FontCosmicResolver (249⾏, OK)
│   ├── shaper.rs              # CosmicShaper (88⾏, OK)
│   ├── rasterizer.rs          # rasterize_cosmic_glyph (176⾏, OK)
│   ├── spans.rs               # ★ 新增: AttrsList 分段解析 (~200⾏)
│   │                          #   输⼊: event.text + style
│   │                          #   输出: Vec<(String, Attrs)>
│   ├── layout.rs              # ← 从 cosmic.rs 拆出: 渲染主循环 (~250⾏)
│   │                          #   pub fn render_event_cosmic()
│   │                          #   pub fn render_karaoke_cosmic()
│   ├── bbox.rs                # ← 从 cosmic.rs 拆出: 边界计算 (~80⾏)
│   │                          #   pub(super) fn compute_glyph_bounds()
│   │                          #   pub(super) fn compute_sub_region()
│   └── effects/               # ★ 新增: 特效模块组
│       ├── mod.rs             # 重导出
│       ├── blur.rs            # ← 从 effects.rs 拆出: ⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿⓿
│       │               # apply_gaussian_blur() (~120⾏)
│       ├── shadow.rs          # ← 从 effects.rs 拆出: apply_shadow() (~100⾏)
│       ├── composite.rs       # ← 从 compositing.rs 拆出: composite_over(), alpha_multiplier() (~120⾏)
│       └── clip.rs            # ← 从 compositing.rs 拆出: apply_clip_mask(), apply_drawing_clip_mask() (~180⾏)
├── renderer/
│   ├── mod.rs                 # Renderer struct + render_ass (492⾏, 需减到 < 300)
│   │                          #   → 拆出 font.rs? 不, font 已归档
│   │                          #   → 拆出 build_context.rs  (~250⾏)
│   └── context/               # (已存在, dead code, 清理)
│       ├── mod.rs             # 标记 #[deprecated] 或移除
│       ├── font.rs...
│       └── border.rs...
└── animation/                 # ★ 新增: 动画模块组
    ├── mod.rs                 # 重导出
    ├── fade.rs                # ← 从 animation.rs 拆出: compute_fad_alpha(), compute_fade_complex() (~100⾏)
    ├── transform.rs           # ← 从 animation.rs 拆出: apply_transform_tag() (~250⾏)
    └── move.rs                # ← 从 animation.rs 拆出: interpolate_move() (~60⾏)
```

## 执行波次

### Wave 1: 文件模块化（4 工匠可并行）

#### W1.1 — 拆分 `renderer/cosmic.rs` (918⾏ → 3 文件)

**操作**：
1. 抽出 `cosmic/layout.rs` — render_event_cosmic() + render_karaoke_cosmic() 主循环
2. 抽出 `cosmic/bbox.rs` — compute_glyph_bounds() + compute_sub_region()
3. cosmic.rs 原文件只保留 pub use 重导出（~30⾏）
4. 更新 cosmic/mod.rs 添加 mod layout + mod bbox

**验收**：cargo check 通过，行数 918→30+250+80

#### W1.2 — 拆分 `animation.rs` (834⾏ → 3 文件)

**操作**：
1. 新建 `animation/fade.rs` — compute_fad_alpha() + compute_fade_complex()
2. 新建 `animation/transform.rs` — apply_transform_tag() + parse_override_block()
3. 新建 `animation/move.rs` — interpolate_move()
4. animation.rs 原文件保留重导出（~30⾏）
5. 更新 renderer/mod.rs 导入路径

**验收**：cargo check 通过，行数 834→30+100+250+60

#### W1.3 — 拆分 `compositing.rs` (488⾏ → 3 文件)

**操作**：
1. 新建 `cosmic/effects/composite.rs` — composite_over() + apply_alpha_multiplier()
2. 新建 `cosmic/effects/clip.rs` — apply_clip_mask() + apply_drawing_clip_mask()
3. compositing.rs 原文件保留 composite_over/alpha 重导出（~50⾏）
4. 更新 cosmic/effects/mod.rs

**验收**：cargo check 通过，行数 488→50+120+180

#### W1.4 — 拆分 `effects.rs` (446⾏ → 2 文件)

**操作**：
1. 新建 `cosmic/effects/blur.rs` — apply_gaussian_blur()
2. 新建 `cosmic/effects/shadow.rs` — apply_shadow()
3. effects.rs 原文件保留重导出（~30⾏）

**验收**：cargo check 通过，行数 446→30+120+100

#### W1.5 — 拆分 `renderer/mod.rs` (492⏄ → 2 文件)

**操作**：
1. 抽出 `renderer/build_context.rs` — build_context() (~250⾏)
2. renderer/mod.rs 保留 Renderer struct + render_ass (242⾏)

**验收**：cargo check 通过，行数 492→242+250

#### W1.6 — 清理 `renderer/context/` (dead code, 可并行于 W1.1-1.5)

**操作**：
1. renderer/context/mod.rs → 添加 `#![allow(dead_code)]` 或 整体 cfg(gate)
2. 或者直接删除（不被任何代码引用）

**验收**：cargo check 通过，零 warning 变化

### Wave 2: 功能补全（依赖 W1）

#### W2.1 — Spans 解析 (`cosmic/spans.rs`, ~200⏄)

**设计**：
```rust
/// 将 event.text 分解为 (plain_text, Attrs) 段
/// "{\b1}Bold{\b0}Normal" → [("Bold", Attrs::bold), ("Normal", Attrs::normal)]
pub fn parse_spans(text: &str, style: &Style) -> Vec<(String, cosmic_text::Attrs<'static>)>;
```

**逻辑**：
1. 调⽤ strip_override_blocks() 得 plain text
2. 遍历 event.override_tags，按 byte offset 分段
3. 每段映射为 cosmic_text::Attrs（family, weight, style, color, font_size）
4. 构建 AttrsList 写⼊ cosmic-text Buffer

**验收**：单元测试 5+ case（纯文本、粗体、颜⾊、混合、空）

#### W2.2 — \p4 clip mask (`cosmic/effects/clip.rs` 扩展, ~100⾏)

**操作**：
1. 在 render_event_cosmic 中检测 `drawing_level == 4`
2. 解析 drawing commands → tiny-skia Path
3. 渲染文本到临时 layer
4. 以 drawing path 作为 clip mask 合成

**验收**：\p4 测试产⽣可见像素

#### W2.3 — Perspective 修复 (`animation/transform.rs`, ~100⾏)

**根因**：AffineTransform::apply_with_perspective() 以 (0,0) 为原点旋转时⽂本移出屏幕

**修复**：
1. 当 origin_x/origin_y 均为 0 时，使⽤ alignment 默认位置作为原点
2. 与 build_context 的 `!has_pos` 逻辑保持⼀致

**验收**：test_perspective 测试重新启⽤（移除 cfg_attr ignore）

### Wave 3: 验证

#### W3.1 — 单元边界测试 (`tests/test_cosmic_edge.rs`, ~200⾏)

| 测试 | 场景 | 预期 |
|------|------|------|
| `test_fad_zero_duration` | \fad(0,0) | 不 panic, 产⽣可见帧 |
| `test_move_out_of_bounds` | \move(-1000,-1000,5000,5000) | 不 panic |
| `test_clip_empty_rect` | \clip(0,0,0,0) | 不 panic, 空帧 |
| `test_font_not_found` | \fn(NonExistentFont) | 退回到 default font |
| `test_empty_text` | 空 Dialogue | 不 panic |
| `test_unicode_surrogate` | 含 surrogate pair | 正确 shaping |
| `test_extremely_long_text` | 10,000 字符 | 不 panic, 合理裁剪 |
| `test_karaoke_empty_seg` | 空 karaoke segment | 不 panic |
| `test_drawing_p4_basic` | \p4 绘制 + 文本 | 输出有像素 |

#### W3.2 — 像素回归框架 (`tests/pixel_regression/`, Python + Rust)

**不在此 sprint 范围内** — 这是⼀个独⽴⼯程。记录为后续⼯作：

```python
# scripts/pixel_regression.py (未来⼯作)
# 1. 对每个 ASS 特效，调⽤ libass 渲染参考 PNG
# 2. 调⽤ ass2sup --cosmic-text 渲染⽬标 PNG
# 3. 逐像素对⽐，输出 diff 热⼒图 + 相似度评分
```

## 依赖图

```
W1.1 ───┐
W1.2 ───┤
W1.3 ───┤── 可并行 ──→ W2.1 ──→ W3.1
W1.4 ───┤              W2.2 ──→ W3.1
W1.5 ───┤              W2.3 ──→ W3.1
W1.6 ───┘
```

W1 全部 6 个子任务可并行执⾏。W2 依赖 W1 完成后启动，W2.1/2.2/2.3 可并行。
W3.1 依赖全部 W2 完成后启动。

## 验证门 (Definition of Done)

- [ ] **模块化**: cosmic/ + renderer/ 下所有 .rs ⽂件 ≤ 300⾏（mod.rs 除外）
- [ ] **编译**: `cargo check -p subtitle-renderer` 零错误零警告
- [ ] **clippy**: `cargo clippy -p subtitle-renderer -- -D warnings` 零警告
- [ ] **回归**: `cargo test -p subtitle-renderer` 全部通过
- [ ] **边界**: 9 个边界测试全部通过
- [ ] **透视**: test_perspective_frx_renders / test_perspective_with_org 重新启⽤
- [ ] **\p4**: drawing_level_4 测试产⽣可见像素
- [ ] **dead code**: renderer/context/ 清理（标 deprecated 或移除）

## 风险矩阵

| # | 风险 | 可能性 | 影响 | 缓解 |
|---|------|--------|------|------|
| R1 | 模块拆分导致 import 循环 | 中 | ⾼ | 先画依赖图再拆分 |
| R2 | cosmic/layout.rs 引⽤ cosmic/effects/ 中的函数 | 低 | 低 | effects 是⼦模块, cosmic 内访问⽆问题 |
| R3 | 918⾏的 cosmic.rs ⼀次拆完太危险 | ⾼ | 中 | 分 3 步: 先抽 bbox → 再抽 layout → 最后 context |
| R4 | effect 模块拆分后性能退化 | 低 | 低 | 保持 pub(super) + inline hints |
| R5 | renderer/context/ ⼲净删除后发现还有引⽤ | 中 | 低 | 先 grep 确认⽆引⽤,再删 |

## 不做

- ❌ GPU 加速（属于 2.6）
- ❌ HDR 色彩管线（属于 2.5）
- ❌ 新特效实现（只做边界补全）
- ❌ 新输出格式（属于 2.7）
- ❌ libass 像素回归框架（记录为后续⼯作，⾮此 sprint）
- ❌ cosmic-text 版本升级（保持 0.19）
