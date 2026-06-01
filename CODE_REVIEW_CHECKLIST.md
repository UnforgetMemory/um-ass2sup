# CODE_REVIEW_CHECKLIST.md

# ass2sup 代码审查清单

> 每次 PR 必须通过全部 ✅ 项，⚠️ 项需要说明理由。

---

## 一、编译与测试

| # | 检查项 | 命令 | 阻断 |
|---|--------|------|------|
| R-01 | 编译无错误 | `cargo check --workspace` | ✅ |
| R-02 | 编译无警告 | `cargo check --workspace 2>&1 \| grep warning` | ✅ |
| R-03 | Clippy 无警告 | `cargo clippy --workspace -- -D warnings` | ✅ |
| R-04 | 单元测试通过 | `cargo test --workspace --lib` | ✅ |
| R-05 | 集成测试通过 | `cargo test --workspace` | ✅ |
| R-06 | E2E 测试通过 | `cargo test --workspace -- --ignored` | ✅ (发布时) |

## 二、代码质量

| # | 检查项 | 说明 | 阻断 |
|---|--------|------|------|
| Q-01 | 无 `unwrap()` 在生产代码 | `grep -rn "\.unwrap()" crates/*/src/` 应只出现在测试中 | ✅ |
| Q-02 | 无 `panic!()` 在生产代码 | 使用 `Result` + `thiserror` 返回错误 | ✅ |
| Q-03 | 无 `#[allow(dead_code)]` 滥用 | 仅用于确实需要的临时代码 | ⚠️ |
| Q-04 | 匹配穷举 | 无 `_ => {}` 隐藏逻辑（除非有注释说明） | ⚠️ |
| Q-05 | 错误使用 `tracing` | 不使用 `println!`/`eprintln!` | ✅ |
| Q-06 | 错误类型用 `thiserror` | 自定义错误枚举必须派生 `thiserror::Error` | ✅ |

## 三、文档

| # | 检查项 | 说明 | 阻断 |
|---|--------|------|------|
| D-01 | 公开 API 有 `///` | `cargo doc --workspace` 无 "missing documentation" | ✅ |
| D-02 | 复杂逻辑有注释 | 算法、数学公式、非直观设计决策必须有注释 | ⚠️ |
| D-03 | `# Examples` 文档测试 | 公开函数应有至少一个可运行示例 | ⚠️ |
| D-04 | 模块级 `//!` 文档 | 每个 crate 的 lib.rs 应有模块级文档 | ⚠️ |

## 四、测试

| # | 检查项 | 说明 | 阻断 |
|---|--------|------|------|
| T-01 | 新功能有单元测试 | 每个新增 `pub fn` 有对应测试 | ✅ |
| T-02 | Bug 修复有回归测试 | 测试必须覆盖修复前的失败场景 | ✅ |
| T-03 | 测试覆盖正常/异常路径 | 不只是 happy path | ✅ |
| T-04 | 测试描述清晰 | `test_<what>_<condition>_<expected>` 格式 | ⚠️ |

## 五、性能 (影响性能的变更)

| # | 检查项 | 说明 | 阻断 |
|---|--------|------|------|
| P-01 | Benchmark 无退化 | 对比 `cargo bench` baseline | ⚠️ |
| P-02 | 无不必要的内存分配 | 大循环避免 `String::push_str`，使用 `write!` | ⚠️ |
| P-03 | Send/Sync 正确性 | 跨线程类型必须有正确的 trait impl | ✅ |

## 六、安全

| # | 检查项 | 说明 | 阻断 |
|---|--------|------|------|
| S-01 | 无硬编码密钥/路径 | 敏感数据通过环境变量或配置文件传入 | ✅ |
| S-02 | 输入验证 | 外部输入（文件内容）必须验证/清理 | ✅ |
| S-03 | 无 `unsafe` | 除非有充分理由并通过审查 | ✅ (默认) |

## 七、PR 规范

| # | 检查项 | 说明 | 阻断 |
|---|--------|------|------|
| PR-01 | PR 描述清晰 | 包含: 变更原因、方案、测试方式 | ✅ |
| PR-02 | 关联 Issue | 每个修复/功能关联到 Issue | ⚠️ |
| PR-03 | 单一职责 | 一个 PR 只做一件事 | ⚠️ |
| PR-04 | Commit 信息规范 | 使用 conventional commit 格式 | ✅ |
