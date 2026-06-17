# ass2sup v2.0 实施路线图（Master TODO List）

> **截止**: v0.5.5 (2026-06-17)
> **目标**: v2.0.0 (预计 2026-11-17, 5 个月)

## Sprint 总览

| Sprint | Sub-Project | 周期 | 周累计 | 状态 | 关键交付 |
|--------|-------------|------|--------|------|----------|
| **S0** | Sub-1 基础设施 | 1-2 周 | 2 | 🔄 启动中 | 统一 Error + Config + telemetry |
| **S1** | Sub-2 ASS 解析器 | 2-3 周 | 5 | ⏳ | V4+ 22 字段 + libass 兼容 |
| **S2** | Sub-3 字体引擎 | 1-2 周 | 7 | ⏳ | CJK 正确 + cosmic-text 迁移 |
| **S3** | Sub-4 渲染器 | 2-3 周 | 10 | ⏳ | 像素精度 + 特效完整 |
| **S4** | Sub-5 色彩管线 | 1-2 周 | 12 | ⏳ | HDR BT.2020+PQ + HLG |
| **S5** | Sub-6 GPU | 2-3 周 | 15 | ⏳ | vello 10x 加速 |
| **S6** | Sub-7 输出格式 | 1-2 周 | 17 | ⏳ | TTML + WebVTT + 插件 |
| **S7** | Sub-8 CLI | 1-2 周 | 19 | ⏳ | REPL + 智能错误 + 配置 |
| **持续** | Sub-9 测试 | 全程 | - | ⏳ | CI 矩阵 + 视觉回归 |

---

## Sprint 0 — Sub-1 基础设施（启动中）

**负责人**: TBD
**Branch**: `v2.0/sub-1-infrastructure`
**依赖**: 无

### 任务清单

| ID | 任务 | 状态 | 验证门 | 任务 MD |
|----|------|------|--------|---------|
| 1.1 | 创建 `ass2sup` umbrella crate | 🔄 | `cargo build` 通过 | [task-01-error-types.md](01-infrastructure/task-01-error-types.md) |
| 1.2 | `Error` 枚举（10 个变体） | 🔄 | unit test 覆盖 | [task-01-error-types.md](01-infrastructure/task-01-error-types.md) |
| 1.3 | `Config` 结构（30+ 字段） | 🔄 | serde round-trip | [task-02-config-system.md](01-infrastructure/task-02-config-system.md) |
| 1.4 | `telemetry::init()` | 🔄 | 重复 init 安全 | [task-03-telemetry.md](01-infrastructure/task-03-telemetry.md) |
| 1.5 | MSRV 1.88 升级 | ⏳ | 编译 + clippy | （内嵌 task-01） |
| 1.6 | 现有 440+ 测试通过 | ⏳ | 0 失败 | 全部 task |

### Sprint 0 验证门

- [ ] `Error` 完整 + 文档化
- [ ] `Config` schema 完整 + TOML 验证
- [ ] telemetry 统一入口
- [ ] 现有测试 0 失败
- [ ] `cargo clippy -D warnings` 零警告
- [ ] CI 三平台通过

---

## Sprint 1 — Sub-2 ASS 解析器

**依赖**: Sub-1 完成
**Branch**: `v2.0/sub-2-ass-parser`

### 任务清单

| ID | 任务 | 状态 | 验证门 | 任务 MD |
|----|------|------|--------|---------|
| 2.1 | V4+ Styles 22 字段强类型 | ⏳ | round-trip test | `02-ass-parser/task-01-v4-styles.md` |
| 2.2 | Events 10 字段 + Style 引用 | ⏳ | golden test 50+ | `02-ass-parser/task-02-events.md` |
| 2.3 | Override tag 解析器 | ⏳ | 50+ unit test | `02-ass-parser/task-03-override-tags.md` |
| 2.4 | `\t(\tag, t1, t2, accel)` 动画 | ⏳ | 视觉对比 libass | `02-ass-parser/task-04-animations.md` |
| 2.5 | 错误恢复 | ⏳ | 故意损坏测试 | `02-ass-parser/task-05-error-recovery.md` |
| 2.6 | SRT → ASS 升级 | ⏳ | round-trip | `02-ass-parser/task-06-srt-upgrade.md` |
| 2.7 | libass 兼容测试套件 | ⏳ | 100+ fixtures | `02-ass-parser/task-07-libass-compat.md` |

### Sprint 1 验证门

- [ ] V4+ 22 字段全部解析 + 序列化
- [ ] Override tag 100% 覆盖
- [ ] 错误恢复产出可用 AST + warnings
- [ ] libass 兼容 ≥ 95% pass
- [ ] 现有 parser 测试 0 失败

---

## Sprint 2 — Sub-3 字体引擎

**依赖**: Sub-2 完成
**Branch**: `v2.0/sub-3-font-engine`

### 任务清单

| ID | 任务 | 状态 | 验证门 | 任务 MD |
|----|------|------|--------|---------|
| 3.1 | 添加 `cosmic-text` 依赖 | ⏳ | `cargo build` | `03-font-engine/task-01-integration.md` |
| 3.2 | `AssFallback` trait | ⏳ | unit test 各分支 | `03-font-engine/task-02-fallback-trait.md` |
| 3.3 | `--cjk-fallback` CLI | ⏳ | CLI test | `03-font-engine/task-03-cjk-fallback-config.md` |
| 3.4 | TOML `cjk_fallback` | ⏳ | serde test | `03-font-engine/task-03-cjk-fallback-config.md` |
| 3.5 | 三平台字体发现 | ⏳ | CI 矩阵 | `03-font-engine/task-04-platform-discovery.md` |
| 3.6 | 视觉回归: CJK 无 tofu | ⏳ | 3+ 真实 ASS | `03-font-engine/task-05-visual-regression.md` |
| 3.7 | 移除旧 fontdb 路径 | ⏳ | `cargo tree` | `03-font-engine/task-06-remove-old-stack.md` |

### Sprint 2 验证门

- [ ] CJK 字符 100% 正确渲染
- [ ] 未指定 CJK fallback 时清晰报错
- [ ] 三平台像素级接近
- [ ] 现有 440+ 测试通过
- [ ] 性能: 单事件 ≤ 5ms 退化

---

## Sprint 3 — Sub-4 渲染器

**依赖**: Sub-2, Sub-3
**Branch**: `v2.0/sub-4-renderer`

### 任务清单

| ID | 任务 | 状态 | 验证门 |
|----|------|------|--------|
| 4.1 | Buffer 编排 | ⏳ | unit test |
| 4.2 | Spans 解析 | ⏳ | unit test |
| 4.3 | `\fad`, `\fade` | ⏳ | 视觉对比 libass |
| 4.4 | `\pos`, `\move` | ⏳ | 视觉对比 |
| 4.5 | `\clip`, `\iclip` | ⏳ | 视觉对比 |
| 4.6 | `\frz`, `\frx`, `\fry` | ⏳ | 视觉对比 |
| 4.7 | `\blur` | ⏳ | 视觉对比 |
| 4.8 | `\t(\tag, ...)` 动画 | ⏳ | 视觉对比 |
| 4.9 | Karaoke | ⏳ | 视觉对比 |
| 4.10 | Drawing mode `\p` | ⏳ | 视觉对比 |
| 4.11 | Layer ordering | ⏳ | 视觉对比 |
| 4.12 | 像素精度 | ⏳ | libass ≥ 90% |

### Sprint 3 验证门

- [ ] 11 个特效全部实现
- [ ] Karaoke 4 模式
- [ ] libass 像素匹配 ≥ 90%
- [ ] 单事件 ≤ 50ms (debug) / ≤ 10ms (release)

---

## Sprint 4 — Sub-5 色彩管线

**依赖**: Sub-4
**Branch**: `v2.0/sub-5-color-pipeline`

### 任务清单

| ID | 任务 | 状态 | 验证门 |
|----|------|------|--------|
| 5.1 | ColorSpace 枚举 + 矩阵 | ⏳ | unit test |
| 5.2 | TransferFunction (Linear/sRGB/PQ/HLG) | ⏳ | unit test |
| 5.3 | 自动检测源色彩空间 | ⏳ | unit test |
| 5.4 | Tonemapping (Hable) | ⏳ | 视觉对比 |
| 5.5 | PGS HDR bitstream | ⏳ | 二进制 spec |
| 5.6 | 配置文件 + CLI | ⏳ | unit test |

### Sprint 4 验证门

- [ ] SDR 路径与 v0.5.5 像素一致
- [ ] HDR BT.2020+PQ roundtrip
- [ ] HLG roundtrip
- [ ] Tonemapping 视觉可接受

---

## Sprint 5 — Sub-6 GPU

**依赖**: Sub-4
**Branch**: `v2.0/sub-6-gpu`

### 任务清单

| ID | 任务 | 状态 | 验证门 |
|----|------|------|--------|
| 6.1 | `RendererBackend` trait | ⏳ | unit test |
| 6.2 | vello PoC | ⏳ | 输出有效 PGS |
| 6.3 | vello 集成 | ⏳ | 视觉对比 CPU |
| 6.4 | 混合 pipeline | ⏳ | benchmark |
| 6.5 | Criterion 基准 | ⏳ | ≥ 10x |
| 6.6 | GPU 故障 fallback | ⏳ | unit test |

### Sprint 5 验证门

- [ ] vello 集成 + 视觉一致
- [ ] ≥ 10x 加速（> 5000 事件）
- [ ] GPU 失败 fallback CPU

---

## Sprint 6 — Sub-7 输出格式

**依赖**: Sub-5
**Branch**: `v2.0/sub-7-output-formats`

### 任务清单

| ID | 任务 | 状态 | 验证门 |
|----|------|------|--------|
| 7.1 | `OutputSink` trait | ⏳ | unit test |
| 7.2 | TTML 序列化 | ⏳ | spec 合规 |
| 7.3 | WebVTT 序列化 | ⏳ | spec 合规 |
| 7.4 | ASS 透传 | ⏳ | unit test |
| 7.5 | 插件加载 | ⏳ | unit test |
| 7.6 | CLI `--format` | ⏳ | CLI test |
| 7.7 | 跨格式一致性 | ⏳ | 视觉 |

### Sprint 6 验证门

- [ ] 5 个 sink 全部实现
- [ ] TTML / WebVTT spec 合规
- [ ] 插件 API 稳定

---

## Sprint 7 — Sub-8 CLI

**依赖**: Sub-1 ~ Sub-7
**Branch**: `v2.0/sub-8-cli`

### 任务清单

| ID | 任务 | 状态 | 验证门 |
|----|------|------|--------|
| 8.1 | REPL 模式 | ⏳ | unit test |
| 8.2 | 智能错误 + 建议 | ⏳ | unit test |
| 8.3 | JSON 进度输出 | ⏳ | JSON schema |
| 8.4 | Shell completions | ⏳ | 手动 |
| 8.5 | 配置文件 + flag | ⏳ | unit test |
| 8.6 | clap_mangen 文档 | ⏳ | 手动 |

### Sprint 7 验证门

- [ ] REPL 工作
- [ ] 错误可操作
- [ ] JSON 输出可 jq 解析
- [ ] Shell completions 三种

---

## Sprint 持续 — Sub-9 测试

**横向**: 全程

### 任务清单

| ID | 任务 | 状态 | 验证门 |
|----|------|------|--------|
| 9.1 | insta 图像快照 | ⏳ | 快照生成 |
| 9.2 | CI 矩阵 | ⏳ | 3 OS |
| 9.3 | proptest 策略 | ⏳ | unit test |
| 9.4 | Golden baseline | ⏳ | v0.5.5 → v2.0 |
| 9.5 | Criterion | ⏳ | 报告 |
| 9.6 | Coverage | ⏳ | 报告 |
| 9.7 | 模糊测试 | ⏳ | 无 crash |

### Sprint 9 验证门

- [ ] CI 三平台全过
- [ ] Coverage ≥ 80%
- [ ] 视觉回归覆盖全 Sub
- [ ] 性能基准可复现

---

## 风险与回退

每 Sprint 启动时回顾：
- 时间盒（sprint 周期上限）是否可超
- 依赖是否到位
- 风险是否升级

如某 Sprint 失败：
- 旧 v0.5.5 仍然可用（branch 保留）
- 该 Sprint 任务推迟到 v2.0.1 patch
- 不阻塞其他 Sprint 并行推进

---

## 进度跟踪

每周末更新此文件，标记 ✅ 完成 / 🔄 进行中 / ⏳ 待办。
