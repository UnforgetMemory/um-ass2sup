# Sub-9: 测试基础设施 (Testing Infrastructure)

**横向**: 全程（伴随所有 Sub-1 ~ Sub-8）
**周期**: 持续
**依赖**: Sub-1
**阻塞**: 无（提供测试基础）

## 目标

建立生产级测试基础设施：视觉回归、三平台 CI、property-based 测试扩展、Golden 测试框架、性能基准。

## 范围

### In Scope
- 视觉回归（`insta` 已用，扩展到图像）
- 三平台 CI 矩阵（Win/macOS/Linux runners）
- property-based 测试扩展（proptest 已用）
- Golden 测试框架（v0.5.5 baseline + 新 baseline）
- Criterion 性能基准
- Coverage 报告（tarpaulin / llvm-cov）
- 模糊测试（cargo-fuzz 已有 5 target，扩展）

### Out of Scope
- 单元测试本身（由各 sub-project 写）

## 架构决策

### 测试分层
| 层 | 工具 | 时长目标 |
|----|------|---------|
| 单元 | cargo test | < 30s |
| 集成 | cargo test --test | < 5min |
| Golden | cargo test + insta | < 10min |
| 视觉回归 | cargo test + image diff | < 15min |
| 性能 | cargo bench | < 30min |
| 模糊 | cargo fuzz | 持续 |

### CI 矩阵
- 3 OS × 2 Rust version（stable + MSRV）= 6 组合
- 仅 stable 跑完整测试，MSRV 跑编译验证
- 缓存：`~/.cargo`, `target` 持久化

### 视觉回归
```rust
#[test]
fn cjk_no_tofu() {
    let output = render("tests/fixtures/cjk_simple.ass");
    insta::assert_image_snapshot!(output.to_png(), @r###"cjk_no_tofu.png"###);
}
```

## 任务清单

| # | 任务 | 验证 |
|---|------|------|
| 9.1 | insta 图像快照 | 快照生成 + 接受流程 |
| 9.2 | CI 矩阵 (.github/workflows/ci.yml) | CI 通过 |
| 9.3 | proptest 策略库 | 单元测试覆盖 |
| 9.4 | Golden baseline | v0.5.5 → v2.0 baseline |
| 9.5 | Criterion 基准 | benchmark 报告 |
| 9.6 | Coverage 报告 | 报告生成 |
| 9.7 | 模糊测试扩展 | crash 报告 |

## 验证门 (Definition of Done)

- [ ] CI 三平台全过
- [ ] Coverage ≥ 80% 核心模块
- [ ] 视觉回归覆盖所有 Sub-project
- [ ] 性能基准可复现
- [ ] 模糊测试无 crash
