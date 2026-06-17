# Sub-3: 字体引擎 (Font Engine)

**Sprint**: S2
**周期**: 1-2 周
**依赖**: Sub-2（Style 解析）, Sub-1
**阻塞**: Sub-4

## 目标

将字体系统迁移到 cosmic-text（保留其内部 fontdb），实现用户完全可配置的 CJK fallback 链，三平台等效行为。

## 范围

### In Scope
- 集成 `cosmic-text = "0.18"`（最新稳定版）
- 实现 `cosmic_text::Fallback` trait：`AssFallback`
- 用户完全可配置 CJK fallback（CLI `--cjk-fallback`，TOML `cjk_fallback.chain`）
- 跨平台字体发现：Win (DirectWrite) / macOS (CoreText) / Linux (fontconfig)
- swash 集成做零拷贝 CJK 验证（保留 v0.5.5 优化的预扫描）
- 渐进迁移：保留旧 fontdb 路径 1 个 Sprint

### Out of Scope
- GPU 渲染（属于 Sub-6）
- 字体子集化（属于 Sub-7）

## 架构决策

### 决策：使用 cosmic-text 而非自研栈
**理由**：
- 三平台原生字体发现（DirectWrite / CoreText / fontconfig）
- 内置 HarfRust shaping（生产质量，HarfBuzz v13+ 兼容）
- 自带 swash 光栅化（替代 tiny-skia 部分功能）
- 内置 `Fallback` trait 满足 ASS 样式选择需求

**风险与缓解**：
- cosmic-text Editor 抽象过于复杂 → 只用 `FontSystem` + `Buffer` 子集
- 性能回退（Bevy 报告）→ 单元测试验证 < 5ms/事件
- 编译时间增加 → CI 监控

### 字体解析模块
```rust
pub struct FontResolver {
    font_system: cosmic_text::FontSystem,
    fallback: AssFallback,
}

impl cosmic_text::Fallback for AssFallback {
    fn fallback_for(&self, ctx: &cosmic_text::FontFallback) -> Option<cosmic_text::FontKey> {
        // 1. Style 显式指定
        // 2. Style.font_map 配置
        // 3. --cjk-fallback 全局
        // 4. 错误：返回 None（CLI 报错）
    }
}
```

### CJK fallback 配置
```rust
pub struct CjkFallback {
    pub chain: Vec<String>,  // ["Noto Sans CJK SC", "Microsoft YaHei"]
    pub per_style: HashMap<String, Vec<String>>,  // OP_1 → ["Source Han Sans"]
    pub strict: bool,         // true = 无匹配时报错而非渲染 tofu
}
```

## 任务清单

| # | 任务 | 验证 |
|---|------|------|
| 3.1 | 添加 `cosmic-text = "0.18"` 依赖 | `cargo build` |
| 3.2 | 实现 `AssFallback` trait | 单元测试每个分支 |
| 3.3 | `--cjk-fallback` CLI flag | CLI 测试 |
| 3.4 | TOML `cjk_fallback` 配置 | 单元测试 serde |
| 3.5 | 三平台字体发现验证 | CI 矩阵 |
| 3.6 | 视觉回归：CJK 不再 tofu | 3+ 真实 CJK ASS 文件 |
| 3.7 | 移除旧 fontdb 路径 | `cargo tree` 验证 |

## 验证门 (Definition of Done)

- [ ] CJK 字符 100% 正确渲染（无 tofu）
- [ ] `--cjk-fallback` 未指定且遇 CJK 时清晰报错
- [ ] 三平台输出像素级接近（≤ 5px 差异）
- [ ] 现有 440+ 测试通过
- [ ] cosmic-text 依赖收口（不再使用 fontdb / rustybuzz / ttf-parser）
- [ ] 性能：单事件渲染 ≤ 5ms 退化
