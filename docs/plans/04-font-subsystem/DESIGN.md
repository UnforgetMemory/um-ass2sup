# Font Subsystem v3.0 — Design Document (v2.8.0)

## 核心原则

1. **FontRegistry 只管"有没有"** — 加载、索引、查询、报告。回退策略完全由下游调用者传参控制。
2. **零内置配置** — 没有默认 fallback 链。没有隐藏的降级行为。调用者决定一切。
3. **索引即真相** — 高效的复合键索引，O(1) 精匹配，O(k) 族名模糊匹配。
4. **错误即信息** — 找不到时，错误携带所有可用替代项。上层自行决策。
5. **单域专职** — 每个子模块只做一件事，边界清晰，可独立测试和替换。

---

## DDD 子模块结构

```
crates/subtitle-renderer/src/font/       # 字体DDD子模块 (v2.8.0)
│
├── mod.rs               # pub 重导出
│
├── types.rs             # 纯数据类型 — 零逻辑
│   ├── FontId(u32)                     # opaque handle
│   ├── FontWeight                       # Thin(100) … Black(900), Custom(u16)
│   ├── FontStyle { Normal, Italic }
│   ├── FontStretch { Condensed, Normal, Expanded }
│   ├── FontFace { id, family, weight, style, path, is_system, cjk, corrupt }
│   └── FontQuery { family, weight, style }  # 查询参数（不包含回退策略！）
│
├── index.rs             # FontIndex — 高效多维度索引
│   ├── HashMap<(FamilyHash, Weight, Style), Vec<FontId>>  # O(1) 精匹配
│   ├── HashMap<FamilyHash, Vec<FontId>>                    # O(k) 族名模糊匹配
│   ├── family_hash(String) → u64    # fnv1a, 大小写归一化
│   └── insert / query_exact / query_family / list_families
│
├── database.rs          # FontDatabase — 字体存储 + 解析
│   ├── Vec<FontEntry>                    # 按 FontId (index) 存储原始数据
│   ├── load_fonts_dir(dir, is_system) → Vec<FontId>
│   ├── load_font_data(bytes) → Result<FontId>
│   ├── load_font_file(path) → Result<FontId>
│   ├── 解析内部: swash::FontRef 提取 metadata
│   ├── 完整性检查: cmap存在、必需字形覆盖、CJK标志
│   └── 损坏字体: 标记 corrupt=true，WARN日志，不影响其他字体
│
├── discovery.rs         # FontDiscovery — 跨平台字体发现
│   ├── discover_system_fonts() → Vec<PathBuf>
│   │   ├── Linux: /usr/share/fonts, ~/.local/share/fonts, ~/.fonts
│   │   ├── macOS: /System/Library/Fonts, /Library/Fonts, ~/Library/Fonts
│   │   └── Windows: C:\Windows\Fonts, %LOCALAPPDATA%\Microsoft\Windows\Fonts
│   └── discover_user_fonts(dirs: &[PathBuf]) → Vec<PathBuf>
│
├── registry.rs          # FontRegistry — 统一门面
│   ├── system_db: FontDatabase    # 系统字体（is_system=true）
│   ├── user_db: FontDatabase      # 用户字体（is_system=false）
│   ├── index: FontIndex           # 跨数据库统一索引
│   ├── glyph_cache: LruCache      # 字形光栅化结果缓存
│   ├── query(&self, q: &FontQuery) → QueryResult
│   │   └── QueryResult { found: Option<FontId>, candidates: Vec<FontFace> }
│   ├── check(&self, q: &FontQuery) → Availability
│   │   └── Availability { exact_match: bool, variants: Vec<FontFace>,
│   │         suggestion: Option<FontFace> }
│   ├── load_system_fonts() → usize
│   ├── load_user_fonts_dir(dir) → usize
│   └── load_user_font_data(bytes) → Result<FontId>
│
├── shaper.rs            # SimpleShaper — 字形塑形 (领域: 字符→字形映射)
│   ├── shape(text, registry, font_id, size) → Vec<ShapedGlyph>
│   └── 内部: swash::charmap + glyph_metrics
│
├── rasterizer.rs        # GlyphRasterizer — 字形光栅化 (领域: 字形→Alpha)
│   ├── rasterize(registry, glyph, size) → RasterizedGlyph
│   └── 内部: swash::Scaler + swash::image
│
├── telemetry.rs         # FontTelemetry — 结构化事件
│   ├── FontEvent::Loaded { id, family, weight, path, corrupt, took_us }
│   ├── FontEvent::Queried { query, result, candidates_count, took_us }
│   ├── FontEvent::Corrupted { path, reason, recoverable }
│   └── FontEvent::GlyphCache { hit, miss, size }
│
└── error.rs             # FontError — 领域错误（携带上下文）
    ├── NotFound { query: FontQuery, candidates: Vec<FontFace> }
    ├── Corrupted { path, reason }
    ├── NoSystemFonts
    ├── Io { path, source: io::Error }
    └── Parse { path, detail }
```

---

## 索引设计

```
FontIndex:

  family_hash → u64  (fnv1a, ascii_lowercase)
  
  exact_index: HashMap<(u64, FontWeight, FontStyle), Vec<FontId>>
  │  key = (family_hash("misansdemibold"), Normal(400), Normal)
  │  → [FontId(42), FontId(43)]   // 多个 face 匹配（如 collection）
  │  O(1) 查找到所有精确匹配
  │
  family_index: HashMap<u64, Vec<FontId>>
  │  key = family_hash("misansdemibold")
  │  → [FontId(42), FontId(43), FontId(44)]  // 该族所有变体
  │  O(k) 扫描族内变体用于模糊匹配
  │
  weight_sorted: 每个 family 内的变体按 weight 排序
  │  用于查找"最接近权重"的变体
```

### 查询流程

```rust
registry.query(&FontQuery { family: "MiSans Demibold", weight: Normal, style: Normal })
  │
  ├─ Step 1: exact_index.lookup(family_hash("misansdemibold"), Normal(400), Normal)
  │   ├─ 找到 → return QueryResult { found: Some(id), candidates: [] }
  │   └─ 未找到 → Step 2
  │
  ├─ Step 2: family_index.lookup(family_hash("misansdemibold"))
  │   ├─ 找到变体列表 → return QueryResult { found: None, candidates: [FontFace{weight:600}, ...] }
  │   └─ 未找到该族 → return QueryResult { found: None, candidates: [] }
  │
  （回退逻辑在上层调用者，FontRegistry 不参与）
```

**FontRegistry 只返回：精确匹配与否 + 可选候选项。调用者自行决定是否降级。**

---

## 错误设计 — 逐级上报，携带关键信息

```rust
/// 字体未找到 — 携带所有可用候选项，调用者可自行决策
#[derive(Debug)]
pub struct FontNotFound {
    pub query: FontQuery,
    pub candidates: Vec<FontFace>,     // 同族所有可用变体
    pub suggestion: Option<FontFace>,  // 权重最近匹配
}

/// 可用性检查结果 — 不是错误，是状态
pub struct Availability {
    pub exact_match: bool,
    pub variants: Vec<FontFace>,         // 所有可用变体
    pub suggestion: Option<FontFace>,    // 最佳替代
}

impl fmt::Display for FontNotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Font '{}' weight={} style={:?} not found. ", 
            self.query.family, self.query.weight, self.query.style)?;
        if !self.candidates.is_empty() {
            write!(f, "Available variants: ")?;
            for (i, c) in self.candidates.iter().enumerate() {
                if i > 0 { write!(f, ", ")?; }
                write!(f, "weight={}", c.weight)?;
            }
        }
        if let Some(s) = &self.suggestion {
            write!(f, ". Closest: '{}' weight={}", s.family, s.weight)?;
        }
        Ok(())
    }
}
```

---

## 调用者回退示例（ASS→SUP CLI）

```rust
// 回退策略完全由 CLI 层定义，FontRegistry 不知情
fn resolve_with_fallback(reg: &FontRegistry, event_font: &str, bold: bool, cjk: bool) -> FontId {
    let weight = if bold { FontWeight::Bold } else { FontWeight::Normal };
    let q = FontQuery { family: event_font.into(), weight, style: FontStyle::Normal };
    
    let result = reg.query(&q);
    if let Some(id) = result.found {
        return id;  // 精确匹配
    }
    
    // 回退策略 1: 权重容差 (由 CLI 控制)
    if let Some(sug) = &result.suggestion {
        warn!("Font {}: exact weight {} not found, using weight={} as fallback",
            event_font, weight, sug.weight);
        return sug.id;
    }
    
    // 回退策略 2: CJK 字体回退链 (由 CLI 配置)
    if cjk {
        for fb in &["Noto Sans CJK SC", "WenQuanYi Micro Hei", "Source Han Sans CN"] {
            let fb_result = reg.query(&FontQuery { family: (*fb).into(), weight, .. });
            if let Some(id) = fb_result.found {
                warn!("Font {}: not found, falling back to CJK default: {}", event_font, fb);
                return id;
            }
        }
    }
    
    // 回退策略 3: 系统默认
    reg.query(&FontQuery { family: "sans-serif".into(), weight, .. })
        .found
        .expect("No system fonts available")
}
```

---

## 实现阶段

| # | Phase | 产出 | 测试策略 |
|---|-------|------|---------|
| P1 | `types.rs` + `error.rs` | FontId, FontWeight, FontQuery, FontFace, FontNotFound, Availability | 单元测试 + Display |
| P2 | `index.rs` | FontIndex with HashMap composite keys | load→query 往返, 100 fonts 性能 |
| P3 | `discovery.rs` | 跨平台字体路径扫描 | Linux 环境验证 |
| P4 | `database.rs` | FontDatabase: load+parse+verify | 损坏字体、上百字体、TTF/OTF/TTC |
| P5 | `telemetry.rs` | 结构化 FONT_EVENT | 事件触发验证 |
| P6 | `shaper.rs` | SimpleShaper | 拉丁+CJK 字形映射 |
| P7 | `rasterizer.rs` | GlyphRasterizer | 字形大小验证 |
| P8 | `registry.rs` | FontRegistry 门面集成 | 全流程集成测试 |
| P9 | 管线接入 | 替换 renderer/cosmic.rs | 真实 ASS→SUP 端到端 |
| P10 | 依赖清理 | 移除 fontdb+cosmic_text | 编译 + 全工作区测试 |

---

## 外部依赖变更 (v2.8.0)

```toml
# 移除:
- fontdb = "0.23"          # → 自建 index.rs + database.rs
- cosmic-text = "0.19"     # → 自建 shaper.rs + rasterizer.rs
- ttf-parser                # → swash 替代 (P3 已移除)

# 新增 (仅 1 个运行时依赖):
+ swash = "0.2"             # TTF/OTF 解析 + 字形光栅化 (已是 workspace transitive)

# 保留:
  tiny-skia                 # 像素合成 (与字体无关)
  tracing                   # 结构化日志
  parking_lot               # 并发控制
```

---

*Created: 2025-06-25. Target version: 2.8.0.*
*Supersedes: the fontdb+cosmic_text era (v2.7.x and earlier).*
