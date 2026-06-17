# Sub-5: 色彩管线 (Color Pipeline)

**Sprint**: S4
**周期**: 1-2 周
**依赖**: Sub-4, Sub-1
**阻塞**: Sub-7（输出格式扩展）

## 目标

建立完整的色彩管线：SDR BT.709（当前完善）、HDR BT.2020 + PQ、HDR HLG 支持；自动检测源色彩空间并转换到目标。

## 范围

### In Scope
- SDR BT.709（已有，完善）
- HDR BT.2020 + PQ (SMPTE ST 2084)
- HLG (ARIB STD-B67)
- 色彩空间自动检测（从源 ASS 解析 + 输出目标）
- SDR↔HDR tonemapping
- BT.601 / BT.709 / BT.2020 矩阵
- PGS HDR bitstream（如果规格支持）

### Out of Scope
- Display P3
- Dolby Vision（v2.1+）

## 架构决策

### 色彩空间枚举
```rust
pub enum ColorSpace {
    SdrBt709,           // 现有
    HdrBt2020Pq,        // HDR10
    HdrBt2020Hlg,       // HLG
}

pub struct ColorPipeline {
    input_space: ColorSpace,
    output_space: ColorSpace,
    transfer: TransferFunction,  // Linear / sRGB / PQ / HLG
}
```

### Tonemapping
- 使用 Hable / ACES / Reinhard（可配置）
- 保留 highlight 不爆色
- 与 libass 视觉对齐

## 任务清单

| # | 任务 | 验证 |
|---|------|------|
| 5.1 | `ColorSpace` 枚举 + 矩阵 | unit test |
| 5.2 | `TransferFunction` (Linear/sRGB/PQ/HLG) | unit test |
| 5.3 | 自动检测源色彩空间 | unit test |
| 5.4 | Tonemapping (Hable) | 视觉对比 |
| 5.5 | PGS HDR bitstream | 二进制 spec 对比 |
| 5.6 | 配置文件 `color.output_space` | 单元测试 |
| 5.7 | CLI `--output-color-space` | CLI 测试 |

## 验证门 (Definition of Done)

- [ ] SDR 路径保持像素级一致（与 v0.5.5）
- [ ] HDR BT.2020 + PQ roundtrip 测试
- [ ] HLG roundtrip 测试
- [ ] Tonemapping 视觉可接受
- [ ] 配置文件 + CLI 集成
- [ ] 现有 440+ 测试通过
