# PLAN SPEC v2.6 — pgs-encoder & ass2sup-cli 下游重构

> **版本**: v2.6（基于代码库实际状态的全面修订版）
> **状态**: ✅ 已批准，执行中
> **依赖**: color-quantizer 2.5 完成（新 ColorPipeline + 统一类型）✅
> **估算**: ~8.5 天（串行），~5 天（并行 4 轨道）

---

## 架构诊断

### 依赖链

```
color-quantizer (新管线 + 统一类型) ✅ 已完成
    ├── pgs-encoder    (4 源文件 + 6 测试/bench)
    └── ass2sup-cli    (1 源文件 + 6 测试)
```

### 3 个病灶

| # | 病灶 | 位置 | 严重度 |
|---|------|------|--------|
| 1 | 旧 Quantizer API 残留 | `ass2sup-cli/src/lib.rs` 4 处 | 🔴 P1 |
| 2 | `build_palette` 用 `display_height` 硬编码 | `pgs-encoder/src/color.rs:145` | 🔴 P1 |
| 3 | Epoch-split 合成帧 color_space 丢失 | `encoder.rs:306` | 🔴 P1 |

### 计划中自动满足的项（无需额外工作）

| v2.0 项 | 实际状态 | 决策 |
|---------|---------|------|
| `DisplaySetKind` (4 态) | ✅ 已存在 | **跳过** |
| `EpochContinue(0xC0)` | ✅ 已使用 | **跳过** |
| ODS flags 0x80+0x40=0xC0 | ✅ 正确实现 | **跳过** |
| `prev_palette_hash` / `prev_object_rle_hash` | ✅ 已存在 PgsEncoder | **跳过** |
| `AcquirePoint(0x40)` 5 态 | ❌ 从未使用 | **跳过后延** |

---

## 执行波次

### Wave 1: 测试门（2 天，无行为变更）

| # | 文件 | 操作 | 并行 |
|---|------|------|------|
| 1.1 | `tests/proptest.rs` | RLE proptest: `rle_encode(rle_decode(x)) = x` | ⚡ |
| 1.2 | `rle.rs` test mod | RLE 边缘: 全同像素/最大1920宽/交替透明 | ⚡ |
| 1.3 | `rle.rs` test mod | 截断数据拒绝 | ⚡ |
| 1.4 | `encoder.rs` test mod | 多窗口分割测试 | ⚡ |
| 1.5 | `encoder.rs` test mod | Epoch-split 大帧测试 | ⚡ |
| 1.6 | `encoder.rs` test mod | `palette_clear` roundtrip | ⚡ |
| 1.7 | `tests/test_golden.rs` | 确定性子段 golden 测试 | ⚡ |
| 1.8 | `ass2sup-cli/tests/` | CLI 现有测试基线确认 | ⚡ |

### Wave 2: ColorSpace 传播（1 天）

| # | 文件 | 操作 |
|---|------|------|
| 2.1 | `color.rs` | 添加 `color_space_for_height(h)` 辅助函数 |
| 2.2 | `color.rs` | `build_palette` 接受 `ColorSpace` 参数 |
| 2.3 | `encoder.rs` | 更新 4 处 build_palette 调用点 |
| 2.4 | `encoder.rs:306` | 修复 epoch-split 合成帧 color_space |
| 2.5 | `color.rs` tests | BT.709 roundtrip 测试 |

### Wave 3: EpochManager 提取（1 天，纯重组）

| # | 文件 | 操作 |
|---|------|------|
| 3.1 | `epoch.rs` (新建) | 创建 `EpochManager` struct |
| 3.2 | `epoch.rs` | `decide_kind()` — 从 encoder.rs 提取 |
| 3.3 | `epoch.rs` | `update()` — 存储新 hash |
| 3.4 | `encoder.rs` | PgsEncoder 持有 EpochManager |
| 3.5 | `encoder.rs` | `build_display_set` 委托给 EpochManager |
| 3.6 | `tests/proptest.rs` | FSM proptest |

### Wave 4: PotPlayer 兼容（0.5 天）

| # | 文件 | 操作 |
|---|------|------|
| 4.1 | `encoder.rs` | 添加 `potplayer_config.palette_clear_num_objects` |
| 4.2 | `encoder.rs` | 门控 build_palette_clear_display_set |
| 4.3 | `ass2sup-cli/lib.rs` | `--potplayer-compat` / `--no-potplayer-compat` CLI |

### Wave 5: CLI 迁移到 ColorPipeline（1.5 天）

| # | 文件 | 操作 |
|---|------|------|
| 5.1 | `lib.rs:29` | 替换导入 |
| 5.2 | `lib.rs:910` | 构建 `ColorPipeline` |
| 5.3 | `lib.rs:950-968` | 并行路径替换 |
| 5.4 | `lib.rs:1009-1016` | 时序路径替换 |
| 5.5 | `lib.rs:1000` | `prev_palette` → `prev_frame` |
| 5.6 | `lib.rs` Args | 添加 `--color-space` CLI |
| 5.7 | `lib.rs` Args | 添加 `--tonemap` CLI |
| 5.8 | `tests/test_cli.rs` | CLI 集成测试 |

### Wave 6: Golden 测试基础设施（1 天）

| # | 文件 | 操作 |
|---|------|------|
| 6.1 | `tests/golden/` | 创建目录 |
| 6.2 | `test_golden.rs` | RLE golden: 已知向量 SHA-256 |
| 6.3 | `test_golden.rs` | palette golden: BT.709 精确 YCbCr |
| 6.4 | `test_golden.rs` | 编码 golden: 确定性子段 SHA-256 |
| 6.5 | `test_golden.rs` | `UPDATE_GOLDEN=1` 支持 |

### Wave 7: RLE 防御（0.5 天，完全独立）

| # | 文件 | 操作 |
|---|------|------|
| 7.1-7.4 | `tests/proptest.rs` + `rle.rs` | 全同像素/最大行宽/交替透明/截断数据 |

### Wave 8: 稳定性（1 天）

| # | 操作 | 验证 |
|---|------|------|
| 8.1 | 审计 `unwrap()` in encoder.rs | Clippy |
| 8.2 | `#[non_exhaustive]` on public enums | 编译 |
| 8.3 | `Quantizer` `#[deprecated]` | 编译 |
| 8.4-8.7 | workspace clippy/test/doc/fmt | 全绿 |

---

## 依赖图与并行策略

```
Wave 1 ──→ 所有后续依赖
  ├── Wave 2 ──→ Wave 5 需要
  ├── Wave 3 ──→ Wave 5 需要
  ├── Wave 4 (独立于 2/3)
  ├── Wave 7 (完全独立)
  │
  ├── Wave 5 (需要 Wave 2 + Wave 3)
  ├── Wave 6 (独立)
  └── Wave 8 (需要全部)
```

**并行轨道**:
| 轨道 A | 轨道 B | 轨道 C | 轨道 D |
|--------|--------|--------|--------|
| 1→2→3→5→8 | 1→4→8 | 7→8 | 6→8 |

---

## 风险矩阵

| # | 风险 | P | I | 缓解 |
|---|------|---|---|------|
| R1 | PotPlayer `num_objects=0` 崩溃 | M | H | 默认保持 1 |
| R2 | YCbCr f64 vs f32 精度差异 | L | M | 保留 pgs-encoder 用 f64 |
| R3 | `prev_palette` 类型变更 | M | M | `QuantizedFrame` 含 `.palette` |
| R4 | epoch-split color_space 丢失 | H | M | Wave 2.4 修复 |
| R5 | Golden SHA-256 跨平台 | M | L | 仅无字型依赖的逻辑 |
| R6 | `#[deprecated]` 警告 | L | L | `#[allow(deprecated)]` |

---

## 提交策略

```
pgs(test): add RLE proptest + epoch/multi-window/clear roundtrips
pgs(color): ColorSpace propagation — build_palette accepts ColorSpace, fix epoch-split
pgs(epoch): extract EpochManager from encoder — no behavior change
pgs(compat): add PotPlayer workaround toggle, --no-potplayer-compat CLI flag
pgs(rle): add defensive proptests — uniform, max-width, alternating, corrupt
cli(api): migrate to ColorPipeline, add --color-space/--tonemap
pgs(test): add deterministic golden tests (RLE, palette, encode)
chore: stability pass — clippy, deprecated, #[non_exhaustive], fmt
```
