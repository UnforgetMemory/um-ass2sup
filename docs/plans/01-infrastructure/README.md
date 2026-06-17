# Sub-1: 基础设施层任务清单

## Sprint 0 — Sub-1 基础设施

**周期**: 1-2 周
**依赖**: 无
**Branch**: `v2.0/sub-1-infrastructure`

## 任务列表

| ID | 任务 | 状态 | 任务 MD |
|----|------|------|---------|
| 1.1 | `Error` 枚举 + thiserror | 🔄 | [task-01-error-types.md](task-01-error-types.md) |
| 1.2 | `Config` + serde + TOML | 🔄 | [task-02-config-system.md](task-02-config-system.md) |
| 1.3 | `telemetry::init()` | 🔄 | [task-03-telemetry.md](task-03-telemetry.md) |
| 1.4 | MSRV 1.88 升级 | ⏳ | 集成到 task-01 |
| 1.5 | 现有 440+ 测试通过 | ⏳ | 验证门 |

## 验证门

- [ ] `Error` 完整 + 文档化
- [ ] `Config` schema + TOML 验证
- [ ] telemetry 统一入口
- [ ] 现有测试 0 失败
- [ ] `cargo clippy -D warnings` 零警告
- [ ] CI 三平台通过
