# 重构 2.1 — ass-core: 全新 ASS/SRT 解析器

> **这不是重构。这是从零构建一个全新的 crate，替换旧的 `ass-parser`。**
>
> v0.1 的错误教训 + libass 参考实现 + 大胆重构——三者融合为 `ass-core`。
>
> 旧 crate（`ass-parser`）继续存在直到新 crate 验证完毕。
> 新分支 `v2.1/rewrite`，从零开始，质量优先。

## 0. 0.1 时代的血泪教训（直接决定架构取舍）

| 教训 | 根因 | 2.1 中如何避免 |
|------|------|---------------|
| `\b0` 在事件内失效 | event.rs 和 override_tag.rs 的重复代码版本不同步 | **单一解析路径**：不存在 event.rs 的 parse_single_tag，全部统一 |
| `\fsp`/`\blur` 等标签缺失 | 同上，event.rs 版本缺少 | **以 libass 标签表为完整清单**，缺失就是 bug |
| 事件 margin 默认0而不是回退到样式 | 用 `unwrap_or(0)` 替代了正确的样式回退逻辑 | **margin 回退到样式是 event 解析的责任**，用 `Option<u32>` 表示「未设置」，下游查样式 |
| 浮点 PTS 漂移 | `ms_to_90khz` 用 f64 除/ceil/乘/round | **纯整数：ms * 90 = PTS**，snap_to_frame 用有理数 ceil |
| 字体回退死锁 | fontconfig + 多线程 + TTF 解析 | 新 crate 不涉及字体——纯解析，渲染层处理 |
| 嵌入字体路径穿越 | ASS 文件可以包含 `../../etc/passwd` 路径 | 新 crate 只解析 fontname+filename，安全检查在 CLI 层 |
| 换行符解析错误 | `\N` 和 `\n` 的处理不一致 | 新 crate 保留原始文本（`text_raw: String`）+ 提供 `text_display: String`（`\N`→`\n` 转换） |
| Section 头尾空格处理 | `[Script Info] ` 尾部空格导致识别失败 | Lexer 层统一处理：`[...]` 后忽略空格和 `;` 注释 |
| PGS 编码器的 timecode 重复 | `pgs-encoder` 自己实现了 `timecode_to_ms` | 时间格式转换全部集中在 `time/` 模块，下游依赖此模块 |

## 1. 战略定位

```
    旧的                      新的
  ass-parser (v0.5)         ass-core (v2.1)
  ─────────────────         ─────────────────
  修补式改进                从零全新构建
  解析+部分渲染概念混合      纯数据模型，不涉及渲染
  嵌入路径安全检查          路径安全在 CLI 层
  时间浮点抖动              纯整数有理数时间
  50+ unwrap_or 吞数据      零 unwrap_or，所有路径兜底或报错
  无源位置跟踪              Span 全程跟踪
  重复代码（event.rs vs   单一解析路径
    override_tag.rs）
  输出给 10% 成功率的       输出给未来 100% 的新下游
  下游                      （下游 2.2 重建）
```

## 2. 新 crate: `ass-core`

```
crates/ass-core/          ← 新 crate，从零构建
├── Cargo.toml            ← 仅 thiserror（错误） + tracing（基础日志）
└── src/
    ├── lib.rs            ← 公开 API：SubtitleDocument 类型
    ├── lexer.rs          ← Token 流 + Section 识别
    ├── section.rs        ← Section 解析（ScriptInfo/Style/Event/Font）
    ├── override_tag.rs   ← 基于 libass 语义的标签解析（唯一路径）
    ├── karaoke.rs        ← KaraokeSegment 模型
    ├── effect.rs         ← Effect 模型（Banner/Scroll/Karaoke）
    ├── color.rs          ← AssColor（ABGR 格式，复用现有）
    ├── span.rs           ← Span 源位置跟踪
    ├── error.rs          ← ParseError + Warning（带 Span）
    ├── srt.rs            ← SRT 解析（转换到 SubtitleDocument）
    ├── time/
    │   ├── mod.rs
    │   ├── timestamp.rs  ← from_ass_time, parse_srt_timecode
    │   ├── fps.rs        ← Fps{num,den} 有理数帧率
    │   └── convert.rs    ← ms_to_90khz, 5种时间格式转换
    └── validate/         ← 可选的解析时验证（v2.2 扩展）
```

**依赖策略**：
- 运行时：仅 `thiserror`（错误 derive）+ `tracing`（日志）
- 开发：`proptest`、`insta`、`criterion`
- 零外部解析库（手写，因为 ASS 格式简单，用好 type system）

## 3. 产品定义: `SubtitleDocument`

下游工厂拿到的是这个：

```rust
/// 一份字幕文档的全部信息。
/// 设计原则：完整保留原始信息，不丢失，不猜测，不改写。
pub struct SubtitleDocument {
    /// 原始格式（ASS/SSA/SRT）
    pub format: SubtitleFormat,
    /// 脚本元数据。
    /// 所有未知字段保留在 extra 中（不丢信息）。
    pub metadata: ScriptMetadata,
    /// 样式列表（ASS/SSA 有，SRT 为单个默认样式）
    pub styles: Vec<Style>,
    /// 事件列表（对话、评论等）
    pub events: Vec<Event>,
    /// 嵌入字体（ASS [Fonts] 节）
    pub fonts: Vec<EmbeddedFont>,
    /// 解析过程中产生的警告（带源位置）
    pub warnings: Vec<Warning>,
}

pub struct Event {
    /// 原始行号（用于错误报告和调试）
    pub source_line: u32,
    /// 事件类型
    pub event_type: EventType,
    /// 图层号
    pub layer: u32,
    /// 开始时间（毫秒，精确整数）
    pub start_ms: u64,
    /// 结束时间（毫秒，精确整数）
    pub end_ms: u64,
    /// 样式名（对应 styles 中的名称）
    pub style: StyleRef,
    /// 演员/说话人名称
    pub actor: String,
    /// 边距覆盖（None = 使用样式的默认值）
    pub margin_l: Option<u32>,
    pub margin_r: Option<u32>,
    pub margin_v: Option<u32>,
    /// 效果
    pub effect: Effect,
    /// 原始文本（保留所有 {}、\N、\n 等原始字符，不修改）
    pub text_raw: String,
    /// 提取的 override 标签（带源位置信息）
    pub override_tags: Vec<(OverrideTag, Span)>,
    /// 卡拉 OK 分段
    pub karaoke_segments: Vec<KaraokeSegment>,
}
```

**与旧 `AssFile::Event` 的关键差异**：

| 旧字段 | 旧问题 | 新字段 | 新设计 |
|--------|--------|--------|--------|
| `margin_l/r/v: u32` | `unwrap_or(0)` 吞掉「未设置」语义 | `margin_l/r/v: Option<u32>` | `None` = 回退到样式的边距 |
| `text: String` | `\N`→`\n` 转换丢失了原始形态 | `text_raw: String` | 完全保留原始文本，不做任何修改 |
| `override_tags: Vec<OverrideTag>` | 无位置信息 | `Vec<(OverrideTag, Span)>` | 每个标签有源位置 |
| `raw_override_block: String` | 罕见使用 | 删除，`text_raw` 已包含 | 消除冗余 |

## 4. 解析管线（流水线）

```
  .ass / .ssa / .srt
         │
         ▼
   ┌──────────┐
   │  Lexer   │  ← 工匠1：行级 Token 化，不涉及语义
   │          │     处理：BOM、\r\n/\n、注释、Section 头尾
   └────┬─────┘
        │ Token 流
        ▼
   ┌──────────┐
   │  Router  │  ← 工匠2：根据 Token 分派到各 Section 处理器
   │          │     根据 RecoveryMode 控制错误传播策略
   └────┬─────┘
        │
   ┌────┴────┬──────┬──────┬──────┐
   ▼         ▼      ▼      ▼      ▼
ScriptInfo  Style  Event  Font  Unknown
                                           ← 每个处理器产生结构化字段
   │         │      │      │
   │         │   ┌──┴──┐                ← Event 的 text 进入
   │         │   │     │
   │         │   ▼     │
   │         │  ┌──────────┐
   │         │  │ Override │  ← 工匠3：{...} 提取 + \ 分割 + libass 语义匹配
   │         │  │ Tagger   │     每个标签：(OverrideTag, Span)
   │         │  └────┬─────┘
   │         │       │
   │         │       ▼
   │         │  ┌──────────┐
   │         │  │ Karaoke  │  ← karaoke 分段提取（与 tag 解析分离）
   │         │  │ Splitter │
   │         │  └────┬─────┘
   │         │       │
   ▼         ▼       ▼        ▼
   ┌───────────────────────────┐
   │     SubtitleDocument      │  ← 最终产品
   │  + warnings / errors      │
   └───────────────────────────┘
```

## 5. libass 语义等价清单（TAG_MATRIX.md 完整版）

**核心原则**：如果 libass 能解析，ass-core 必须能解析；如果 libass 不能解析，ass-core 可以选择拒绝或容错但必须报告。

| # | 标签 | libass 行为 | ass-core 行为 | 测试 |
|---|------|------------|-------------|------|
| 1 | `\K` | 等同 `\kf`（大写K） | 等同 `\kf` | `\K100`→duration=1000ms |
| 2 | `\b0`/`\b1`/`\b-1` | Bold(true/false) | Bold(bool) | `\b0`→false |
| 3 | `\b100-900` | BoldWeight | BoldWeight(u32) | `\b700`→weight=700 |
| 4 | `\i0`/`\i1`/`\i-1` | Italic(bool) | Italic(bool) | `\i0`→false |
| 5 | `\a1`-`\a11` | `((val&3)==0)?5:val` | 存原始+标志位 | `\a4`→mapped_to_5 |
| 6 | `\be(N)` | `int(N+0.5)` clamp | 存 `be_raw`+`be_rounded` | 下游决定用哪个 |
| 7 | `\fs+N`/`\fs-N` | 相对字号 | `FontSizeRelative(isize)` | `\fs+2` |
| 8 | `\clip` 缺 `)` | 容错到串尾 | 容错 | `\clip(10,20,30`→OK |
| 9 | `\p` 负值 | `max(0,N)` | clamp 后存 u8 | `\p-1`→0 |
| 10 | `\c` | 等同 `\1c` | 等同 `\1c` | `\c&HFF0000&`→红色 |
| 11 | `\fn@FontName` | 垂直标记 | 保留 `@` 在 family 中 | `\fn@MS Gothic` |
| 12 | `\move` t1>t2 | swap(t1,t2) | swap | `\move(0,0,100,100,500,100)` |
| 13 | `\an`/`\a` 互斥 | 先到先得 | `first_wins` 标志位 | `\an5\a1`→an5生效 |
| 14 | 缺 `)` 的标签 | 容错 | 容错 | `\pos(100,200`→Pos(100,200) |

**完整标签覆盖（60+）**：确保 `override_tag.rs` 的匹配链覆盖 libass `ass_parse.c` 的所有标签分支。

## 6. 验证策略

```
覆盖层级    验证方式                                运行频率
──────     ────────                                ────────
基础       cargo test -p ass-core （所有单元测试）     每次提交
           cargo clippy -D warnings
           cargo fmt --check

属性       cargo test -p ass-core proptest            每次提交
           （确定性、不 panic、Timestamp 往返）

语义       cargo insta test （libass fixtures 快照）   每次提交
           更新期望值用 cargo insta review

压力       4 fuzz targets 各 5min                     CI nightly
           cargo +nightly fuzz run ...

回归       cargo test --workspace                     合并前
           （等旧 ass-parser 测试仍通过）
```

**从第 1 行代码就开始写测试。** TDD 风格：先写测试定义，再写实现。

## 7. 分支策略

```bash
# 新分支，从零开始
git checkout -b v2.1/rewrite

# 分支生命周期：
# v2.1/rewrite → 开发分支（本计划的所有工作）
#   完成后 → 合并到 master（替换 ass-parser）
#   旧 ass-parser 保留直到合并前

# 工作流：每个 Task 一个子分支
# v2.1/rewrite → Task 完成后 squash merge
```

## 8. 执行波次（更新版）

### Wave 0: 基础设施 + 分支（~30min）

```bash
git checkout -b v2.1/rewrite
cargo init crates/ass-core --lib
# Cargo.toml: thiserror + tracing（运行时），proptest + insta + criterion（开发）
```

### Wave 1: 核心数据模型（5任务并行，无依赖）

| # | 任务 | 产出 | 验证 |
|---|------|------|------|
| 1.1 | `time/` 模块 + Fps + Timestamp + 5种时间格式 | 纯整数时间系统 | `ms_to_90khz=ms*90` proptest |
| 1.2 | `span.rs` + `error.rs` | Span 类型 + ParseError/Warning | clippy clean |
| 1.3 | `color.rs`（从旧 crate 迁移） | AssColor | 现有 10 个测试移植 |
| 1.4 | `lib.rs`: `SubtitleDocument` + `SubtitleFormat` | 产品类型定义 | 编译通过 |
| 1.5 | `effect.rs` + `karaoke.rs`（从旧 crate 迁移） | Effect/KaraokeSegment | 现有测试移植 |

### Wave 2: 核心解析器（依赖 Wave 1，3任务并行）

| # | 任务 | 产出 | 验证 |
|---|------|------|------|
| 2.1 | `lexer.rs` | Token 流 + Section 识别 | 122 fixtures token 计数正确 |
| 2.2 | `override_tag.rs` | libass 语义等价的标签解析器 | TAG_MATRIX.md 逐项验证 |
| 2.3 | `srt.rs` | SRT 解析器（用新的 time/ 和 document 类型） | SRT 往返测试 |

### Wave 3: Section 解析 + 组装（依赖 Wave 2）

| # | 任务 | 产出 | 验证 |
|---|------|------|------|
| 3.1 | `section.rs`: ScriptInfo/Style/Event/Font 解析器 | 结构化 section 数据 | 基本 ASS 文件解析 |
| 3.2 | `lib.rs`: `parse()` / `parse_lenient()` / `parse_with_recovery()` | 完整解析管线 | 122 libass fixtures 全部通过 |
| 3.3 | 消除所有 unwrap_or，替换为 warn+default 或 propagate 策略 | 零静默吞数据 | grep unwrap_or = 0 |

### Wave 4: 质量加固（依赖 Wave 3）

| # | 任务 | 产出 | 验证 |
|---|------|------|------|
| 4.1 | 添加 proptest（确定性、Timestamp 往返、不 panic） | 属性测试 | 256 例 x 10 策略通过 |
| 4.2 | 添加 fuzz targets | 4 targets | 各 5min 无崩溃 |
| 4.3 | 基准测试（与旧 ass-parser 对比解析速度） | Criterion 基准 | 性能退化 ≤5% |

## 9. 成功退出标准

```
□ ass-core 是全新 crate，不包含 ass-parser 的旧代码
□ 所有 60+ OverrideTag 变体正确解析（按 TAG_MATRIX.md）
□ libass 边缘情况全部覆盖
□ 零 unwrap_or(default) 在解析路径
□ 每个 ParseError/Warning 携带 Span
□ Option<u32> 替代 margin 的 unwrap_or(0)
□ text_raw 保留原始文本，text_display 提供 \N→\n 版本
□ time/ 模块：纯整数 ms_to_90khz + Fps 有理数 + 5 种时间格式
□ 122 libass fixtures 全部解析通过（快照建立新基线）
□ 4 fuzz targets 各 5min 无崩溃
□ proptest：确定性、Timestamp 往返、不 panic
□ margin_l/r/v 用 Option 表示「未设置」
□ 旧 ass-parser 的测试仍然全部通过（回归护栏）
```
