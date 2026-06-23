# Cosmic-Text 字体引擎 — 原子任务分解

## 执行波次

```
Wave 1 (基座)              Wave 2 (3任务并行)        Wave 3 (核心)           Wave 4 (2并行)         Wave 5 (验证)
┌─────────────┐           ┌──────────────────┐    ┌─────────────────┐    ┌──────────────┐     ┌──────────────┐
│ T1: dep +   │───────→  │ T2: FontCosmic-  │───→│ T5: render_     │───→│ T7: lib re-  │────→│ T10: 集成测试│
│   feature   │          │     Resolver     │    │     event_      │    │     export   │     │              │
│             │          ├──────────────────┤    │     cosmic()    │    ├──────────────┤     ├──────────────┤
│ quick, 5min │          │ T3: CosmicShaper │    │ (ultrabrain,    │    │ T8: CLI     │     │ T11: fmt +  │
└─────────────┘          │ (shaper_cosmic)  │    │  60min)         │    │ --cosmic-   │     │ clippy + doc│
                         ├──────────────────┤    ├─────────────────┤    │ text flag   │     │ (10min)     │
 已就绪                   │ T4: 字形光栅化    │    │ T6: cosmic-    │    │ (quick,     │     └──────────────┘
  ✅ dep: cosmic-text     │ (rasterizer_)     │    │     text       │    │  10min)     │
  ✅ feature gate         └──────────────────┘    │     karaoke    │    └──────────────┘
  ✅ cosmic/ 子目录                                │ (deep, 30min)  │
                                                  └─────────────────┘
```

## 原子任务清单

### Wave 1: 基座（已就绪 ✅）

| # | 文件 | 动作 | 验证 | 状态 |
|---|------|------|------|------|
| 1 | `Cargo.toml`, `subtitle-renderer/Cargo.toml` | 添加 cosmic-text 0.19 dep + feature gate | `cargo check -p subtitle-renderer --features cosmic-text` | ✅ |

### Wave 2: 核心组件（3 并行）

| # | 文件 | 模块 | 产出 | 关键测试 | 估算 |
|---|------|------|------|---------|------|
| 2 | `cosmic/resolver.rs` | FontCosmicResolver | FontSystem + SwashCache + AssFallback | 6 单元测试 | ~250 LOC, 45min |
| 3 | `cosmic/shaper.rs` | CosmicShaper | Buffer shaping + per-glyph FontKey | 5 单元测试 | ~180 LOC, 30min |
| 4 | `cosmic/rasterizer.rs` | 字形光栅化 | SwashImage → Pixmap + aniso outline | 4 单元测试 | ~200 LOC, 35min |

### Wave 3: 核心管线（依赖 Wave 2）

| # | 文件 | 模块 | 产出 | 关键测试 | 估算 |
|---|------|------|------|---------|------|
| 5 | `renderer/cosmic.rs` | render_event_cosmic | 全管线：字体→塑形→光栅化→effects→composite | 旧路径继续通过 | ~350 LOC, 60min |
| 6 | `renderer/cosmic.rs` (续) | render_karaoke_cosmic | 卡拉OK per-syllable 塑形+光栅化 | 卡拉OK 测试 | ~200 LOC, 30min |

### Wave 4: 集成（2 并行）

| # | 文件 | 动作 | 估算 |
|---|------|------|------|
| 7 | `src/lib.rs` | Re-export cosmic 类型 | ~10 LOC, 5min |
| 8 | `ass2sup-cli/src/lib.rs` | `--cosmic-text` CLI 标志 | ~25 LOC, 10min |

### Wave 5: 验证

| # | 文件 | 动作 | 估算 |
|---|------|------|------|
| 9 | `tests/test_font_cosmic.rs` | FontCosmicResolver 单元测试 | ~120 LOC, 20min |
| 10 | `tests/test_cosmic_vs_existing.rs` | 像素级回归对比（7 场景） | ~150 LOC, 25min |
| 11 | 全工作区 | fmt + clippy + doc | 10min |

## 退出标准

```
□ cargo check -p subtitle-renderer (no features) — 零错误（旧路径不变）
□ cargo check -p subtitle-renderer --features cosmic-text — 零错误
□ cargo clippy -p subtitle-renderer --features cosmic-text -- -D warnings — 零警告
□ cargo test -p subtitle-renderer — 全部通过（旧路径 515+ 测试）
□ cargo test -p subtitle-renderer --features cosmic-text — 新增 22+ 测试全部通过
□ FontCosmicResolver 单元测试：空→系统→CJK→图像→粗体→load_file (6)
□ CosmicShaper 单元测试：基础→CJK→混合→空→advance (5)
□ 光栅化测试：填充→轮廓→空→彩色 (4)
□ 像素回归测试：简单→CJK→粗体→轮廓→阴影→模糊→混合 (7)
□ --cosmic-text CLI 标志存在且文档完整
□ 所有 public 项有 rustdoc（#[warn(missing_docs)]）
□ fmt 无漂移
```
