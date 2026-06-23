# 2.2 剩余任务 Plan Spec

> 接续当前进度。Wave 1+2+3 已完成，剩 Wave 4（测试）+ 2.3（PGS 编码器补完）。

## 当前完成状态

```
Wave 1: ✅ 基础架构 (dep swap + RenderContext + EventError)
Wave 2: ✅ 10 tag 处理器 + orchestrator
Wave 3: ✅ render_ass_core 管线 (catch_unwind 隔离、53-tag 分派)
Wave 4: ⏳ 测试迁移 + 53 标签覆盖测试
2.3:    ⏳ PGS 编码器补完 (EpochContinue/PDS/ODS/palette_update)
```

## Wave 4: 测试迁移 + 53 标签覆盖

### Task 18: 测试套件迁移到 ass_core

**文件**: `crates/subtitle-renderer/tests/test_context.rs`, `test_renderer.rs` + benches

**工作量**: 8 个测试文件，需要从 ass_parser 类型改为 ass_core 类型

**关键 API 映射**:

| 旧 (ass_parser) | 新 (ass_core) |
|---|---|
| `AssFile::new()` | `SubtitleDocument::default()` |
| `Event { start: Timestamp, end: Timestamp, ... }` | `Event { start_ms, end_ms, text_raw, ... }` |
| `event.start.as_ms()` | `event.start_ms` |
| `event.end.as_ms()` | `event.end_ms` |
| `event.is_visible_at(ts)` | `start_ms <= ts && ts < end_ms` |
| `event.style_name` | `event.style.as_str()` |
| `event.text` | `event.text_raw` |
| `effect::parse_effect()` | `ass_core::effect::parse_effect()` |
| `AssColor::from_ass_hex()` | `ass_core::AssColor::from_ass_hex()` |
| `ass_parser::parse_override_tag()` | `ass_core::override_tag::parse_one_tag()` |

### Task 19: 53 标签覆盖测试

**文件**: `crates/subtitle-renderer/src/renderer/context/tests.rs`

**模式**: 为每个 OverrideTag 变体写测试，验证 build_context 输出正确 RenderContext

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ass_core::*;

    struct BuildContextTest {
        tags: Vec<OverrideTag>,
        check: fn(&RenderContext),
    }

    #[test]
    fn test_tag_pos() {
        let ctx = test_build_context(vec![
            OverrideTag::Pos { x: 100.0, y: 200.0 },
        ]);
        assert!(ctx.has_pos);
        assert!((ctx.x - 100.0).abs() < 0.001);
        assert!((ctx.y - 200.0).abs() < 0.001);
    }
    // ... 52 more tests
}
```

| 处理器 | 标签数 | 测试数 |
|--------|--------|--------|
| position | 3 | 6 |
| font | 8 | 12 |
| color | 9 | 12 |
| border | 6 | 8 |
| geometry | 7 | 10 |
| clip | 6 | 8 |
| karaoke | 1 | 3 |
| reset | 2 | 4 |
| transform | 1 | 3 |
| misc | 9 | 12 |
| **合计** | **53** | **78** |

---

## 2.3: PGS 编码器补完

> 修复从 2.1 审计中发现的 7 个 PGS 规范合规问题。

### 已知问题清单

| # | 问题 | 文件 | 优先级 |
|---|------|------|--------|
| 1 | `CompositionState::EpochContinue(0xC0)` 枚举缺失 | `types.rs:39-46` | 🔴 高 (PotPlayer 崩溃根因) |
| 2 | `palette_update` 始终 true | `encoder.rs` | 🟡 中 (应基于 palette_hash 变化) |
| 3 | ODS flags 字段值 0xC0 vs 引用的 0x80 | `encoder.rs` | 🟡 中 (需与参考 SUP 对比) |
| 4 | ODS total_size 3字节 LE 格式 | `types.rs:351-364` | 🟡 中 |
| 5 | Epoch 管理：每帧 EpochStart | `encoder.rs:build_display_set` | 🟡 中 (应为首帧 EpochStart→后续 NormalCase) |
| 6 | PDS YCbCr 字节序 (Y,Cr,Cb vs Y,Cb,Cr) | `encoder.rs` | 🟢 低 (部分播放器敏感) |
| 7 | 多对象 ODS 序列兼容性 | `encoder.rs` | 🟢 低 |

### Task 20: 修复 CompositionState::EpochContinue

```rust
// types.rs
pub enum CompositionState {
    NormalCase = 0x00,
    AcquirePoint = 0x40,
    EpochStart = 0x80,
    EpochContinue = 0xC0,  // ← 新增
}
```

**影响**: `build_display_set` 需要根据上下文选择正确的 CompositionState：
- 首帧 → `EpochStart`
- 后续帧（内容变化）→ `NormalCase` 或 `AcquirePoint`
- 无变化 → `EpochContinue` (只发 PCS+END)

### Task 21: palette_update 基于 hash 检测

```rust
// encoder.rs
fn hash_palette(entries: &[PaletteEntry]) -> u64 {
    // Simple hash of all palette entries
}
// encode_frame 时比较前后 palette_hash
// 只有变化时才设 palette_update = true
```

### Task 22: ODS 格式对齐

对比参考 SUP 文件。检查：
- ODS flags 字节（0x80 vs 0xC0）
- ODS total_size 字段（3字节 LE）

### Task 23: Epoch 管理重构

```rust
enum DisplaySetKind {
    EpochStart,     // 新epoch，完整
    NormalCase,     // 内容变化
    AcquisitionPoint, // 可同步
    PaletteOnly,    // 仅调色板
    EpochContinue,  // 无变化
}
```

**实现**:
1. `encode_frame` 跟踪 `last_frame_hash: u64`
2. 如果 bitmap 无变化 → `EpochContinue` (只发 PCS+END)
3. 如果仅 palette 变化 → `PaletteOnly` (只发 PDS)
4. 如果内容变化 → 根据策略选 `NormalCase` 或 `AcquirePoint`

---

## 执行顺序

```
Day 2 (明天):
├── Task 18: 测试迁移 (~2h)
├── Task 19: 53 标签测试 (~3h)
└── cargo test -p subtitle-renderer ✅

Day 3 (后天):
├── Task 20: EpochContinue 修复 (~1h)
├── Task 21: palette_update hash (~1h)  
├── Task 22: ODS 格式对齐 (~2h) — 需参考 SUP
└── Task 23: Epoch 管理重构 (~2h)

Day 4:
├── 回归测试 + 全链路检查
├── 与旧 ass-parser 管线对比输出
├── CHANGELOG + commit
└── 等待审查
```

## 退出标准

```
□ cargo clippy -D warnings              → 零警告
□ cargo test -p subtitle-renderer       → 所有测试通过
□ cargo test --workspace                → 全工作区测试通过
□ 53 OverrideTag 覆盖测试               → 78+ 测试全部通过
□ EpochContinue 枚举存在                 → CompositionState::EpochContinue
□ palette_update 不等于 always-true      → 基于 hash 检测
□ 参考 SUP 对比通过                      → bytes 匹配参考实现
```
