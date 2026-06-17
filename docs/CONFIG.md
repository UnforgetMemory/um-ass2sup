# ass2sup 配置文件 (v2.0 基础设施)

> 自 v2.0 起，ass2sup 支持从 TOML 文件加载配置。CLI flag 仍可覆盖配置
> 文件中的对应字段。

## 加载优先级

按以下顺序解析，第一个命中即胜出：

1. `--config <PATH>` CLI 参数（最高优先级）
2. 当前工作目录下的 `./ass2sup.toml`
3. 用户级配置 `~/.config/ass2sup/config.toml`
4. 内置默认值（`ass2sup::config::Config::default()`）

> 注：若上述任一文件存在但解析失败，立即终止并报告错误位置。
> 缺失文件 ≠ 错误 — 直接退回到默认配置。

## 完整 Schema

```toml
# ========== 运行时默认值 ==========
[defaults]
fps = 23.976                 # 默认帧率（23.976 / 24.0 / 25.0 / 29.97 / 30.0）
max_colors = 255             # 调色板最大颜色数（1..=255）
dither = "floyd-steinberg"   # 抖动: "none" | "ordered" | "floyd-steinberg"
parallel_frames = true       # 单文件内并行量化（rayon）

# ========== CJK 字体回退 ==========
[cjk_fallback]
chain = [
  "Noto Sans CJK SC",
  "Source Han Sans CN",
  "Microsoft YaHei",
]
strict = false               # true = 缺失 CJK 字形时报错而非渲染 tofu

# Per-style override: 给指定样式单独的字体回退链
[cjk_fallback.per_style]
OP_1    = ["Source Han Sans CN"]
ED_1    = ["Noto Sans CJK SC", "Microsoft YaHei"]
Default = ["Noto Sans CJK SC"]

# ========== 输出色彩空间 ==========
[color]
output_space = "sdr-bt709"          # "sdr-bt709" | "hdr-bt2020-pq" | "hdr-bt2020-hlg"
tonemap = "hable"                    # "hable" | "aces" | "reinhard"

# ========== Per-style 字体覆盖 ==========
[[style_overrides]]
style = "OP_1"
font  = "Source Han Sans CN"

[[style_overrides]]
style = "ED_1"
font  = "Noto Sans CJK SC"

# ========== 渲染器后端 ==========
[rendering]
backend = "auto"            # "cpu" | "gpu" | "auto"
pixel_accuracy = "high"     # "fast" | "high" | "exact"

# ========== 日志 ==========
log_level = "info"          # "trace" | "debug" | "info" | "warn" | "error"
                            # 可被 ASS2SUP_LOG / RUST_LOG / --verbose / --debug / --quiet 覆盖
```

## 字段约束

| 字段 | 类型 | 默认 | 备注 |
|------|------|------|------|
| `defaults.fps` | float | `23.976` | 必须 > 0 |
| `defaults.max_colors` | int (1..=255) | `255` | PGS 硬上限 |
| `defaults.dither` | enum | `"floyd-steinberg"` | 见上 |
| `defaults.parallel_frames` | bool | `true` | CLI `--parallel-frames` 强制 `true` |
| `cjk_fallback.chain` | string[] | `[]` | 顺序遍历 |
| `cjk_fallback.strict` | bool | `false` | |
| `color.output_space` | enum | `"sdr-bt709"` | |
| `color.tonemap` | enum | `"hable"` | |
| `rendering.backend` | enum | `"auto"` | |
| `rendering.pixel_accuracy` | enum | `"high"` | |
| `log_level` | enum | `null` | `null` → 使用 `--verbose`/`--debug`/`--quiet` 决策 |

## CLI 集成

```bash
# 使用自定义配置
ass2sup input.ass -o output.sup --config ./my-config.toml

# CLI flag 覆盖 config 文件
ass2sup input.ass -o output.sup --config ./base.toml --fps 60.0 --max-colors 128

# CJK fallback 链从 CLI 追加
ass2sup input.ass --cjk-fallback "Noto Sans CJK SC" --cjk-fallback "Microsoft YaHei"
```

## 错误处理

| 场景 | 行为 |
|------|------|
| 文件不存在 | 退回到 `Config::default()`，不报错 |
| 文件存在但 TOML 语法错 | 报错 `ConfigError::Parse`，退出码 1 |
| 文件存在但字段未知 | 报错 `ConfigError::Parse` (`deny_unknown_fields`) |
| CLI 字段未提供 | 使用 config 文件中的值 |
| CLI 字段提供 | 覆盖 config 文件中的值 |

## 编程接口

```rust
use ass2sup_cli::config::{Config, MergeArgs};

// 加载
let cfg = Config::load_default(Some("/etc/ass2sup.toml".as_ref()))?;

// 修改
let mut cfg = Config::default();
cfg.cjk_fallback.chain.push("Noto Sans CJK SC".into());

// CLI 合并
cfg.merge_with_args(&MergeArgs {
    fps: Some(60.0),
    max_colors: Some(128),
    dither: Some("none"),
    parallel_frames: true,
    log_level: None,
});

// 保存
cfg.save("/etc/ass2sup.toml")?;
```

## 验证门

- [x] `deny_unknown_fields` 拒绝未知字段
- [x] serde round-trip 保留所有字段
- [x] 缺失文件不报错
- [x] 错误文件返回 `ConfigError::Parse`
- [x] `Config::merge_with_args` CLI 覆盖优先级
- [x] `docs/CONFIG.md` 文档完成
- [x] `cargo clippy -D warnings` 零警告
- [x] `cargo test` 11/11 配置测试通过
