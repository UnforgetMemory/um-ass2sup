# Sub-7: 输出格式扩展 (Output Formats)

**Sprint**: S6
**周期**: 1-2 周
**依赖**: Sub-5, Sub-1
**阻塞**: 无

## 目标

在保持 PGS / BDN 稳定的基础上，扩展输出格式：TTML（流媒体 HDR/SDR）、WebVTT（HTML5）、ASS 透传（保真）、自定义插件。

## 范围

### In Scope
- PGS / SUP（保持）
- BDN XML + PNG（保持）
- ASS / SSA 透传（重渲染时保留样式）
- TTML (SMPTE-TT) HDR/SDR
- WebVTT
- 插件 `OutputSink` trait

### Out of Scope
- SRT 输出（v1.1）
- STL（v1.1）

## 架构决策

### OutputSink trait
```rust
pub trait OutputSink: Send + Sync {
    fn write_frame(&mut self, frame: &RenderedFrame) -> Result<()>;
    fn finalize(&mut self) -> Result<()>;
}

pub struct PgsSink { ... }
pub struct BdnXmlSink { ... }
pub struct TtmlSink { ... }
pub struct WebVttSink { ... }
pub struct AssPassthroughSink { ... }
```

### CLI 集成
- `--format pgs|bdn|ttml|webvtt|ass`
- `--output-plugin <PATH>` 加载 .so/.dll/.dylib

## 任务清单

| # | 任务 | 验证 |
|---|------|------|
| 7.1 | `OutputSink` trait 设计 | unit test |
| 7.2 | TTML 序列化（W3C TTML2 spec） | conformance test |
| 7.3 | WebVTT 序列化（W3C WebVTT spec） | conformance test |
| 7.4 | ASS 透传 | 单元测试 + 视觉 |
| 7.5 | 插件加载（libloading） | 单元测试 |
| 7.6 | CLI `--format` flag | CLI 测试 |
| 7.7 | 跨格式一致性测试 | 视觉 |

## 验证门 (Definition of Done)

- [ ] 5 个 sink 全部实现
- [ ] TTML / WebVTT spec 合规
- [ ] 插件 API 稳定
- [ ] 跨格式视觉一致
- [ ] 现有 440+ 测试通过
