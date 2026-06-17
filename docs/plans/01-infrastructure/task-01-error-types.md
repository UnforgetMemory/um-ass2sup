# Task 1.1 — Error 枚举 + thiserror

## 目标

建立 v2.0 全部错误处理的统一基线：`ass2sup::Error` 枚举。

## 依赖

- 无

## 文件变更

| 文件 | 变更 |
|------|------|
| `crates/ass2sup-cli/src/error.rs` | 新增 `ass2sup::Error` 枚举 |
| `crates/ass2sup-cli/src/lib.rs` | 重新导出 `Error`, `Result` |
| `Cargo.toml` | 添加 `thiserror = "2"`（如果未就绪） |
| `crates/ass2sup-cli/tests/error_test.rs` | 新增测试 |

## 详细步骤

### 1. 定义错误类型

```rust
// crates/ass2sup-cli/src/error.rs

use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("parse error in {file}: {message}")]
    Parse {
        file: PathBuf,
        message: String,
        line: Option<usize>,
    },

    #[error("render error: {0}")]
    Render(#[from] RenderError),

    #[error("output error: {0}")]
    Output(#[from] OutputError),

    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("font error: {0}")]
    Font(#[from] FontError),

    #[error("color error: {0}")]
    Color(#[from] ColorError),

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("format detection failed for {0}")]
    FormatDetection(PathBuf),

    #[error("validation failed: {0} errors")]
    Validation(usize),

    #[error("invalid CLI argument: {0}")]
    Cli(String),
}

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("event {event_idx} at {pts_ms}ms: {message}")]
    Event {
        event_idx: usize,
        pts_ms: u64,
        message: String,
    },
    #[error("effect {effect:?} failed: {message}")]
    Effect {
        effect: String,
        message: String,
    },
    #[error("backend failed: {0}")]
    Backend(String),
}

#[derive(Error, Debug)]
pub enum OutputError {
    #[error("PGS encoding error: {0}")]
    Pgs(String),
    #[error("BDN XML error: {0}")]
    Bdn(String),
    #[error("TTML error: {0}")]
    Ttml(String),
    #[error("WebVTT error: {0}")]
    WebVtt(String),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to read config {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("config validation error: {0}")]
    Validation(String),
}

#[derive(Error, Debug)]
pub enum FontError {
    #[error("font not found: {0}")]
    NotFound(String),
    #[error("font has no CJK glyphs: {0}")]
    NoCjkGlyphs(String),
    #[error("fallback chain exhausted for {0}")]
    FallbackExhausted(String),
    #[error("fontconfig error: {0}")]
    Fontconfig(String),
}

#[derive(Error, Debug)]
pub enum ColorError {
    #[error("unsupported color space: {0}")]
    Unsupported(String),
    #[error("color conversion error: {0}")]
    Conversion(String),
}
```

### 2. 实现 `From<io::Error>`

```rust
impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}
```

### 3. 单元测试

```rust
// crates/ass2sup-cli/tests/error_test.rs

use ass2sup_cli::error::*;

#[test]
fn test_parse_error_display() {
    let err = Error::Parse {
        file: "test.ass".into(),
        message: "invalid style".into(),
        line: Some(42),
    };
    assert!(err.to_string().contains("test.ass"));
    assert!(err.to_string().contains("invalid style"));
    assert!(err.to_string().contains("42"));
}

#[test]
fn test_io_error_chains_source() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err = Error::io("missing.ass", io_err);
    assert!(err.source().is_some());
}

#[test]
fn test_render_error_via_event() {
    let inner = RenderError::Event {
        event_idx: 100,
        pts_ms: 1234,
        message: "glyph render failed".into(),
    };
    let err: Error = inner.into();
    assert!(err.to_string().contains("event 100"));
    assert!(err.to_string().contains("1234"));
}
```

### 4. 迁移现有错误（渐进）

```rust
// 临时兼容层
impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Cli(s)
    }
}
```

这样现有 `Result<T, String>` 调用点可以渐进迁移到 `Result<T, Error>`。

## 验证门

- [ ] `Error` 10+ 变体全部定义 + 文档
- [ ] 每个变体有 unit test
- [ ] `From<io::Error>`, `From<RenderError>` 等转换实现
- [ ] `cargo clippy -D warnings` 零警告
- [ ] `cargo test` 全部通过

## 后续

- Task 1.2: Config 系统
- Task 1.3: Telemetry
