# Sub-1: 基础设施层 (Infrastructure)

**Sprint**: S0
**周期**: 1-2 周
**依赖**: 无（基线）
**阻塞**: Sub-2 ~ Sub-8 全部

## 目标

建立 v2.0 全部子项目依赖的统一基线：错误处理、配置、telemetry、工作区结构。

## 范围

### In Scope
- `ass2sup::Error` 统一错误类型（`thiserror::Error` + 链式 source）
- `ass2sup::Config` 配置系统（`serde` + TOML）
- `ass2sup::Telemetry` tracing 统一层（已部分就绪，需增强）
- 统一依赖版本（MSRV 1.88，`Cargo.toml` `[workspace.package]` 收敛）
- `tracing-subscriber` 环境变量约定（`ASS2SUP_LOG`, `RUST_LOG` 兼容）

### Out of Scope
- 具体业务逻辑（属于 Sub-2+）
- CLI 接口设计（属于 Sub-8）

## 架构决策

### 错误类型
```rust
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parse error in {file}: {message}")]
    Parse { file: PathBuf, message: String, line: Option<usize> },
    #[error("render error: {0}")]
    Render(#[from] RenderError),
    #[error("output error: {0}")]
    Output(#[from] OutputError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

### 配置系统
- 配置文件路径优先级：CLI `--config <PATH>` > `./ass2sup.toml` > `~/.config/ass2sup/config.toml`
- 热重载（watch 模式）v2.1，v1.0 仅启动时加载
- Schema 验证（serde 拒绝未知字段）

### Telemetry
- `tracing` 已就绪；新增 `ass2sup::telemetry::init()` 统一入口
- 默认级别由 `Config.log_level` 控制，回退 `RUST_LOG`
- `--debug` 自动启用 trace 级别 + source location

## 任务清单

| # | 任务 | 验证 |
|---|------|------|
| 1.1 | 创建 `ass2sup` umbrella crate | `cargo build` 通过 |
| 1.2 | 定义 `Error` 枚举（10 个变体） | unit test 覆盖每个变体 |
| 1.3 | `Config` 结构（30+ 字段） | unit test serde round-trip |
| 1.4 | `telemetry::init()` | unit test 重复 init 安全 |
| 1.5 | MSRV 1.88 升级测试 | `cargo build` + `cargo clippy` |
| 1.6 | 全部现有 440+ 测试通过 | `cargo test --workspace` |

## 验证门 (Definition of Done)

- [ ] `Error` 文档化（每个变体有 `///` rustdoc）
- [ ] `Config` schema 文档化（`docs/CONFIG.md`）
- [ ] 现有测试 0 失败
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 零漂移
- [ ] CI 三平台（Win/macOS/Linux）全过
