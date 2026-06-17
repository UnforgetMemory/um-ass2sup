# Task 1.3 — Telemetry 统一入口

## 目标

建立 `ass2sup::telemetry::init()` 统一 telemetry 初始化入口，统一 logging 行为。

## 依赖

- Task 1.1 (Error)

## 文件变更

| 文件 | 变更 |
|------|------|
| `crates/ass2sup-cli/src/telemetry.rs` | 新增 telemetry 模块 |
| `crates/ass2sup-cli/src/lib.rs` | 重新导出 |
| `crates/ass2sup-cli/tests/telemetry_test.rs` | 新增测试 |

## 详细步骤

### 1. 定义 telemetry 模块

```rust
// crates/ass2sup-cli/src/telemetry.rs

use crate::error::Result;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// Telemetry configuration
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub level: LevelFilter,
    pub color: ColorChoice,
    pub with_source: bool,
    pub with_thread_ids: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            level: LevelFilter::INFO,
            color: ColorChoice::Auto,
            with_source: false,
            with_thread_ids: false,
        }
    }
}

/// Initialize telemetry. Safe to call multiple times (subsequent calls no-op).
pub fn init(config: TelemetryConfig) -> Result<()> {
    use_color = match config.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => std::io::IsTerminal::is_terminal(&std::io::stderr()),
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(config.level.into())
        .from_env_lossy();

    let fmt_layer = fmt::layer()
        .with_ansi(use_color)
        .with_target(config.with_source)
        .with_file(config.with_source)
        .with_line_number(config.with_source)
        .with_thread_ids(config.with_thread_ids)
        .with_timer(fmt::time::uptime())
        .with_writer(std::io::stderr);

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init();

    Ok(())
}

/// Init with default config + ASS2SUP_LOG env var support.
pub fn init_default() -> Result<()> {
    let mut config = TelemetryConfig::default();

    if let Ok(level) = std::env::var("ASS2SUP_LOG") {
        config.level = match level.to_lowercase().as_str() {
            "trace" => LevelFilter::TRACE,
            "debug" => LevelFilter::DEBUG,
            "info" => LevelFilter::INFO,
            "warn" | "warning" => LevelFilter::WARN,
            "error" => LevelFilter::ERROR,
            _ => LevelFilter::INFO,
        };
    }

    if let Ok(color) = std::env::var("ASS2SUP_COLOR") {
        config.color = match color.to_lowercase().as_str() {
            "always" => ColorChoice::Always,
            "never" => ColorChoice::Never,
            _ => ColorChoice::Auto,
        };
    }

    init(config)
}
```

### 2. 单元测试

```rust
// crates/ass2sup-cli/tests/telemetry_test.rs

use ass2sup_cli::telemetry::*;
use tracing::Level;

#[test]
fn test_default_config() {
    let cfg = TelemetryConfig::default();
    assert!(matches!(cfg.level, LevelFilter::INFO));
}

#[test]
fn test_init_idempotent() {
    init(TelemetryConfig::default()).unwrap();
    init(TelemetryConfig::default()).unwrap();
    // 不应 panic
}

#[test]
fn test_ass2sup_log_env() {
    std::env::set_var("ASS2SUP_LOG", "debug");
    init_default().unwrap();
    // 无 panic
    std::env::remove_var("ASS2SUP_LOG");
}
```

## 验证门

- [ ] `telemetry::init()` 接受 `TelemetryConfig`
- [ ] `telemetry::init_default()` 支持 `ASS2SUP_LOG` / `ASS2SUP_COLOR` 环境变量
- [ ] 重复 init 安全（无 panic）
- [ ] uptime timer 集成
- [ ] `cargo clippy -D warnings` 零警告
