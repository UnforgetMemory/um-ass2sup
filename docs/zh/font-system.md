# 🔤 字体子系统

> FontRegistry、SimpleShaper、GlyphRasterizer：纯 Rust 字体管线

---

## 📋 目录

- [设计哲学](#设计哲学)
- [模块结构](#模块结构)
- [数据类型（types.rs）](#数据类型typesrs)
- [字体索引（index.rs）](#字体索引indexrs)
- [字体数据库（database.rs）](#字体数据库databasers)
- [字体发现（discovery.rs）](#字体发现discoveryrs)
- [FontRegistry（registry.rs）](#fontregistryregistryrs)
- [SimpleShaper（shaper.rs）](#simpleshapershaperrs)
- [GlyphRasterizer（rasterizer.rs）](#glyphrasterizerrasterizerrs)
- [8 级字体回退链](#8-级字体回退链)
- [日志与遥测（telemetry.rs）](#日志与遥测telemetryrs)
- [错误处理（error.rs）](#错误处理errorrs)
- [外部依赖](#外部依赖)
- [性能特点](#性能特点)

---

## 设计哲学

字体子系统是 native-backend 的核心，替代了 v2.7.x 时代的 fontdb + cosmic-text 方案。自建方案实现了完全的依赖控制，并针对蓝光字幕渲染的特定需求进行了优化。

**五大核心原则：**

1. **FontRegistry 只管"有没有"** — 加载、索引、查询、报告。回退策略完全由下游调用者传参控制。
2. **零内置配置** — 没有默认 fallback 链。没有隐藏的降级行为。调用者决定一切。
3. **索引即真相** — 高效的复合键索引，O(1) 精匹配，O(k) 族名模糊匹配。
4. **错误即信息** — 找不到时，错误携带所有可用替代项。上层自行决策。
5. **单域专职** — 每个子模块只做一件事，边界清晰，可独立测试和替换。

---

## 模块结构

```
crates/subtitle-renderer/src/font/
│
├── mod.rs               # pub 重导出
├── types.rs             # 纯数据类型 — 零逻辑
├── index.rs             # FontIndex — 高效多维度索引
├── database.rs          # FontDatabase — 字体存储 + 解析
├── discovery.rs         # FontDiscovery — 跨平台字体发现
├── registry.rs          # FontRegistry — 统一门面
├── shaper.rs            # SimpleShaper — 字形塑形
├── rasterizer.rs        # GlyphRasterizer — 字形光栅化
├── telemetry.rs         # FontTelemetry — 结构化事件
└── error.rs             # FontError — 领域错误
```

---

## 数据类型（types.rs）

纯数据类型定义，**零逻辑**：

| 类型 | 描述 |
|------|------|
| `FontId(u32)` | 不透明句柄 |
| `FontWeight` | 字重：Thin(100) … Black(900), Custom(u16) |
| `FontStyle` | Normal / Italic |
| `FontStretch` | Condensed / Normal / Expanded |
| `FontFace` | 字体完整描述（id, family, weight, style, path, is_system, cjk, corrupt） |
| `FontQuery` | 查询参数（family, weight, style）**不含**回退策略 |

> **注意**：`FontQuery` **不包含**回退策略信息。回退完全由调用者控制。

---

## 字体索引（index.rs）

高效的复合键索引，使用 fnv1a 哈希（ASCII 大小写归一化）：

```
exact_index:
  HashMap<(u64, FontWeight, FontStyle), Vec<FontId>>
  → O(1) 精匹配

family_index:
  HashMap<u64, Vec<FontId>>
  → O(k) 族内模糊查找
```

### 查询流程

```
query("Source Han Sans SC", Normal, Normal)
  │
  ├─ exact_index.lookup(hash, Normal(400), Normal)
  │   ├─ 找到 → return { found: Some(id), candidates: [] }
  │   └─ 未找到 → Step 2
  │
  └─ family_index.lookup(hash)
      ├─ 找到变体列表
      │  → return { found: None, candidates: [FontFace{...}] }
      └─ 未找到 → return { found: None, candidates: [] }
```

**FontRegistry 只返回：精确匹配与否 + 可选候选项。调用者自行决定是否降级。**

---

## 字体数据库（database.rs）

字体数据的存储及解析：

```rust
struct FontDatabase {
    entries: Vec<FontEntry>,  // 按 FontId 索引存储
}

struct FontEntry {
    id: FontId,
    data: Vec<u8>,           // 原始 TTF/OTF 数据
    face: FontFace,          // 解析出的元数据
}
```

### 核心方法

| 方法 | 描述 |
|------|------|
| `load_fonts_dir(dir, is_system)` | 扫描目录加载所有字体 |
| `load_font_data(bytes)` | 从内存加载字体 |
| `load_font_file(path)` | 从文件加载字体 |
| `get_font_data(id)` | 获取字体原始二进制 |

### 加载时校验

- **cmap 存在性**：确保字体含字符映射表
- **必需字形覆盖**：检查基本 ASCII 与常见字形
- **CJK 标志**：检测中/日/韩字体
- **损坏字体处理**：标记 `corrupt=true`，WARN 日志，不影响其他字体加载

---

## 字体发现（discovery.rs）

跨平台系统字体路径扫描：

| 平台 | 系统字体路径 |
|------|-------------|
| **Linux** | `/usr/share/fonts/`, `/usr/local/share/fonts/`, `~/.local/share/fonts/`, `~/.fonts/` |
| **macOS** | `/System/Library/Fonts/`, `/Library/Fonts/`, `~/Library/Fonts/` |
| **Windows** | `C:\Windows\Fonts\`, `%LOCALAPPDATA%\Microsoft\Windows\Fonts\` |

`discover_user_fonts(dirs)` — 接受用户指定的自定义字体目录。

---

## FontRegistry（registry.rs）

统一门面，整合所有子组件：

```rust
pub struct FontRegistry {
    system_db: FontDatabase,   // 系统字体
    user_db: FontDatabase,     // 用户字体
    index: FontIndex,          // 跨数据库统一索引
    glyph_cache: LruCache,     // 字形光栅化缓存
}
```

### 核心 API

| 方法 | 描述 |
|------|------|
| `query(&self, q: &FontQuery) -> QueryResult` | 查询（精确匹配 + 候选项） |
| `check(&self, q: &FontQuery) -> Availability` | 可用性检查 |
| `load_system_fonts() -> usize` | 加载系统字体 |
| `load_user_fonts_dir(dir) -> usize` | 加载用户字体目录 |
| `load_user_font_data(bytes) -> Result<FontId>` | 从字节加载用户字体 |
| `get_font_data(id) -> &[u8]` | 获取字体原始数据 |

---

## SimpleShaper（shaper.rs）

字形塑形——将文本字符串映射为字形序列：

```
输入: ("Hello", font_data, 48.0)
输出: Vec<ShapedGlyph>
        [{ glyph_id: 43, advance: 28.5 },
         { glyph_id: 56, advance: 22.3 }, ...]
```

实现基于 swash：

- `swash::FontRef::charmap()` 将字符映射到字形 ID
- 记录每个字形的 `advance`（前进宽度）
- 支持横排（horizontal）和竖排（vertical）塑形
- **无复杂 OpenType 布局**（GPOS/GSUB）—— 蓝光字幕不需要复杂的连字替换

---

## GlyphRasterizer（rasterizer.rs）

字形光栅化——从字形 ID 到 alpha 位图：

```
输入: (font_data, glyph_id, 48.0)
输出: RasterizedGlyph { width, height, offset_x, offset_y, alpha_bitmap }
```

实现基于 swash：

- `swash::Scaler` 设置字号和渲染参数
- `swash::image` 生成 alpha 位图
- 使用 `swash::CacheKey` 进行字形缓存查询
- 缓存命中避免重复光栅化

---

## 8 级字体回退链

当精确匹配失败时，调用者可以按以下链进行回退（FontRegistry 本身不参与回退策略）：

```
Level 1: 精确匹配           → FontRegistry.query() 返回 some(id)
Level 2: 后缀剥离匹配       → 如 "Source Han Sans SC Bold" → "Source Han Sans SC"
Level 3: 别名查找           → "sans-serif" → "DejaVu Sans"
Level 4: 硬编码 CJK 回退   → "Noto Sans CJK SC" → "WenQuanYi Micro Hei"
Level 5: 跨平台 CJK 扫描   → 扫描系统已安装的 CJK 字体
Level 6: 泛型回退          → 按字重匹配任意相近字体
Level 7: SansSerif 默认     → 系统默认 sans-serif
Level 8: 任意可用字体       → 取第一个可用字体
```

---

## 日志与遥测（telemetry.rs）

结构化字体事件，用于诊断和性能分析：

| 事件 | 负载 |
|------|------|
| `FontEvent::Loaded` | id, family, weight, path, corrupt, took_us |
| `FontEvent::Queried` | query, result, candidates_count, took_us |
| `FontEvent::Corrupted` | path, reason, recoverable |
| `FontEvent::GlyphCache` | hit, miss, size |

---

## 错误处理（error.rs）

领域错误携带上下文信息，调用者可据此决策：

| 错误 | 上下文 |
|------|--------|
| `FontError::NotFound` | query, candidates（所有可用替代） |
| `FontError::Corrupted` | path, reason |
| `FontError::NoSystemFonts` | — |
| `FontError::Io` | path, source |
| `FontError::Parse` | path, detail |

---

## 外部依赖

| 依赖 | 用途 |
|------|------|
| **swash** | TTF/OTF 解析 + 字形光栅化（唯一运行时 dep） |
| **tiny-skia** | 像素合成（与字体无直接关系） |
| **tracing** | 结构化日志 |
| **parking_lot** | 并发控制（Mutex 包装共享资源） |

**已移除：** fontdb v0.23（→ 自建 index + database）、cosmic-text v0.19（→ 自建 shaper + rasterizer）、ttf-parser（→ swash 替代）

---

## 性能特点

| 方面 | 描述 |
|------|------|
| **字形缓存** | LruCache 缓存光栅化结果，避免重复渲染 |
| **查询复杂度** | 精匹配 O(1)，族名模糊匹配 O(k) |
| **并发** | FontRegistry 包在 `Mutex` 中，通过 parking_lot 高效共享 |
| **无堆分配** | 字形循环和渲染热路径上禁止堆分配 |
| **跨平台一致性** | swash 无 hinting 保证各平台字形外观一致 |

---

<p align="center">
  <sub>← [色彩量化管线](color-quantizer.md) | [返回首页](index.md)</sub>
</p>
