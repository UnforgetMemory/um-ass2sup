# 🧩 PGS 编码器设计

> 领域驱动设计（DDD）—— domain/（纯模型） + encoding/（序列化）

---

## 📋 目录

- [设计哲学](#设计哲学)
- [模块结构](#模块结构)
- [domain/ — 纯领域模型](#domain--纯领域模型)
- [encoding/ — 编码实现](#encoding--编码实现)
- [PGS 段类型](#pgs-段类型)
- [显示集生命周期](#显示集生命周期)
- [关键架构约束](#关键架构约束)
- [PotPlayer 兼容性](#potplayer-兼容性)
- [代码示例](#代码示例)
- [基准性能](#基准性能)

---

## 设计哲学

PGS 编码器采用**领域驱动设计**（Domain-Driven Design），将纯领域模型与编码/序列化关注点严格分离：

```
┌─────────────────────────────────────────────┐
│              domain/（纯模型）                │
│  组合状态、对象组合、窗口、调色板、RLE        │
│  零 I/O、零编码知识、纯数据类型               │
├─────────────────────────────────────────────┤
│              encoding/（编码）                │
│  显示集构建、PGS 二进制序列化、SUP 文件写入    │
│  知道如何把 domain 对象变成字节               │
└─────────────────────────────────────────────┘
```

**核心原则：**

- `domain/` 中的类型不知道如何序列化为 PGS 二进制——它们只是领域概念
- `encoding/` 知道如何将领域对象转为 PGS 段，但不依赖 I/O
- 领域错误携带上下文信息（缺失的调色板、无效的组合状态等）
- Pipeline （`PgsEncoder::encode_frame`）位于 `encoding/`，编排从帧到显示集的完整流程

---

## 模块结构

```
crates/pgs-encoder/src/
│
├── domain/                         # 纯领域模型 — 无 I/O，无编码知识
│   ├── composition.rs              # CompositionState, ObjectComposition, WindowDef
│   ├── epoch.rs                    # EpochManager — 对象版本化，epoch 生命周期
│   ├── palette.rs                  # PaletteEntry, YCbCr 转换, 颜色交换
│   ├── segment.rs                  # Segment, SegmentPayload (PCS/WDS/PDS/ODS/END), SupFile
│   ├── rle.rs                      # RLE 编码, chunk_rle_data
│   ├── timing.rs                   # 帧率代码, ms_to_90khz 转换
│   └── mod.rs                      # 重导出
│
├── encoding/                       # 编码 — 领域对象如何序列化为二进制
│   ├── display_set.rs              # DisplaySet 构建器：EpochStart/NormalCase/EpochContinue/PaletteOnly
│   ├── encoder.rs                  # PgsEncoder — 帧 → 显示集管线
│   ├── sup.rs                      # SUP 文件写入器
│   └── mod.rs                      # 重导出
│
├── color.rs                        # 颜色类型重导出
├── encoder.rs                      # 旧版编码器（部分；新逻辑在 encoding/）
├── epoch.rs                        # 旧版 epoch（部分；新逻辑在 domain/epoch.rs）
├── lib.rs                          # crate 根
├── rle.rs                          # 旧版 RLE（部分）
└── types.rs                        # 旧版类型别名
```

> 标记为"旧版"的文件是 DDD 迁移 Wave 1 完成后的残留。新代码应直接使用 `domain/` 和 `encoding/`。

---

## domain/ — 纯领域模型

### composition.rs

定义显示集的内容模型：

| 类型 | 职责 |
|------|------|
| `CompositionState` | 组合状态（EpochStart / NormalCase / EpochContinue / PaletteOnly） |
| `ObjectComposition` | 单个对象的组合描述（裁剪矩形、坐标、对象版本） |
| `WindowDef` | 显示窗口定义（x, y, width, height） |

### epoch.rs

Epoch（时期）是 PGS 的核心生命周期概念：

| 类型 | 职责 |
|------|------|
| `EpochManager` | 对象版本化、epoch 生命周期追踪 |
| 版本管理 | 新帧 → 检测对象是否变化 → 增量或全量更新 |
| epoch 类型选择 | 自动选择 EpochStart / NormalCase / EpochContinue / PaletteOnly |

### palette.rs

调色板的纯数据定义：

| 类型 | 职责 |
|------|------|
| `PaletteEntry` | 单条调色板条目（Y, Cb, Cr, Alpha） |
| YCbCr 转换 | RGBA → YCbCr（Rec.601 / Rec.709） |
| 颜色交换 | 调色板内颜色替换 |

### segment.rs

PGS 段的纯数据表示：

| 类型 | 职责 |
|------|------|
| `Segment` | PGS 段（长度前缀 + 时间戳 + 类型 + 负载） |
| `SegmentPayload` | 段负载枚举（PCS / WDS / PDS / ODS / END） |
| `SupFile` | SUP 文件（`Vec<Segment>`） |

### rle.rs

RLE（游长编码）是 PGS 对位图数据进行压缩的方式：

```
ODS 中的位图数据使用 RLE 压缩
格式：像素数据交替编码为"运行"（相同像素的连续序列）
```

| 函数 | 职责 |
|------|------|
| `rle_encode` | 索引位图 → RLE 字节序列 |
| `chunk_rle_data` | 将 RLE 数据分块以适应 PGS 段大小限制 |

### timing.rs

PGS 时序转换：

| 函数 | 职责 |
|------|------|
| `ms_to_90khz` | 毫秒 → 90 kHz PTS 时钟 |
| 帧率代码 | FPS → PGS 帧率枚举代码 |

---

## encoding/ — 编码实现

### display_set.rs

四种显示集类型的构建器：

| 类型 | 用途 | 触发条件 |
|------|------|---------|
| **EpochStart** | 新 epoch 开始 | 场景切换或首次编码 |
| **NormalCase** | 对象完全变化 | 对象内容/尺寸/位置改变 |
| **EpochContinue** | 对象部分或未变化 | 仅更新 PTS，复用对象版本 |
| **PaletteOnly** | 仅调色板变化 | 淡入淡出效果，无需重编码 ODS |

### encoder.rs

`PgsEncoder` 是编码器的主入口：

```rust
PgsEncoder::new(1920, 1080, FrameRate::PsF23976)
    .encode_frame(indexed_frame, 1000)  // 1000 ms
    .encode_frame(next_frame, 2000)
    .finalize()  // 刷入 END 段
    .to_bytes()  // → Vec<u8> / SUP 文件
```

**Encoder 职责：**
1. 接收量化后的 `IndexedFrame`
2. 决定 epoch 类型（EpochStart / NormalCase / EpochContinue / PaletteOnly）
3. 构建显示集（PCS + WDS + PDS + ODS 序列）
4. 管理 `composition_number` 递增
5. 处理多对象拆分（`chunks(2)`）

### sup.rs

`SupWriter` 负责将 `Vec<Segment>` 写入文件：

- 写入 `PGS Magic`（`PG`）
- 序列化每个 `Segment` 为长度前缀的二进制块
- 同步写入

---

## PGS 段类型

PGS 字幕流由五种段类型构成：

| 段 | 全称 | 用途 | 内容 |
|----|------|------|------|
| **PCS** | Presentation Composition Segment | 组合控制 | 对象数量、窗口、组合状态（EpochStart/NormalCase/EpochContinue/PaletteOnly） |
| **WDS** | Window Definition Segment | 窗口定义 | 显示窗口位置和尺寸 |
| **PDS** | Palette Definition Segment | 调色板定义 | 颜色查找表（Y/Cb/Cr/Alpha 条目） |
| **ODS** | Object Definition Segment | 对象定义 | RLE 压缩的位图数据 |
| **END** | End of Display Set Segment | 显示集结束 | 显示集终止符 |

**典型显示集序列：**

```
EpochStart:  PCS + WDS + PDS + ODS + END
NormalCase:  PCS + WDS + PDS + ODS + END
EpochContinue: PCS + WDS + END              （ODS 复用）
PaletteOnly:   PCS + PDS + END              （无 ODS / WDS）
```

---

## 显示集生命周期

### Epoch（时期）概念

Epoch 是一系列共享相同"对象版本"的显示集。只要对象内容不变，后续的 NormalCase 或 EpochContinue 显示集可以引用之前版本的 ODS，无需重新传输位图数据。

```
Epoch 1 (EpochStart):  对象版本 1
  ├─ Frame 1: ODS v1  ← 包含完整位图
  ├─ Frame 2: NormalCase → 引用 ODS v1
  ├─ Frame 3: EpochContinue → 引用 ODS v1
  └─ Frame 4: NormalCase → 引用 ODS v1

Epoch 2 (EpochStart):  对象版本 2（内容变化）
  ├─ Frame 5: ODS v2  ← 新的完整位图
  └─ ...
```

### composition_number

- 每次 `encode_frame()` 后递增（包括 NormalCase 和 EpochContinue）
- 使用 `wrapping_add`——到最大值后自动回绕
- 蓝光播放器通过此编号检测显示集变更

---

## 关键架构约束

记录于 AGENTS.md 中的项目记忆，来自实际测试：

### MAX_OBJECT_REFS=2

**PotPlayer 兼容坑：** PotPlayer 在 PCS 中遇到超过 2 个对象引用时会崩溃。

- 解决方案：`chunks(2)` 自动拆分多对象显示集
- 编码器确保任何单个 PCS 中的 `num_objects` ≤ 2

### palette_update=true

**PotPlayer 需求：** PotPlayer 要求所有 PCS 设置 `palette_update=true`。

- `num_objects=0` 在 palette_clear 中导致 PotPlayer 崩溃
- 编码器在所有 PCS 上强制 `palette_update`

### 淡入淡出处理

**逐帧闪烁问题：** 淡入淡出事件的显示 PCS 如果使用不透明度 >0 的调色板，会产生 1 帧全显示闪白。

- 解决方案：使用 `encode_multi_object_display_set_with_alpha(Some(0))` 强制 alpha=0 调色板
- 确保淡入的首帧完全透明，避免闪白

### composition_number 递增

**每次 encode_frame 都递增：** `composition_number` 在每次 `encode_frame` 后使用 `wrapping_add` 递增——包括 NormalCase 和 EpochContinue，不仅仅是 EpochStart。

---

## PotPlayer 兼容性

PotPlayer 是蓝光字幕的最常见测试平台。编码器通过以下措施确保兼容性：

| 措施 | 原因 |
|------|------|
| `MAX_OBJECT_REFS=2` | PotPlayer 崩溃于 >2 对象 |
| 所有 PCS 标记 `palette_update=true` | PotPlayer 行为要求 |
| 淡入淡出 alpha=0 调色板 | 防止 1 帧全白闪光 |
| composition_number 正确递增 | 播放器正确的状态机同步 |
| YCbCr Rec.601 色彩矩阵 | 蓝光标准规范 |

---

## 代码示例

### 基本编码流程

```rust
use pgs_encoder::domain::timing::FrameRate;
use pgs_encoder::encoding::encoder::PgsEncoder;

// 创建编码器（1920×1080, 23.976 fps）
let mut encoder = PgsEncoder::new(1920, 1080, FrameRate::PsF23976);

// 编码帧（frame 是 color-quantizer 输出的 IndexedFrame）
let segments = encoder.encode_frame(frame, pts_ms: 1000);
// segments: Vec<Segment> — 可以写入 SUP 文件

// 下一帧
let segments = encoder.encode_frame(frame2, pts_ms: 2000);

// 结束
let end_segments = encoder.finalize();

// 写入 SUP
use pgs_encoder::encoding::sup;
sup::write_sup("output.sup", &encoder)?;
```

### 作为库使用

```toml
# Cargo.toml
[dependencies]
pgs-encoder = "2.7"
```

```rust
use pgs_encoder::encoding::encoder::PgsEncoder;
use pgs_encoder::domain::segment::{Segment, SupFile};
```

```bash
cargo run --release --example encode_sup -p pgs-encoder
```

---

## 基准性能

| 基准 | 规模 | 中位耗时 |
|------|------|---------|
| `pgs_encode_medium_320x180` | 320×180 | 90.3 µs |
| `pgs_encode_ntsc_320x180` | 320×180 | 91.1 µs |
| `rle_small_64x32` | 64×32 | 2.84 µs |
| `rle_large_1920x1080` | 1080p | 2.45 ms |

---

<p align="center">
  <sub>← [开发指南](development.md) | [返回首页](index.md) | 下一篇：[色彩量化管线](color-quantizer.md) →</sub>
</p>
