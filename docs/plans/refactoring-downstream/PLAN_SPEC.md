# PLAN SPEC — pgs-encoder & ass2sup-cli 全面底层重构

> **版本**: v2.0（全面重构版）
> **状态**: ⏳ 待审核
> **依赖**: color-quantizer 2.5 完成（新 ColorPipeline + 统一类型）
> **估算**: 10~13 天（串行），8 天（并行）

---

## 1. 架构诊断

### 1.1 依赖链

```
color-quantizer (新管线 + 统一类型)
    ├── pgs-encoder    (4 源文件 + 6 测试/bench + ~2000 行)
    └── ass2sup-cli    (1 源文件 + 6 测试 + ~1200 行)
```

### 1.2 12 个"已知问题"的真相诊断

通过对代码库的实际审核，发现之前列举的 12 个问题中有 **4 个实际上已经修复**：

| # | 问题 | 真实状态 | 处理 |
|---|------|---------|------|
| 1 | `CompositionState::EpochContinue(0xC0)` 枚举缺失 | ✅ **已存在** `types.rs:47` | 无需处理 |
| 2 | `palette_update` 始终 true | 🟡 **部分正确**——encoder 逻辑已条件化，但 `build_palette_clear_display_set` 强制 `palette_update=true` + `num_objects=1`（PotPlayer workaround） | Wave 4 |
| 3 | ODS flags 0xC0 vs 0x80 | ✅ **已正确**——`first_in_sequence=0x80, last_in_sequence=0x40` composited 为 0xC0 | 无需处理 |
| 4 | ODS total_size 3字节 LE 格式 | ✅ **实际是 BE，正确**——`encoder.rs:356-359` | 无需处理 |
| 5 | Epoch 管理：每帧 EpochStart | 🟡 **部分正确**——已有 4 态 DisplaySetKind + hash 检测，但 `AcquirePoint(0x40)` 从未使用 | Wave 3 |
| 6 | PDS YCbCr 字节序 | ✅ **自洽且符合 spec**——序列化 Y→Cr→Cb，解码同样顺序 | 无需处理 |
| 7 | 多对象 ODS 兼容性 | 🟢 **低优先级**——从未测试 | Wave 6 |
| 8 | `build_palette` 硬编码 BT.709/BT.601 | 🔴 `color.rs:146`：`display_height > 576`，`frame.color_space` 从未读取 | Wave 2 |
| 9 | `color_space` 从不读取 | 🔴 编码器所有路径忽略 `frame.color_space` | Wave 2 |
| 10 | 无参考 SUP 基线 | 🔴 零 golden 文件测试 | Wave 1 + 6 |
| 11 | RLE 透明行分隔符歧义 | 🟢 兼容但未充分测试 | Wave 7 |
| 12 | PotPlayer palette_update workaround | 🟡 `build_palette_clear_display_set` 因 PotPlayer 崩溃而设 `num_objects=1` | Wave 4 |

### 1.3 真正需要做的事（修订后优先级）

| 优先级 | 问题 | 影响 |
|--------|------|------|
| 🔴 P1 | `color_space` 不传播 | 功能缺失 |
| 🔴 P2 | 无 golden SUP 基线 | 质量风险 |
| 🟡 P3 | Epoch 管理器紧耦合 | 规范合规 |
| 🟡 P4 | PotPlayer workaround 无法关闭 | 播放器兼容锁死 |
| 🟡 P5 | CLI 仍用旧 `Quantizer` API | 技术债务 |
| 🟢 P6 | RLE 边界测试不足 | 防御性 |

---

## 2. 目标架构

### 2.1 pgs-encoder 模块图

```
pgs-encoder/src/
├── color.rs             ← 重构：build_palette 接受 ColorSpace 参数
│   ├── ycbcr_to_rgba()     保留（f64，解码必需）
│   ├── palette_to_rgba()   保留
│   ├── swap()              保留（RLE 必需）
│   ├── rgba_to_ycbcr()     重构：接受 ColorSpace 参数
│   └── build_palette()     重构：接受 ColorSpace，移除 display_height
│
├── encoder.rs           ← 拆分出 epoch.rs
│   ├── PgsEncoder         持有 EpochManager
│   └── segment builders   精简，委托给 EpochManager
│
├── epoch.rs             ★ 新增
│   ├── EpochManager { state, counters, hashes }
│   └── 五态 FSM：EpochStart / NormalCase / AcquirePoint / EpochContinue / PaletteOnly
│
├── types.rs             小幅清理
├── decoder.rs           小幅清理
├── rle.rs               + 边界测试
├── decode_to_image.rs   不变
└── tests/
    ├── golden/           ★ 新增：参考 .sup 二进制
    └── golden_tests.rs   ★ 新增：SHA-256 哈希比较
```

### 2.2 Epoch 五态状态机

```
                    ┌─────────────────────────┐
                    │    EpochStart (0x80)    │←── 首帧/大帧切分
                    │  PCS+WDS+PDS+ODS+END    │
                    └───────────┬─────────────┘
                                │
                    ┌───────────▼─────────────┐
                    │  hash(object) changed?  │
                    └──────┬──────────┬───────┘
                      YES  │          │  NO
                           ▼          ▼
              ┌──────────────────┐  ┌───────────────────┐
              │ NormalCase(0x00) │  │ hash(palette)     │
              │ PCS+WDS+PDS+ODS │  │ changed?           │
              └──────────────────┘  └──────┬─────┬──────┘
                                      YES  │     │  NO
                                           ▼     ▼
                                    ┌──────────┐ ┌──────────────┐
                                    │PaletteOnly│ │EpochContinue │
                                    │(PCS→PDS→ │ │ (0xC0)       │
                                    │ END)     │ │ PCS+END      │
                                    └──────────┘ └──────────────┘
```

**AcquirePoint(0x40)**：每 N 帧（默认 300 ≈ 10s @ 29.97fps）插入作为重新同步点。

### 2.3 色彩管线数据流

```
ColorPipeline
  .with_color_space(ColorSpace::Bt709)
  .with_tonemap(ToneMapOperator::Reinhard)
        │
        ▼
QuantizedFrame { color_space: ColorSpace::Bt709, ... }
        │
        ▼
PgsEncoder::encode_frame(&frame, pts, duration)
        │
        ├── EpochManager::decide() → DisplaySetKind
        ├── build_palette(frame.palette, frame.color_space)
        │     → Vec<PaletteEntry>
        ├── rle_encode(frame.indices, ...)
        └── segment assembly → SUP
```

---

## 3. 原子级任务分解

### Wave 1：测试门（3 天，无行为变更）

| ID | 任务 | 文件 | 并行 | 验证门 |
|----|------|------|------|--------|
| 1.1 | 创建 `tests/golden/` + 参考 SUP 生成脚本 | `tests/golden/` | — | 脚本可运行 |
| 1.2 | `golden_tests.rs`：编码已知帧哈希比较 | `tests/golden_tests.rs` | ⚡ | SHA-256 |
| 1.3 | proptest：`rle_encode(rle_decode(x)) = x` | `rle.rs` | ⚡ | 500 用例 |
| 1.4 | proptest：`encode(decode(bytes))` 结构级 | `decoder.rs` | ⚡ | 200 用例 |
| 1.5 | `build_palette` ColorSpace 已知 YCbCr 测试 | `color.rs` | ⚡ | 向量匹配 |
| 1.6 | palette_clear 有效 SUP 测试 | `encoder.rs` | ⚡ | verify_roundtrip |
| 1.7 | 多窗口拆分测试 | `encoder.rs` | ⚡ | verify_roundtrip |
| 1.8 | epoch-split 大帧测试 | `encoder.rs` | ⚡ | verify_roundtrip |

### Wave 2：ColorSpace 传播（2 天）

| ID | 任务 | 文件 | 并行 | 验证门 |
|----|------|------|------|--------|
| 2.1 | `build_palette` 接受 `ColorSpace` | `color.rs` | ⚡ | 现有 roundtrip |
| 2.2 | `display_height` 改为 `ColorSpace` | `color.rs` + 调用者 | ⚡ | Wave 1 golden |
| 2.3 | 编码器 3 个调用点更新 | `encoder.rs` | ⚡ | 全部测试 |
| 2.4 | epoch-split 合成帧传播 color_space | `encoder.rs` | ⚡ | verify_roundtrip |
| 2.5 | PgsEncoder 可选默认 ColorSpace | `encoder.rs` | | Clippy |
| 2.6 | `color_space_for_height(h)` 辅助函数 | `color.rs` | ⚡ | 向后兼容 |

### Wave 3：Epoch 管理器（2 天）

| ID | 任务 | 文件 | 并行 | 验证门 |
|----|------|------|------|--------|
| 3.1 | 创建 `EpochManager` | `epoch.rs` | ⚡ | Wave 1 golden |
| 3.2 | 移入 hash 跟踪 | `epoch.rs` | ⚡ | 测试通过 |
| 3.3 | `acquire_point_interval` | `epoch.rs` | ⚡ | 每 300=AcquirePoint |
| 3.4 | 提取 `decide_kind()` | `encoder.rs→epoch.rs` | ⚡ | 测试通过 |
| 3.5 | PgsEncoder 委托 EpochManager | `encoder.rs` | | Clippy |
| 3.6 | FSM proptest | `epoch.rs` | ⚡ | proptest |
| 3.7 | AcquirePoint 间隔测试 | `epoch.rs` | ⚡ | 单元测试 |

### Wave 4：PotPlayer 兼容（1 天）

| ID | 任务 | 文件 | 并行 | 验证门 |
|----|------|------|------|--------|
| 4.1 | `PotPlayerCompat { enable }` | `encoder.rs` | ⚡ | 默认 true |
| 4.2 | palette_update gate | `encoder.rs` | ⚡ | Golden 验证 |
| 4.3 | palette_clear num_objects gate | `encoder.rs` | ⚡ | 两种模式 |
| 4.4 | `--no-potplayer-workaround` CLI | `ass2sup-cli` | ⚡ | 集成测试 |
| 4.5 | 删除过时注释 | `decoder.rs` | ⚡ | — |

### Wave 5：CLI 迁移（2 天）

| ID | 任务 | 文件 | 并行 | 验证门 |
|----|------|------|------|--------|
| 5.1 | `--color-space` CLI | `lib.rs Args` | ⚡ | 参数解析 |
| 5.2 | `--tonemap` CLI | `lib.rs Args` | ⚡ | 参数解析 |
| 5.3 | 时序路径 ColorPipeline | `lib.rs:997-1035` | | verify_roundtrip |
| 5.4 | 并行路径 ColorPipeline | `lib.rs:950-968` | ⚡ | 测试通过 |
| 5.5 | quantize_with_prev 替换 | `lib.rs:1009-1016` | ⚡ | 测试通过 |
| 5.6 | color_space 全链传递 | `lib.rs` | | 集成测试 |
| 5.7 | `--color-space bt709` 集成测试 | `tests/` | ⚡ | verify_roundtrip |
| 5.8 | 删除旧导入 | `lib.rs:29` | ⚡ | 编译 |

### Wave 6：Golden 测试（1 天）

| ID | 任务 | 文件 | 并行 | 验证门 |
|----|------|------|------|--------|
| 6.1 | 生成参考 SUP（5 场景） | `tests/golden/` | | — |
| 6.2 | 哈希 golden 测试 | `tests/golden_tests.rs` | ⚡ | SHA-256 |
| 6.3 | SUP→PNG→OCR 测试 | `decode_to_image.rs` | ⚡ | 文本匹配 |
| 6.4 | `UPDATE_GOLDEN=1` CI 门 | `tests/golden/` | ⚡ | CI gate |

### Wave 7：RLE 防御（1 天）

| ID | 任务 | 文件 | 并行 | 验证门 |
|----|------|------|------|--------|
| 7.1 | 全同像素最大压缩 | `rle.rs` | ⚡ | rle→decode |
| 7.2 | 最大行宽 1920px | `rle.rs` | ⚡ | rle→decode |
| 7.3 | 交替透明/不透明 | `rle.rs` | ⚡ | rle→decode |
| 7.4 | 截断数据拒绝 | `rle.rs` | ⚡ | 返回错误 |

### Wave 8：稳定性（1 天）

| ID | 任务 | 文件 | 并行 | 验证门 |
|----|------|------|------|--------|
| 8.1 | 审计 `unwrap()` | `encoder.rs` | ⚡ | Clippy |
| 8.2 | `#[non_exhaustive]` | `types.rs` | ⚡ | 编译 |
| 8.3 | clippy workspace | Workspace | | 零警告 |
| 8.4 | test workspace | Workspace | | 全绿 |
| 8.5 | doc test | Workspace | | 全绿 |
| 8.6 | fmt | Workspace | | 无漂移 |

---

## 4. 依赖图

```
Wave 1 (测试门)  ←── 所有后续依赖
  ├── Wave 2 (ColorSpace)   需要 1.5
  │   ├── Wave 3 (Epoch)    需要 1.2，与 Wave 2 独立
  │   └── Wave 4 (PotPlayer) 需要 3.5 + 2.1
  ├── Wave 5 (CLI)          需要 2.6 + 3.5
  ├── Wave 6 (Golden)       需要 1.1, 1.2
  └── Wave 7 (RLE)          独立
Wave 8 (稳定性)              需要所有前述 Wave
```

**可并行执行**：Wave 2 ↔ Wave 3。Wave 7 完全独立。

---

## 5. 验证门

1. **RLE roundtrip**: `rle_encode(rle_decode(x)) = x` ∀ 有效输入
2. **YCbCr roundtrip**: `rgba→ycbcr(ycbcr→rgba)` 每分量 ±1
3. **Epoch FSM**: EpochStart 后只允许 NormalCase/EpochContinue/PaletteOnly
4. **Golden SUP**: 编码器输出 = SHA-256 参考
5. **verify_roundtrip**: 任何编码输出通过自身验证

---

## 6. 时间线

| Wave | 天数 | 累计 | 交付物 |
|------|------|------|--------|
| 1: 测试门 | 3 | 第 3 天 | 测试就绪，无行为变更 |
| 2: ColorSpace | 2 | 第 5 天 | `build_palette` 接受 `ColorSpace` |
| 3: Epoch | 2 | 第 7 天 | `EpochManager` 提取 |
| 4: PotPlayer | 1 | 第 8 天 | 兼容性可配置 |
| 5: CLI | 2 | 第 10 天 | 完全 ColorPipeline |
| 6: Golden | 1 | 第 11 天 | 参考 SUP 入库 |
| 7: RLE | 1 | 第 12 天 | RLE 边界测试 |
| 8: 稳定性 | 1 | 第 13 天 | 全 CI 绿 |

**总计**：10~13 天 / 并行 ~8 天

---

## 7. 风险矩阵

| # | 风险 | 可能性 | 影响 | 缓解 |
|---|------|--------|------|------|
| R1 | PotPlayer 因 `palette_update=false` 回退 | 中 | 高 | 默认保持 true，验证后翻转 |
| R2 | RLE 行分隔符与罕见像素冲突 | 低 | 中 | Wave 7 + proptest |
| R3 | Epoch FSM 边缘帧与当前行为不同 | 中 | 中 | Wave 1 golden 捕获基线 |
| R4 | ColorSpace 传播遗漏路径 | 中 | 中 | epoch-split 合成帧须复制 |
| R5 | 旧 API 删除破坏 bdn-xml | 低 | 低 | bdn-xml 直接使用 color_quantizer |
| R6 | Golden SUP 平台相关 | 低 | 低 | RLE 确定，CI 固定 |

---

## 8. 提交策略

格式: `pgs(范围): 操作 — 理由`

每个 Wave 完成后合并到 `main`。不批量合并。
