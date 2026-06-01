# ass2sup Phase 9 — 特效字幕测试/实现/修复/优化 详细规格与执行计划

> 基于代码库深度审计 (59 .rs 文件, 420+ 测试, 62 夹具, 1658 行 renderer.rs, 49 个 OverrideTag)
> 更新日期: 2025-06-01

---

## 一、特效字幕问题清单

### EFFECT-001: `\ko` 描边卡拉OK 未实现
- **位置**: `crates/subtitle-renderer/src/karaoke.rs`
- **问题**: 仅 `\kf` (填充扫描) 有完整实现; `\ko` (描边卡拉OK) 在 `render_event` 中被当作普通文本处理
- **影响**: `\ko` 标签不应显示描边, 仅在描边完成时才显示完整字形
- **修复**: 在 karaoke.rs 中添加 `ko` 模式分支, 跳过描边渲染直到计时到达
- **优先级**: P1
- **工作量**: 中等 (~40 行代码 + 5 个测试)

### EFFECT-002: `apply_transform_tag` 缺少多个标签的动画支持
- **位置**: `crates/subtitle-renderer/src/renderer.rs:667-794`
- **问题**: `\t` 动画插值仅支持 20 个标签, 29 个标签落入 `_ => {}` 被静默忽略
- **缺失标签**: Spacing, Shadow, Border, BorderX/Y, ShadowX/Y, colors (4c/1c-4c), Alpha (1a-4a), Underline, Strikeout, BoldWeight, Org, Fax/Fay
- **影响**: `\t(\bord N)` 等动画不会产生视觉变化
- **修复**: 为关键标签添加 LERP 插值分支
- **优先级**: P1 (高影响力标签) / P2 (低频标签)
- **工作量**: 中等 (~60 行代码 + 10 个测试)

### EFFECT-003: `\org` 旋转原点未实际使用
- **位置**: `crates/subtitle-renderer/src/renderer.rs:build_context()` → `apply_transform_tag()` → `AffineTransform`
- **问题**: `\org` 被解析并存入 `origin_x/origin_y`, 但 `apply_transform_tag` 中 Rotation{z} 使用硬编码 `ctx.x/ctx.y` 而非 `origin_x/origin_y`
- **影响**: `\org(x,y)\frz45` 不会围绕指定原点旋转
- **修复**: 将 `origin_x/origin_y` 传递给 `AffineTransform::rotation()`
- **优先级**: P1
- **工作量**: 小 (~15 行代码 + 3 个测试)

### EFFECT-004: `\frx`/`\fry` 3D 旋转无透视投影
- **位置**: `crates/subtitle-renderer/src/renderer.rs:apply_transform_tag()` + `transform.rs`
- **问题**: X/Y 轴旋转被简化为 shear 变换, 无透视投影
- **影响**: 无法正确渲染 3D 旋转效果
- **修复**: 需要实现完整的 3x3 透视投影矩阵
- **优先级**: P2 (高级效果, 实现复杂)
- **工作量**: 大 (~100 行代码 + 5 个测试)

### EFFECT-005: 卡拉OK 双层路径缺少阴影模糊
- **位置**: `crates/subtitle-renderer/src/karaoke.rs`
- **问题**: 卡拉OK 渲染的双层路径 (填充层 + 描边层) 未在描边层应用 shadow blur
- **影响**: 卡拉OK 字幕的阴影边缘比普通字幕更锐利
- **修复**: 在描边层渲染后应用 blur 效果
- **优先级**: P2
- **工作量**: 小 (~10 行代码 + 2 个测试)

### EFFECT-006: Drawing `\p` 描边使用了错误的颜色
- **位置**: `crates/subtitle-renderer/src/renderer.rs` drawing 分支
- **问题**: `\p` 绘图模式的描边使用 `shadow_color` 而非 `outline_color`
- **影响**: 绘图字幕描边颜色错误
- **修复**: 将 `shadow_color` 替换为 `outline_color`
- **优先级**: P1
- **工作量**: 极小 (~1 行代码 + 1 个测试)

### EFFECT-007: 多行 underline/strikeout 位置不正确
- **位置**: `crates/subtitle-renderer/src/renderer.rs` underline/strikeout 分支
- **问题**: 多行文本的下划线/删除线未根据每行的 y 坐标重新定位
- **影响**: 下划线/删除线可能只出现在第一行下方
- **修复**: 在每行渲染后分别计算 underline/strikeout 位置
- **优先级**: P2
- **工作量**: 中等 (~30 行代码 + 3 个测试)

### EFFECT-008: 嵌入字体未从 CLI 管线加载到 fontdb
- **位置**: `crates/subtitle-renderer/src/font.rs` + CLI 入口
- **问题**: ASS 文件中引用的嵌入字体 (Embedded Fonts) 未被加载到 fontdb
- **影响**: 使用嵌入字体的 ASS 文件会回退到默认字体
- **修复**: 解析 ASS 文件中的字体引用, 从文件系统或 base64 编码中加载
- **优先级**: P2 (需要 ASS 文件包含字体数据)
- **工作量**: 大 (~80 行代码 + 5 个测试)

---

## 二、测试覆盖差距清单

### TEST-001: `\clip`/`\iclip` 像素级正确性测试 (0 个测试)
- **目标**: 验证裁剪区域的像素级准确性
- **测试用例**:
  - 基本矩形裁剪 (clip_x1/y1/x2/y2)
  - 反向裁剪 (clip_inverse=true)
  - 绘图裁剪 (clip_drawing, clip_inverse_drawing)
  - 边界条件: 裁剪区域为 0、超出画面、部分重叠

### TEST-002: `\p` 绘图模式渲染测试 (0 个测试)
- **目标**: 验证绘图命令的正确渲染
- **测试用例**:
  - 基本绘图 (move, line, bezier)
  - 填充模式 (fill) vs 描边模式 (stroke)
  - 绘图 + 缩放 (pbo)
  - 绘图 + 颜色覆盖

### TEST-003: `\q` 换行模式测试 (0 个测试)
- **目标**: 验证 4 种换行模式
- **测试用例**:
  - `\q=0`: 智能换行 (默认)
  - `\q=1`: 行尾换行 (EOL word wrap)
  - `\q=2`: 不换行 (no wrap)
  - `\q=3`: 底部智能换行 (smart wrap from bottom)

### TEST-004: `\t` 动画插值测试 (0 个测试)
- **目标**: 验证动画插值的正确性
- **测试用例**:
  - 基本尺寸动画: `\t(\fscx150,0,1000,1)`
  - 基本颜色动画: `\t(\1c&H0000FF&,0,1000,1)`
  - 复合动画: `\t(\frz45\fscx120,0,1000,1)`
  - 边界条件: t1=t2, t1>t2, accel=0, accel=2

### TEST-005: `\fade`/`\fad` 渐变测试 (0 个测试)
- **目标**: 验证淡入/淡出效果
- **测试用例**:
  - 基本淡入: `\fad(500,0)`
  - 基本淡出: `\fad(0,500)`
  - 复杂渐变: `\fade(255,0,255,0,500,1000)`
  - 边界条件: fade=0, fade>duration

### TEST-006: `\move` 移动动画测试 (0 个测试)
- **目标**: 验证移动路径的正确性
- **测试用例**:
  - 基本移动: `\move(100,100,500,500,0,1000)`
  - 带加速度的移动: `\move(100,100,500,500,0,1000,0.5)`
  - 中间时刻位置验证

### TEST-007: `\bord`/`\shad` 非对称宽度测试 (0 个测试)
- **目标**: 验证 X/Y 方向不同的边框/阴影宽度
- **测试用例**:
  - `\bord2` vs `\bord2\xbord4\ybord6`
  - `\shad2` vs `\shad2\xshad4\yshad6`
  - 非对称描边渲染验证

### TEST-008: `\org` 旋转原点渲染测试 (0 个测试)
- **目标**: 验证旋转围绕指定原点
- **测试用例**:
  - `\org(100,100)\frz45` 应围绕 (100,100) 旋转
  - 对比无 `\org` 时的旋转

### TEST-009: 组合标签测试 (0 个测试)
- **目标**: 验证多个标签同时生效
- **测试用例**:
  - `\frz45\fscx120\1c&H0000FF&` (旋转 + 缩放 + 颜色)
  - `\bord3\shad5\blur2` (描边 + 阴影 + 模糊)
  - `\t(\frz45,0,1000,1)\fad(500,500)` (动画 + 渐变)

### TEST-010: 性能基准测试 (0 个测试)
- **目标**: 建立性能基线
- **测试用例**:
  - 单帧无效果渲染时间
  - 单帧 5 事件渲染时间
  - 卡拉OK 帧渲染时间
  - 重度效果帧渲染时间 (blur+shadow+rotation)
  - 内存使用峰值

---

## 三、Phase 9 执行计划

### Wave 1 (第 1 周): 关键 Bug 修复

#### Task 1.1: 修复 EFFECT-006 (Drawing 描边颜色错误)
**优先级**: P0 (1 行修复, 立即可做)
```
1. 读取 renderer.rs 中 drawing 分支
2. 将 shadow_color 替换为 outline_color
3. 编写 1 个回归测试
4. 验证 cargo test 通过
```
**工作量**: 0.5 天

#### Task 1.2: 修复 EFFECT-003 (`\org` 旋转原点)
**优先级**: P1
```
1. 在 apply_transform_tag 中 Rotation{z} 分支
   将 AffineTransform::rotation(angle, ctx.x, ctx.y)
   改为 AffineTransform::rotation(angle, ctx.origin_x, ctx.origin_y)
2. 编写 3 个测试: 有 org、无 org、org 在画面外
3. 验证 cargo test 通过
```
**工作量**: 0.5 天

#### Task 1.3: 修复 EFFECT-002 (Spacing/Shadow/Border 动画支持)
**优先级**: P1
```
1. 在 apply_transform_tag 中添加:
   - Spacing → lerp spacing 字段
   - Shadow → lerp shadow_depth 字段
   - Border → lerp outline_width 字段
   - ShadowX/Y → lerp shadow_x/shadow_y 字段
   - BorderX/Y → lerp outline_x_width/outline_y_width 字段
2. 编写 6 个测试: 每个标签 1 个动画测试
3. 验证 cargo test 通过
```
**工作量**: 1 天

#### Task 1.4: 修复 EFFECT-005 (卡拉OK 阴影模糊)
**优先级**: P2
```
1. 在 karaoke.rs 双层路径中, 描边层渲染后添加 blur 效果
2. 编写 2 个测试: 有阴影模糊、无阴影模糊
3. 验证 cargo test 通过
```
**工作量**: 0.5 天

### Wave 2 (第 2 周): 测试覆盖

#### Task 2.1: TEST-004 `\t` 动画插值测试
```
1. 创建 tests/fixtures/transform_animation.ass
2. 编写 6 个测试用例:
   - 基本尺寸动画
   - 基本颜色动画
   - 复合动画
   - 边界条件: t1=t2, t1>t2, accel=0
3. 验证每个测试用例的像素级正确性
```
**工作量**: 1 天

#### Task 2.2: TEST-005 `\fade`/`\fad` 渐变测试
```
1. 创建 tests/fixtures/fade_effects.ass
2. 编写 4 个测试用例:
   - 基本淡入
   - 基本淡出
   - 复杂渐变
   - 边界条件: fade=0, fade>duration
3. 验证 alpha 值在时间线上的正确性
```
**工作量**: 0.5 天

#### Task 2.3: TEST-006 `\move` 移动动画测试
```
1. 创建 tests/fixtures/move_animation.ass
2. 编写 3 个测试用例:
   - 基本移动
   - 带加速度的移动
   - 中间时刻位置验证
3. 验证位置插值的正确性
```
**工作量**: 0.5 天

#### Task 2.4: TEST-001 `\clip`/`\iclip` 像素级正确性测试
```
1. 创建 tests/fixtures/clip_effects.ass
2. 编写 5 个测试用例:
   - 基本矩形裁剪
   - 反向裁剪
   - 绘图裁剪
   - 边界条件: 裁剪区域为 0、超出画面
3. 逐像素验证裁剪区域
```
**工作量**: 1 天

#### Task 2.5: TEST-002 `\p` 绘图模式测试
```
1. 创建 tests/fixtures/drawing_mode.ass
2. 编写 4 个测试用例:
   - 基本绘图 (move, line, bezier)
   - 填充模式 vs 描边模式
   - 绘图 + 缩放
   - 绘图 + 颜色覆盖
3. 验证绘图路径的正确渲染
```
**工作量**: 1 天

### Wave 3 (第 3 周): 高级效果实现

#### Task 3.1: 实现 EFFECT-001 (`\ko` 描边卡拉OK)
**优先级**: P1
```
1. 在 karaoke.rs 中添加 `ko` 模式分支
2. 实现描边卡拉OK 逻辑:
   - 在计时到达前, 不显示描边
   - 在计时到达后, 显示完整字形
3. 创建 tests/fixtures/karaoke_ko.ass
4. 编写 3 个测试用例:
   - 基本描边卡拉OK
   - 描边卡拉OK + 颜色变化
   - 描边卡拉OK 边界条件
5. 验证渲染输出
```
**工作量**: 1.5 天

#### Task 3.2: 实现 TEST-003 `\q` 换行模式测试
```
1. 创建 tests/fixtures/wrap_modes.ass
2. 编写 4 个测试用例:
   - `\q=0`: 智能换行
   - `\q=1`: 行尾换行
   - `\q=2`: 不换行
   - `\q=3`: 底部智能换行
3. 验证换行位置的正确性
```
**工作量**: 1 天

#### Task 3.3: 实现 TEST-007 `\bord`/`\shad` 非对称宽度测试
```
1. 创建 tests/fixtures/asymmetric_border_shadow.ass
2. 编写 3 个测试用例:
   - `\bord2` vs `\bord2\xbord4\ybord6`
   - `\shad2` vs `\shad2\xshad4\yshad6`
   - 非对称描边渲染验证
3. 验证 X/Y 方向的独立控制
```
**工作量**: 0.5 天

#### Task 3.4: 实现 TEST-008 `\org` 旋转原点渲染测试
```
1. 创建 tests/fixtures/rotation_origin.ass
2. 编写 3 个测试用例:
   - `\org(100,100)\frz45` 围绕 (100,100) 旋转
   - 对比无 `\org` 时的旋转
   - `\org` 在画面外
3. 验证旋转中心的正确性
```
**工作量**: 0.5 天

### Wave 4 (第 4 周): 组合测试 + 性能基准

#### Task 4.1: TEST-009 组合标签测试
```
1. 创建 tests/fixtures/combined_tags.ass
2. 编写 3 个测试用例:
   - `\frz45\fscx120\1c&H0000FF&` (旋转 + 缩放 + 颜色)
   - `\bord3\shad5\blur2` (描边 + 阴影 + 模糊)
   - `\t(\frz45,0,1000,1)\fad(500,500)` (动画 + 渐变)
3. 验证多标签同时生效
```
**工作量**: 1 天

#### Task 4.2: TEST-010 性能基准测试
```
1. 创建 benches/renderer_bench.rs (如尚未存在)
2. 编写 5 个基准测试:
   - 单帧无效果渲染时间
   - 单帧 5 事件渲染时间
   - 卡拉OK 帧渲染时间
   - 重度效果帧渲染时间
   - 内存使用峰值
3. 建立性能基线
4. 识别性能瓶颈
```
**工作量**: 1 天

#### Task 4.3: 真实 ASS 文件回归测试
```
1. 收集 5-10 个真实世界的 ASS 字幕文件
2. 创建 tests/fixtures/real_world/ 目录
3. 编写回归测试:
   - 每个文件解析无错误
   - 每个文件渲染无 panic
   - 每个文件生成合理的帧
4. 验证与参考渲染的对比
```
**工作量**: 1 天

---

## 四、验证标准

### 代码质量
- [ ] `cargo test --workspace` 全部通过
- [ ] `cargo clippy --workspace -- -D warnings` 无警告
- [ ] 每个新功能/修复有对应的测试用例
- [ ] 测试覆盖所有边界条件

### 功能完整性
- [ ] EFFECT-001 到 EFFECT-008 全部修复或有明确的"暂不实现"决策
- [ ] TEST-001 到 TEST-010 全部实现
- [ ] 所有测试用例通过

### 性能基线
- [ ] 建立性能基准数据
- [ ] 识别并记录性能瓶颈
- [ ] 无性能退化 (对比 Phase 8)

### 文档
- [ ] 新功能有 rustdoc 注释
- [ ] 测试用例有清晰的描述
- [ ] 已知限制有文档记录

---

## 五、Phase 9 里程碑与交付物

| 里程碑 | 时间 | 交付物 | 验收标准 |
|--------|------|--------|----------|
| M1: 关键 Bug 修复 | W1 | EFFECT-006, 003, 002, 005 修复 | 所有修复有回归测试, cargo test 通过 |
| M2: 动画测试覆盖 | W2 | TEST-004, 005, 006, 001, 002 | 所有测试用例通过 |
| M3: 高级效果实现 | W3 | EFFECT-001 实现 + TEST-003, 007, 008 | \ko 卡拉OK 正确渲染 |
| M4: 组合测试 + 性能 | W4 | TEST-009, 010 + 真实文件回归 | 性能基线建立, 回归测试通过 |
| M5: Phase 9 完成 | W4 末 | 完整的特效测试套件 | 所有验证标准满足 |

---

## 六、风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| `\frx`/`\fry` 3D 旋转实现复杂 | 高 | 中 | 暂时标记为 P2, 优先修复可快速解决的问题 |
| 嵌入字体加载需要 ASS 文件包含字体数据 | 中 | 低 | 暂时跳过, 优先处理渲染逻辑 |
| 绘图模式测试需要复杂的 ASS 文件 | 中 | 中 | 使用简化的绘图命令, 逐步增加复杂度 |
| 性能基准测试需要稳定的环境 | 低 | 低 | 在 CI 环境中运行, 多次运行取平均值 |
| 真实 ASS 文件兼容性问题 | 高 | 中 | 渐进式测试, 优先处理最常见问题 |

---

## 七、依赖关系

```
Phase 8 完成 (BUG-001 到 BUG-006)
    ↓
Phase 9 开始
    ↓
Wave 1: 关键 Bug 修复 (EFFECT-006, 003, 002, 005)
    ↓
Wave 2: 测试覆盖 (TEST-004, 005, 006, 001, 002)
    ↓
Wave 3: 高级效果 (EFFECT-001 + TEST-003, 007, 008)
    ↓
Wave 4: 组合测试 + 性能 (TEST-009, 010 + 回归测试)
    ↓
Phase 9 完成
```

---

## 八、成功标准

Phase 9 成功完成的标志:

1. **所有关键 Bug 修复**: EFFECT-006, 003, 002, 005 全部修复
2. **测试覆盖显著提升**: 从 0 个特效测试增加到 30+ 个测试用例
3. **`\ko` 卡拉OK 实现**: 描边卡拉OK 正确渲染
4. **性能基线建立**: 有可比较的性能数据
5. **真实文件回归**: 能处理真实世界的 ASS 字幕文件
6. **代码质量**: 所有测试通过, 无 clippy 警告

Phase 9 完成后, 项目将具备:
- 完整的特效字幕渲染能力
- 全面的测试覆盖
- 可靠的性能基准
- 真实文件兼容性

这为后续的 GitHub Actions 集成和发布奠定了坚实基础。
