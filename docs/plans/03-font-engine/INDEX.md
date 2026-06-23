# Sprint 03: Font Engine — cosmic-text 全栈替换

> **范围**: `subtitle-renderer` crate
> **目标**: 用 cosmic-text 一站式方案替换 fontdb + rustybuzz + ttf-parser 三件套
> **策略**: feature-gated（`cosmic-text`, default off），渐进安全，保留旧路径

## 文档导航

| 文档 | 内容 |
|------|------|
| `PLAN_SPEC.md` | 完整规格：架构决策、DDD 模块设计、API 合约、特效兼容策略、风险 |
| `TASKS.md` | 3 波次原子任务 + 并行执行图 + 退出标准 |

## DDD 模块架构

```
src/cosmic/
  mod.rs          → 模块声明 + 重导出
  resolver.rs     → FontCosmicResolver (FontSystem + AssFallback + SwashCache)
  shaper.rs       → CosmicShaper (Buffer shaping, per-glyph FontKey)
  rasterizer.rs   → SwashImage → tiny-skia Pixmap 桥接 + 异向轮廓
renderer/
  cosmic.rs       → render_event_cosmic() + render_karaoke_cosmic()
  mod.rs          → 添加 cosmic feature 门控分发
```

## 不做的

- ❌ 移除旧 fontdb 路径（保留直到 cosmic-text 验证稳定）
- ❌ GPU 渲染（属于 Sub-6）
- ❌ 字体子集化（属于 Sub-7）
