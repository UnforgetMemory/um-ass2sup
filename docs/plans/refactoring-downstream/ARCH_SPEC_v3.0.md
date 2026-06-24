# ARCH SPEC v3.0 — pgs-encoder DDD 整洁架构重构

> **类型**: 纯架构重组，零行为变更
> **状态**: ✅ 执行中
> **估算**: 4~6 天
> **策略**: 先新建子模块迁移代码，再删除旧文件

---

## 目标架构

```
crates/pgs-encoder/src/
├── lib.rs                       ← 重导出所有 pub API（保持向下兼容）
│
├── domain/                      ← 领域层：纯数据+纯函数，无 I/O
│   ├── mod.rs                   ← 重导出子模块
│   ├── composition.rs           ← CompositionState, DisplaySetKind, ObjectComposition
│   ├── epoch.rs                 ← EpochManager (从现有 epoch.rs 迁移)
│   ├── palette.rs               ← PaletteEntry, color_space_for_height, swap
│   ├── rle.rs                   ← rle_encode, chunk_rle_data (仅编码，不包含解码)
│   ├── segment.rs               ← Segment, SegmentType, SegmentPayload, 所有Payload类型
│   └── timing.rs                ← frame_rate_code, is_ntsc_fps, ms_to_90khz
│
├── encoding/                    ← 编码应用层
│   ├── mod.rs                   ← 重导出
│   ├── encoder.rs               ← PgsEncoder struct (大幅精简, ~300行)
│   ├── display_set.rs           ← build_display_set + build_*_display_set 方法
│   └── sup.rs                   ← SupFile to_bytes 序列化
│
├── decoding/                    ← 解码应用层
│   ├── mod.rs
│   ├── decoder.rs               ← decode_sup, verify_roundtrip 入口
│   ├── parser.rs                ← 各段类型解析逻辑
│   └── image.rs                 ← decode_frame_to_rgba, frame_to_png
│
└── tests/                       ← 测试移到独立目录(可选,可保留在crate根tests/)
```

## Wave 0: 基础设施（~30min）

| # | 操作 | 验证 |
|---|------|------|
| 0.1 | 创建子模块目录: `src/domain/`, `src/encoding/`, `src/decoding/` | 目录存在 |
| 0.2 | 创建 `mod.rs` 文件 + lib.rs 添加 `pub mod` | cargo check 通过 |
| 0.3 | git checkpoint | — |

## Wave 1: domain/ 层（1.5 天）
策略: 创建新文件, 复制代码, 编译通过, 删除旧文件

| # | 操作 | 验证 |
|---|------|------|
| 1.1 | `domain/timing.rs`: 从 encoder.rs 提取 `frame_rate_code`, `is_ntsc_fps`, `ms_to_90khz`, `timecode_to_ms` | cargo test |
| 1.2 | `domain/composition.rs`: 从 types.rs 提取 `CompositionState`, `ObjectComposition`, `WindowDef`, `CompositionDescriptor` | cargo test |
| 1.3 | `domain/palette.rs`: 从 color.rs 提取 `PaletteEntry`, `PcsPayload`, `PdsPayload`, `color_space_for_height`, `swap()` + 从 types.rs 提取 PaletteEntry | cargo test |
| 1.4 | `domain/segment.rs`: 从 types.rs 提取 `Segment`, `SegmentType`, `SegmentPayload`, `OdsPayload`, `WdsPayload`, `EndPayload` | cargo test |
| 1.5 | `domain/epoch.rs`: 从现有 epoch.rs 迁移（需更新 mod 引用） | cargo test |
| 1.6 | `domain/rle.rs`: 从 rle.rs 提取 `rle_encode` + `chunk_rle_data`（不含 `rle_decode`） | cargo test |

## Wave 2: encoding/ 层（1.5 天）

| # | 操作 | 验证 |
|---|------|------|
| 2.1 | `encoding/sup.rs`: 从 types.rs 提取 `SupFile` + to_bytes 方法 | cargo test |
| 2.2 | `encoding/display_set.rs`: 从 encoder.rs 提取 `build_display_set`, `build_single_window_display_set`, `build_multi_window_display_set`, `build_epoch_split_display_set`, `build_palette_clear_display_set`, `build_continue_display_set`, `build_palette_only_display_set`, `find_split_row` | cargo test |
| 2.3 | `encoding/encoder.rs`: 精简的 `PgsEncoder` struct 保留 `new`, `encode_frame`, `encode_frame_to_bytes`, `ms_to_90khz(wrapper)`+ 测试 | cargo test |

## Wave 3: decoding/ 层（1 天）

| # | 操作 | 验证 |
|---|------|------|
| 3.1 | `decoding/parser.rs`: 从 decoder.rs 提取段解析函数 (`parse_pcs`, `parse_wds`, `parse_pds`, `parse_ods`, `parse_end`) | cargo test |
| 3.2 | `decoding/decoder.rs`: `decode_sup` + `verify_roundtrip` + `DisplaySet` + `ParsedPayload` + `ParsedSegment` (精简后) | cargo test |
| 3.3 | `decoding/image.rs`: 从 decode_to_image.rs 迁移 `decode_frame_to_rgba`, `frame_to_png` 等 | cargo test |

## Wave 4: 清理 + lib.rs 重导出（1 天）

| # | 操作 | 验证 |
|---|------|------|
| 4.1 | 删除旧文件: `types.rs`, `color.rs`, `decoder.rs`, `decode_to_image.rs`, `encoder.rs`, `rle.rs` (epoch.rs 保留但被 domain 替代) | 编译检查 |
| 4.2 | `lib.rs` 更新 `pub use` 重导出：确保所有 pub API 向下兼容 | cargo doc |
| 4.3 | 更新 `examples/` 中的 import 路径 | cargo build |
| 4.4 | 更新 `tests/` 中的 import 路径 | cargo test |
| 4.5 | workspace clippy + test + doc + fmt | 全绿 |

## 依赖图

```
Wave 0 (基础设施) → Wave 1 (domain/) → Wave 2 (encoding/) + Wave 3 (decoding/)
                                       → Wave 4 (清理+重导出)
```

Wave 2 和 Wave 3 可并行执行。

## 风险矩阵

| # | 风险 | P | I | 缓解 |
|---|------|---|---|------|
| R1 | pub API 在新路径下不可见 | H | H | lib.rs 用 `pub use domain::*` + `pub use encoding::*` + `pub use decoding::*` 保持兼容 |
| R2 | 循环依赖: domain 引用了 encoding 的类型 | M | H | domain/ 禁止引用 encoding/ 或 decoding/ 的任何内容 |
| R3 | 测试文件 import 路径过期 | H | M | Wave 4.4 专门修复 |
| R4 | encoder.rs 的测试与生产代码紧耦合 | M | M | 测试随 PgsEncoder 移到 encoding/encoder.rs |
| R5 | 子模块 mod.rs 重导出漏掉类型 | M | M | 每 Wave 完成后运行 cargo test |
