# Task 1.2 — Config 系统

## 目标

建立 `ass2sup::Config` 配置系统：serde + TOML，支持 CLI flag 覆盖。

## 依赖

- Task 1.1 (Error)

## 文件变更

| 文件 | 变更 |
|------|------|
| `crates/ass2sup-cli/src/config.rs` | 新增 `Config` 结构 |
| `crates/ass2sup-cli/src/error.rs` | 集成 `ConfigError` |
| `crates/ass2sup-cli/src/lib.rs` | 重新导出 |
| `crates/ass2sup-cli/tests/config_test.rs` | 新增测试 |
| `docs/CONFIG.md` | 配置 schema 文档 |

## 详细步骤

### 1. 定义 Config 结构

```rust
// crates/ass2sup-cli/src/config.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub cjk_fallback: CjkFallback,
    #[serde(default)]
    pub color: ColorConfig,
    #[serde(default)]
    pub style_overrides: Vec<StyleOverride>,
    #[serde(default)]
    pub rendering: RenderingConfig,
    #[serde(default)]
    pub log_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub fps: Option<f64>,
    pub max_colors: Option<u8>,
    pub dither: Option<String>,
    pub parallel_frames: Option<bool>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            fps: Some(23.976),
            max_colors: Some(255),
            dither: Some("floyd-steinberg".to_string()),
            parallel_frames: Some(true),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CjkFallback {
    pub chain: Vec<String>,
    pub per_style: HashMap<String, Vec<String>>,
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorConfig {
    pub output_space: Option<String>,  // "sdr-bt709" | "hdr-bt2020-pq" | "hdr-bt2020-hlg"
    pub tonemap: Option<String>,      // "hable" | "aces" | "reinhard"
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            output_space: Some("sdr-bt709".to_string()),
            tonemap: Some("hable".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleOverride {
    pub style: String,
    pub font: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderingConfig {
    pub backend: Option<String>,  // "cpu" | "gpu" | "auto"
    pub pixel_accuracy: Option<String>,  // "fast" | "high" | "exact"
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            backend: Some("auto".to_string()),
            pixel_accuracy: Some("high".to_string()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: Defaults::default(),
            cjk_fallback: CjkFallback::default(),
            color: ColorConfig::default(),
            style_overrides: Vec::new(),
            rendering: RenderingConfig::default(),
            log_level: None,
        }
    }
}

impl Config {
    /// Load config from file (or default if file doesn't exist).
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Read { path: path.to_path_buf(), source: e })?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to file.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Validation(e.to_string()))?;
        std::fs::write(path, content)
            .map_err(|e| ConfigError::Read { path: path.to_path_buf(), source: e })?;
        Ok(())
    }

    /// Merge with CLI args (CLI takes precedence).
    pub fn merge_with_args(&mut self, args: &Args) {
        if let Some(fps) = args.fps {
            self.defaults.fps = Some(fps);
        }
        if let Some(max_colors) = args.max_colors {
            self.defaults.max_colors = Some(max_colors);
        }
        if let Some(dither) = &args.dither {
            self.defaults.dither = Some(dither.clone());
        }
        if args.parallel_frames {
            self.defaults.parallel_frames = Some(true);
        }
    }
}
```

### 2. CLI 集成

```rust
// 在 Args 结构中添加 --config flag
#[derive(Parser, Debug)]
pub struct Args {
    /// Config file path
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// CJK fallback font (can be repeated)
    #[arg(long = "cjk-fallback", value_name = "FONT")]
    pub cjk_fallback: Vec<String>,

    // ... 现有 args
}
```

### 3. 单元测试

```rust
// crates/ass2sup-cli/tests/config_test.rs

use ass2sup_cli::config::*;
use tempfile::TempDir;

#[test]
fn test_default_config() {
    let cfg = Config::default();
    assert_eq!(cfg.defaults.fps, Some(23.976));
    assert_eq!(cfg.defaults.max_colors, Some(255));
    assert!(cfg.cjk_fallback.chain.is_empty());
    assert!(!cfg.cjk_fallback.strict);
}

#[test]
fn test_serde_round_trip() {
    let cfg = Config {
        cjk_fallback: CjkFallback {
            chain: vec!["Noto Sans CJK SC".to_string()],
            per_style: HashMap::new(),
            strict: true,
        },
        ..Config::default()
    };
    let toml_str = toml::to_string(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.cjk_fallback.chain, cfg.cjk_fallback.chain);
    assert_eq!(parsed.cjk_fallback.strict, true);
}

#[test]
fn test_unknown_field_rejected() {
    let toml_str = r#"
[defaults]
fps = 23.976
unknown_field = "should fail"
"#;
    let result: Result<Config, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}

#[test]
fn test_load_nonexistent_returns_default() {
    let cfg = Config::load(std::path::Path::new("/nonexistent/path.toml")).unwrap();
    assert_eq!(cfg.defaults.fps, Some(23.976));
}

#[test]
fn test_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    let mut cfg = Config::default();
    cfg.cjk_fallback.chain = vec!["Test Font".to_string()];
    cfg.save(&path).unwrap();
    let loaded = Config::load(&path).unwrap();
    assert_eq!(loaded.cjk_fallback.chain, vec!["Test Font".to_string()]);
}

#[test]
fn test_merge_with_args() {
    let mut cfg = Config::default();
    let args = Args {
        fps: Some(60.0),
        max_colors: Some(128),
        dither: Some("none".to_string()),
        parallel_frames: true,
        ..Default::default()
    };
    cfg.merge_with_args(&args);
    assert_eq!(cfg.defaults.fps, Some(60.0));
    assert_eq!(cfg.defaults.max_colors, Some(128));
    assert_eq!(cfg.defaults.dither, Some("none".to_string()));
}
```

### 4. 文档

```markdown
<!-- docs/CONFIG.md -->
# ass2sup 配置文件

配置文件位置（按优先级）：

1. `--config <PATH>` CLI 参数
2. `./ass2sup.toml` 当前目录
3. `~/.config/ass2sup/config.toml` 用户配置

## Schema

```toml
[defaults]
fps = 23.976              # 默认帧率
max_colors = 255          # 默认调色板颜色数
dither = "floyd-steinberg"  # dither 算法: none|ordered|floyd-steinberg
parallel_frames = true    # 启用并行渲染

[cjk_fallback]
chain = ["Noto Sans CJK SC", "Microsoft YaHei"]  # CJK fallback 顺序
strict = true             # 无 fallback 时报错而非渲染 tofu

[cjk_fallback.per_style]
OP_1 = ["Source Han Sans CN"]
Note_1 = ["Noto Sans CJK SC"]

[color]
output_space = "sdr-bt709"  # sdr-bt709 | hdr-bt2020-pq | hdr-bt2020-hlg
tonemap = "hable"           # hable | aces | reinhard

[[style_overrides]]
style = "OP_1"
font = "Source Han Sans CN"

[rendering]
backend = "auto"             # cpu | gpu | auto
pixel_accuracy = "high"      # fast | high | exact

log_level = "info"           # trace | debug | info | warn | error
```
```

## 验证门

- [ ] `Config` 30+ 字段全部支持
- [ ] serde round-trip 测试通过
- [ ] 未知字段拒绝（deny_unknown_fields）
- [ ] `--config` CLI flag 集成
- [ ] `--cjk-fallback` CLI flag 集成
- [ ] `docs/CONFIG.md` 文档完成
- [ ] `cargo clippy -D warnings` 零警告
