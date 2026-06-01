# ass2sup Phase 8 — 详细规格与执行计划

> 基于代码库审计 (59 .rs 文件, 420+ 测试, 62 夹具) 和 ASS 标签覆盖分析
> 更新日期: 2025-06-01

---

## 一、审计发现的 Bug 清单 (必须修复)

### BUG-001: `\t` 解析器忽略 t2 参数
- **位置**: `crates/ass-parser/src/event.rs:297`
- **问题**: `let t2 = t1;` — t2 参数被覆盖为 t1 的值
- **影响**: `\t(tag,start,end,accel)` 中 end 时间失效，动画总是在 start 时刻完成
- **修复**: 正确解析 t2 参数（约 1 行改动）
- **优先级**: P0

### BUG-002: `\frx`/`\fry` 错误使用 origin 字段
- **位置**: `crates/subtitle-renderer/src/renderer.rs:200-203`
- **问题**: Rotation{x,y,z} 将 x,y 存入 origin_x/origin_y 而非独立旋转字段
- **影响**: `\frx` 和 `\fry` 无法正确实现 X/Y 轴旋转；与 `\org` 冲突
- **修复**: 需添加 rotation_x/rotation_y 字段到 RenderContext，修改 AffineTransform 合成逻辑
- **优先级**: P1
- **工作量**: 中等（约 50 行改动）

### BUG-003: BorderX/BorderY 与 ShadowX/ShadowY 被合并
- **位置**: `crates/subtitle-renderer/src/renderer.rs:188-193`
- **问题**: BorderX/BoardY 都设置 outline_width；ShadowX/ShadowY 都设置 shadow_depth
- **影响**: 无法实现 X/Y 方向不同的边框/阴影宽度
- **修复**: 在 rasterizer 中分别处理 X/Y 方向笔触（需修改 RenderContext 字段）
- **优先级**: P1
- **工作量**: 中等（约 30 行改动）

### BUG-004: `\q=3` 与 `\q=0` 行为相同
- **位置**: `crates/subtitle-renderer/src/renderer.rs:wrap_text()`
- **问题**: wrap_style 3 和 0 走相同分支（智能换行）
- **影响**: `\q=3`（从底部智能换行）未正确实现
- **修复**: 为 style 3 添加特殊处理（从底部开始换行）
- **优先级**: P2
- **工作量**: 小（约 15 行改动）

### BUG-005: `\t` 动画不支持 `\fax`/`\fay`
- **位置**: `crates/subtitle-renderer/src/renderer.rs:apply_transform_tag()`
- **问题**: shear 变换在 apply_transform_tag 中落入 `_ => {}`
- **影响**: `\t(\fax N)` 剪切动画不会插值
- **修复**: 在 apply_transform_tag 中添加 shear_x/y 插值分支
- **优先级**: P1
- **工作量**: 小（约 10 行改动）

### BUG-006: ODS object_version 始终为 0
- **位置**: `crates/pgs-encoder/src/encoder.rs` ODS 构建
- **问题**: object_version 未在对象数据变化时递增
- **影响**: 某些 Blu-ray 播放器可能无法正确处理同一对象 ID 的连续更新
- **修复**: 跟踪对象数据哈希，变化时递增 version
- **优先级**: P2
- **工作量**: 小（约 5 行改动）

---

## 二、Phase 8 执行计划

### Wave 1 (第 1-2 周): Bug 修复 + 代码审查基础

#### Task 1.1: 修复全部 6 个 Bug
**执行顺序** (按依赖关系):
```
BUG-001 (1行) → 写测试 → 修复
BUG-005 (10行) → 写测试 → 修复 (依赖 RenderContext)
BUG-002 (50行) → 写测试 → 修复 (需新增 rotation_x/y 字段)
BUG-003 (30行) → 写测试 → 修复 (需修改 rasterizer)
BUG-004 (15行) → 写测试 → 修复
BUG-006 (5行)  → 写测试 → 修复
```

**代码审查要点**:
- 每个修复必须有对应的回归测试
- 测试必须覆盖修复前的失败场景
- 不允许修改测试来匹配错误行为

**验证标准**:
- `cargo test` 全部通过
- 每个 bug 有至少 1 个针对性测试用例
- 无新增 clippy 警告

#### Task 1.2: 建立代码审查清单
创建 `CODE_REVIEW_CHECKLIST.md`:
- [ ] 新功能是否有文档注释 (`///`)
- [ ] 是否有单元测试覆盖正常/异常路径
- [ ] 是否处理了所有 `Option`/`Result`（无 `unwrap()` 在生产代码）
- [ ] 新增公开 API 是否有 `# Examples` 文档测试
- [ ] 常量是否有 `#[allow(dead_code)]` 误用检查
- [ ] 匹配是否穷举（无 `_ => {}` 隐藏逻辑）
- [ ] 错误类型是否使用 `thiserror` 派生
- [ ] 日志是否使用 `tracing` 而非 `println!`

#### Task 1.3: 创建 PGSDecoder 骨架
**目标**: 实现 SUP → 位图解码器以验证输出正确性

```rust
// crates/pgs-encoder/src/decoder.rs (新文件)
pub struct PgsDecoder;

impl PgsDecoder {
    /// 解码 SUP 字节流为帧序列
    pub fn decode(data: &[u8]) -> Result<Vec<DecodedFrame>>;
    
    /// 解码单个显示集
    fn decode_display_set(data: &[u8]) -> Result<DisplaySet>;
}

pub struct DecodedFrame {
    pub pts_ms: u64,
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

struct DisplaySet {
    pub palette: Vec<(u8, u8, u8, u8)>,
    pub objects: Vec<DecodedObject>,
}

struct DecodedObject {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub bitmap: Vec<u8>,  // 8-bit indexed, RLE-decoded
}
```

**依赖**: 复用现有 rle.rs 解码逻辑
**工作量**: 约 200 行代码 + 10 个测试

---

### Wave 2 (第 3-4 周): 性能优化

#### Task 2.1: 补充 Criterion 基准测试

```rust
// benches/renderer.rs (新增)
fn bench_single_frame(c: &mut Criterion);
fn bench_multi_event(c: &mut Criterion);      // 5 个叠加事件
fn bench_long_subtitle(c: &mut Criterion);    // 1000 事件文件
fn bench_karaoke(c: &mut Criterion);          // 卡拉OK 渲染
fn bench_effects_heavy(c: &mut Criterion);    // blur+shadow+rotation
```

**基线目标** (在 CI runner 上):
| 场景 | 目标 |
|------|------|
| 单帧无效果 | < 20ms |
| 单帧 5 事件 | < 50ms |
| 卡拉OK 帧 | < 80ms |
| 重度效果帧 | < 100ms |

#### Task 2.2: 字体缓存优化
**问题**: 当前每次渲染都通过 fontdb 查询字体，重复光栅化相同字形

**方案**: 在 FontManager 中缓存光栅化后的字形位图

```rust
// crates/subtitle-renderer/src/font.rs 扩展
struct GlyphCache {
    cache: HashMap<(FontId, u32, f32), CachedGlyph>,  // (font_id, glyph_id, size)
    max_entries: usize,
}

struct CachedGlyph {
    bitmap: Vec<u8>,
    width: u32,
    height: u32,
    offset_x: f32,
    offset_y: f32,
}
```

**预期提升**: 重复字形渲染减少 60-80%

#### Task 2.3: 渲染并行化
**方案**: 使用 rayon 并行渲染多个事件

```rust
// renderer.rs 中
pub fn render_ass_parallel(&self, ass: &AssFile, timestamp_ms: u64) -> Option<RenderedFrame> {
    let events: Vec<_> = ass.dialogue_events()
        .filter(|e| e.is_visible_at(Timestamp::from_ms(timestamp_ms)))
        .collect();
    
    let glyphs: Vec<_> = events.par_iter()
        .map(|e| self.render_event_to_glyphs(e, timestamp_ms))
        .collect();
    
    // Composite all glyphs onto final bitmap
    self.composite_glyphs(glyphs)
}
```

**注意事项**:
- FontManager 需实现 `Sync` (使用 RwLock)
- RenderContext 需按事件独立（已有）
- 仅在事件数 > 3 时启用并行（避免小文件开销）

---

### Wave 3 (第 5-6 周): 高级特性 + 错误恢复

#### Task 3.1: `\fe` 字符集支持
**目标**: 正确处理非 Unicode 编码的 ASS 文件

```rust
// ass-parser/src/lib.rs 扩展
impl AssFile {
    /// 根据 ScriptInfo 中的字符集转换文本编码
    /// \fe 编码映射: 1=GBK, 3=Shift-JIS, 5=Big5, 9=EUC-KR, 13=GB18030
    pub fn convert_encoding(&mut self) -> Result<(), EncodingError>;
}
```

**依赖**: `encoding-rs` crate

#### Task 3.2: parse_lenient 增强
**目标**: 从更多错误类型中恢复

```rust
// 新增可恢复错误:
// - 无效时间戳 → 使用 0:00:00.00
// - 未知覆盖标签 → 记录警告，继续解析
// - 字段数量不匹配 → 填充默认值
// - 无效颜色值 → 使用白色
```

#### Task 3: wrap_style 完整实现

```rust
// 完整的 \q 值处理:
// 0 = 智能换行 (已在 wrap_text 中实现)
// 1 = 行尾换行 (EOL word wrap)
// 2 = 不换行 (no word wrap)
// 3 = 底部智能换行 (smart wrap from bottom — 需要特殊定位处理)
```

---

### Wave 4 (第 7-8 周): 验证 + 分发

#### Task 4.1: SUP 解码验证管线
**目标**: 完整的 round-trip 测试

```
ASS 文件 
  → ass-parser 解析
  → subtitle-renderer 渲染为 RGBA
  → color-quantizer 量化
  → pgs-encoder 编码为 SUP
  → pgs-decoder 解码 SUP
  → 对比原始 RGBA vs 解码后 RGBA
  → 报告差异率
```

**通过标准**: 解码后位图与原始渲染位图逐像素一致

#### Task 4.2: AWS S3 权限问题归档
**当前问题**: 访问 S3 bucket `markosapi-assets-private-global` 返回 `AccessDenied`
- Bucket 策略显式拒绝了当前 IAM Role/Id
- 已生成的预签名 URL 均返回 AccessDenied
- **行动**: 隔离等待，需要 Bucket Owner 授权

#### Task 4.3: GitHub Actions CI 配置
**目标**: 自动化测试 + 发布

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace --release
      - run: cargo test --workspace -- --ignored  # E2E tests
      - run: cargo bench --workspace -- --save-baseline ci
  release:
    needs: test
    if: startsWith(github.ref, 'refs/tags/v')
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, x86_64-apple-darwin, 
                 x86_64-pc-windows-msvc, aarch64-unknown-linux-gnu]
    steps:
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: softprops/action-gh-release@v1
        with:
          files: target/${{ matrix.target }}/release/ass2sup*
```

---

## 三、代码审查规范

### CORRE 核心规则 (每次 PR 必检)

| 规则 | 检查命令/方法 | 阻断合并? |
|------|---------------|-----------|
| 编译无警告 | `cargo check --workspace` | ✅ |
| 测试全通过 | `cargo test --workspace` | ✅ |
| 无新 clippy 警告 | `cargo clippy --workspace -- -D warnings` | ✅ |
| 公开 API 有 rustdoc | `cargo doc --workspace 2>&1 \| grep "missing documentation"` | ✅ |
| 新增代码有测试 | 检查 diff 中 `#[test]` 数量 | ✅ |
| 无 `unwrap()` 在生产代码 | `grep -rn "unwrap()"` in src/ | ✅ |
| 无 `#[allow(dead_code)]` 滥用 | `grep -rn "allow(dead_code)"` | ⚠️ |
| Benchmark 无退化 | 对比 baseline | ⚠️ |
| E2E 测试通过 | `cargo test --workspace -- --ignored` | ✅ (release) |

### PR 模板

```markdown
## 变更描述
<!-- 描述此 PR 解决的问题或添加的功能 -->

## 关联 Issue
<!-- 链接到相关 Issue -->

## 测试
- [ ] 新增单元测试
- [ ] 新增集成测试
- [ ] 手动验证（附截图/输出）

## 性能影响
<!-- 如适用: 附 benchmark 对比 -->

## Breaking Changes
<!-- 是/否，如适用描述 -->
```

### 自动化检查脚本

创建 `ci/check.sh`:

```bash
#!/bin/bash
set -euo pipefail

echo "=== 1. cargo check ==="
cargo check --workspace 2>&1 | tail -5

echo "=== 2. cargo test (lib) ==="
cargo test --workspace --lib 2>&1 | tail -5

echo "=== 3. cargo test (all) ==="
cargo test --workspace 2>&1 | tail -5

echo "=== 4. cargo clippy ==="
cargo clippy --workspace -- -D warnings 2>&1 | tail -10

echo "=== 5. cargo doc ==="
cargo doc --workspace 2>&1 | grep -c "missing documentation" || true

echo "=== 6. 生产代码 unwrap 检查 ==="
UNWRAP_COUNT=$(grep -rn "\.unwrap()" crates/*/src/ | grep -v test | grep -v "// " | wc -l)
echo "unwrap count: $UNWRAP_COUNT"
if [ "$UNWRAP_COUNT" -gt 0 ]; then
    echo "FAIL: Found $UNWRAP_COUNT unwrap() in production code"
    grep -rn "\.unwrap()" crates/*/src/ | grep -v test
    exit 1
fi

echo "=== ALL CHECKS PASSED ==="
```

---

## 四、开发效率改进

### 本地开发循环

```bash
# 快速检查（30s）
alias qc='cargo check --workspace && cargo test --workspace --lib'

# 完整验证（2min）
alias qa='ci/check.sh'

# 性能检查（3min）
alias qperf='cargo bench --workspace -- --test'

# 单个 crate 测试（10s）
# cargo test -p ass-parser --lib
# cargo test -p subtitle-renderer --lib
```

### 测试夹具命名规范

```
tests/fixtures/
  alignment_all.ass          # 功能描述
  animation_sequence.ass      # 
  batch_mode.ass             # 
  blur_border.ass            #
  ...
  edge_*.ass                 # 边缘情况
  stress_*.ass               # 压力测试
  regression_*.ass           # 回归测试 (Bug 修复后添加)
```

### 回归测试创建流程

每个 bug 修复必须附带回归测试:

```rust
// crates/ass-parser/tests/test_regression.rs (新文件 或追加)

#[test]
fn bug001_t_tag_preserves_t2() {
    // BUG-001: \t 解析器忽略 t2 参数
    let data = r#"
[Script Info]
Title: Test

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, ShadowColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:10.00,Default,,0,0,0,,{\t(\fs40,100,5000,1)}Test
"#;
    
    let ass = AssFile::parse(data).expect("parse failed");
    let event = &ass.events[0];
    
    // 验证 t2=5000 被正确保留
    let t_tag = event.override_tags.iter()
        .find(|t| matches!(t, OverrideTag::Transform { .. }));
    
    match t_tag {
        Some(OverrideTag::Transform { t2, .. }) => {
            assert_eq!(*t2, 5000, "t2 should be 5000, not overwritten by t1");
        }
        _ => panic!("Expected Transform tag"),
    }
}

#[test]
fn bug005_transform_animates_shear() {
    // BUG-005: \t 动画不支持 \fax/\fay
    // 这个测试验证应用了 shear 动画
    // TODO: 实现后补充
}
```

---

## 五、Phase 8 里程碑与交付物

| 里程碑 | 时间 | 交付物 | 验收标准 |
|--------|------|--------|----------|
| M1: Bug 修复 | W1-2 | 6 个修复 + 回归测试 | cargo test --workspace 全通过 |
| M2: 代码审查规范 | W1-2 | CODE_REVIEW_CHECKLIST.md + ci/check.sh | 脚本可运行，检查覆盖全部规则 |
| M3: PGSDecoder | W2 | pgs-decoder crate | 能解码自身编码的 SUP |
| M4: 性能基线 | W3-4 | 完整 benchmark 报告 | 所有场景有基线数据 |
| M5: 字体缓存 | W4 | GlyphCache 实现 | 重复渲染性能提升 > 40% |
| M6: 高级特性 | W5-6 | \fe 支持 + lenient 增强 + wrap_style | 每个特性有测试 |
| M7: CI/CD | W7 | GitHub Actions 配置 | 全平台自动测试通过 |
| M8: v0.4.0 发布 | W8 | 版本发布 | 全平台二进制 + CHANGELOG |

---

## 六、风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 字体缓存线程安全问题 | 中 | 高 | 使用 RwLock<HashMap>，先单线程验证再并行化 |
| PGS 解码器与编码器不一致 | 中 | 高 | 优先编写 round-trip 测试，迭代修复 |
| 性能优化效果不显著 | 低 | 中 | 先 profile 再优化，避免过早优化 |
| 真实 ASS 文件兼容性差 | 高 | 中 | 渐进式测试，先处理最常见错误类型 |
| wasm 编译目标兼容性 | 中 | 低 | 暂不支持 wasm，专注桌面平台 |
