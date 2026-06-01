# ass2sup Phase 8 — 下一阶段计划

## 当前状态: v0.3.0 完成

```
faf8044 docs: add rustdoc to all public APIs across all crates
9c9be03 feat(v0.3.0): palette reuse, CLI UX, golden tests, benchmarks
a2c1086 feat(renderer): multi-line alignment fix, kf fill sweep, 44 render tests
f859fd6 test: expand ASS fixture collection to 62 files
...
```

**已完成:**
- 完整管线: Parse → Validate → Render → Quantize → PGS Encode
- 50+ ASS override tag 解析器
- 时间动画 (\move, \fad, \fade, \t)
- 卡拉OK渲染 (\k, \kf, \ko, \kt)
- 帧缓存, 调色板复用
- NTSC-aware PTS, 多窗口 PGS
- 字体回退链, 内嵌字体
- 下划线/删除线, \q 换行, \p 绘图模式
- PGS 解码器缓冲区分割
- 420+ 测试, 62 ASS 测试夹具, 完整 rustdoc

---

## Phase 8 方向 (按优先级排序)

### P0: 真实世界兼容性测试

**目标:** 用真实字幕文件验证输出正确性

1. **真实 ASS 文件测试集** (1-2周)
   - 从动漫/影视字幕组收集 20+ 真实 ASS 文件
   - 覆盖: 复杂样式、大量事件、特殊字符、多语言
   - 每个文件: 解析 → 渲染 → 编码 → 验证无 panic
   - 对比 BDSup2Sub 输出 (如果可用)

2. **SUP 输出验证** (1周)
   - 实现 PGS 解码器 (SUP → 位图)
   - 解码 ass2sup 输出的 SUP 文件
   - 验证: 时间码正确、位图非空、段序列合法
   - 对比原始渲染位图 vs 解码位图

3. **跨平台兼容性** (1周)
   - Windows/macOS/Linux 编译测试
   - 字体回退行为差异
   - 行尾符处理 (CRLF vs LF)

### P1: 性能分析与优化

**目标:** 提升渲染和编码速度

1. **Criterion 基准测试完善** (1周)
   - 补充 renderer/encoder 基准测试
   - 覆盖: 单帧渲染、多事件叠加、长字幕文件
   - 建立性能基线

2. **性能分析** (1周)
   - 使用 perf/flamegraph 定位热点
   - 重点关注: 字体光栅化、颜色量化、RLE 编码
   - 目标: 1080p 单帧渲染 < 50ms

3. **优化实施** (1-2周)
   - 字体缓存 (已光栅化字形复用)
   - 调色板复用优化 (已有, 可进一步改进)
   - RLE 编码 SIMD 优化
   - 渲染并行化 (事件级并行)

### P1: 高级 ASS 特性

**目标:** 支持更多 ASS 特效标签

1. **\fe 字符集支持** (0.5周)
   - 解析 \fe 标签
   - 字符集转换 (GBK, Shift-JIS, Big5 等)
   - 依赖: encoding_rs crate

2. **\fscx/\fscy 动画增强** (0.5周)
   - \t 中的 \fscx/\fscy 插值
   - 验证当前实现是否已支持

3. **矩阵变换** (1周)
   - \frx/\fry 独立旋转
   - \fax/\fay 剪切变换
   - 组合变换顺序验证

### P2: CLI 分发与打包

**目标:** 让用户方便安装和使用

1. **交叉编译** (1周)
   - Windows (x64, ARM64)
   - macOS (x64, ARM64)
   - Linux (x64, ARM64, musl)

2. **包管理器支持** (1周)
   - cargo install ass2sup
   - Homebrew formula
   - AUR package
   - Windows scoop/chocolatey

3. **安装脚本** (0.5周)
   - 一键安装脚本
   - 字体依赖说明文档

### P2: 错误恢复增强

**目标:** 更健壮的解析和处理

1. **parse_lenient 增强** (1周)
   - 恢复更多错误类型
   - 行级错误恢复 (跳过坏行, 继续解析)
   - 错误报告包含修复建议

2. **渲染错误处理** (0.5周)
   - 字体缺失时的优雅降级
   - 内存不足时的分块处理
   - 损坏帧的跳过和日志

### P3: 文档与示例

**目标:** 提升易用性

1. **用户指南** (1周)
   - 安装说明
   - 使用示例 (常见场景)
   - 故障排除

2. **API 文档发布** (0.5周)
   - docs.rs 自动发布
   - 示例代码补充

3. **架构文档** (0.5周)
   - 管线流程图
   - 模块依赖图
   - 性能特性说明

---

## 推荐执行顺序

```
Week 1-2:  P0 真实世界兼容性测试 + SUP 解码验证
Week 3-4:  P1 性能分析与优化
Week 5-6:  P1 高级 ASS 特性 + P2 错误恢复
Week 7-8:  P2 CLI 分发打包 + P3 文档
```

## 风险评估

| 风险 | 影响 | 缓解 |
|------|------|------|
| 真实 ASS 文件兼容性差 | 高 | 渐进式测试, 先处理最常见格式 |
| 性能优化效果有限 | 中 | 先 profile, 针对性优化 |
| 交叉编译复杂度高 | 中 | 使用 cross / GitHub Actions |
| PGS 解码器实现复杂 | 高 | 参考 libbluray 实现 |

## 成功标准

- [ ] 20+ 真实 ASS 文件无 panic 处理
- [ ] SUP 解码验证通过率 > 95%
- [ ] 1080p 单帧渲染 < 50ms
- [ ] 全平台编译通过
- [ ] cargo install 可用
- [ ] 用户指南完整
