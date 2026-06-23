# OverrideTag 语义对照表：libass vs ass-core 实现要求

> 基于 libass `ass_parse.c`（commit `master` 2026）反向工程。
> ass-core 必须以 libass 行为为基准。
>
> 图例：✅ 必须等价、⚠️ 有偏差但可接受、❌ 不得出现此行为

## 颜色/Alpha 标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\1c` / `\c` | ✅ `PrimaryColor` | `change_color(&state->c[0])` | 语义一致 | 无 | 无 |
| `\2c` | ✅ `SecondaryColor` | `change_color(&state->c[1])` | 一致 | 无 | 无 |
| `\3c` | ✅ `OutlineColor` | `change_color(&state->c[2])` | 一致 | 无 | 无 |
| `\4c` | ✅ `ShadowColor` | `change_color(&state->c[3])` | 一致 | 无 | 无 |
| `\1a` | ✅ `PrimaryAlpha` | `change_alpha(&state->c[0])` | 一致 | 无 | 无 |
| `\2a` | ✅ `SecondaryAlpha` | `change_alpha(&state->c[1])` | 一致 | 无 | 无 |
| `\3a` | ✅ `OutlineAlpha` | `change_alpha(&state->c[2])` | 一致 | 无 | 无 |
| `\4a` | ✅ `ShadowAlpha` | `change_alpha(&state->c[3])` | 一致 | 无 | 无 |
| `\alpha` | ✅ `Alpha{value}` | `change_alpha` 全部4通道 | 一致 | 无 | 无 |

## 字体标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\fn` | ✅ `FontName(String)` | `state->family =` | 一致 | 无 | 无 |
| `\fs` | ✅ `FontSize(f64)` | `val>0?val:style->FontSize` | 一致 | 无 | 无 |
| `\fs+2` | ❌ 不识别 | `font_size * (1 + pwr*val/10)` | libass 支持相对值 | 极少数文件 | 添加 `FontSizeRelative` 变体 |
| `\fs-2` | ❌ 不识别 | 同上 | 同上 | 极少数文件 | 同上 |
| `\b1`/`\b0`/`\b-1` | ❌ **event.rs**: `BoldWeight(0)` | `Bold(false)` | **event.rs 的 \b0 掉入 BoldWeight** | **所有在事件中用 \\b0 关加粗的文件** | 删除 event.rs 重复代码 |
| `\b100`...`\b900` | ✅ `BoldWeight(u32)` | `state->bold = val` | 一致 | 无 | 无 |
| `\i1`/`\i0`/`\i-1` | ❌ **event.rs**: `\i0` 不匹配 | `Italic(false)` | **event.rs 漏掉 \i0** | **所有在事件中用 \\i0 关斜体的文件** | 删除 event.rs 重复代码 |
| `\u1`/`\u0` | ❌ **event.rs**: `Underline(bool)`, 但 missing `\u0`? | flags |=/-= | event.rs 的 `\u0` 会掉入 `\u{0}`? | 需要检查 | 删除 event.rs 重复代码 |
| `\s1`/`\s0` | 同上 | 同上 | 同上 | 同上 | 同上 |
| `\fe` | ✅ `Charset(u8)` | `encoding` | 一致 | 无 | 无 |
| `\fn` 带 `@` | ❌ 不处理垂直标记 | `@` prefix → `vertical=1` | libass 用 `@FontName` 标记垂直 | 日语竖排字幕 | 添加 `vertical` 标志或 `FontName` 中保留 `@` |

## 位置/运动标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\pos(x,y)` | ✅ `Pos{x,y}` | `pos_x = v1, pos_y = v2` | 一致 | 无 | 无 |
| `\move(x1,y1,x2,y2)` | ✅ `Move{...}` | 插值计算 | 当前存原始值，libass 做 eval | 合理差异 | 无 |
| `\move` 6参数 | ✅ `t1, t2` | `if(t1>t2) swap` | **libass 交换 t1/t2** | 边缘情况 | 添加 swap |
| `\org(x,y)` | ✅ `Origin{x,y}` | `org_x, org_y` | 一致 | 无 | 无 |
| `\fad(in,out)` | ✅ `Fade{dur_in,dur_out}` | 2-arg: α=255→0→255 | 当前存原始值，libass eval | 合理差异 | 无 |
| `\fade(a1,a2,a3,t1,t2,t3,t4)` | ✅ `FadeComplex{...}` | 7-arg: 分段α曲线 | 当前存原始值，libass eval | 合理差异 | 无 |

## 裁剪标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\clip(x1,y1,x2,y2)` | ✅ `Clip{x1,y1,x2,y2}` | 矩形裁剪 | 一致 | 无 | 无 |
| `\iclip(x1,y1,x2,y2)` | ✅ `ClipInverse{...}` | 反向矩形裁剪 | 一致 | 无 | 无 |
| `\clip(scale,cmds)` | ✅ `ClipDrawing{scale,cmds}` | 矢量裁剪 | 一致 | 无 | 无 |
| `\clip(@)` | ⚠️ `ClipDrawingCurrent` 存在 | `nargs==1 && arg=="@"` | 当前已添加但无专项测试 | 低（刚添加） | 添加测试 |
| `\clip` 缺 `)` | ❌ 解析失败 | 容错到字符串尾 | libass 对缺括号容错 | 罕见的畸形文件 | arg 解析中容错 |

## 变换/动画标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\t(tag,t1,t2,accel)` | ✅ `Transform{...}` | 递归解析内层 | 当前存 `tag: String`，不解析 | 由下游 OverrideExpr 处理 | 无（architecture difference） |
| `\t` 缺 t2 | ✅ 默认 t1 | `t2=0→Duration` | 当前默认 ≤t1 | libass 用 event 持续时间 | 需要确认 |

## 旋转/缩放/剪切标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\frz` / `\fr` | ✅ `Rotation{z}` | `frz` | 一致 | 无 | 无 |
| `\frx` | ✅ `Rotation{x}` | `frx` | 一致 | 无 | 无 |
| `\fry` | ✅ `Rotation{y}` | `fry` | 一致 | 无 | 无 |
| `\fscx(pct)` | ✅ `Scale{x}` | `/100` 后存储 | 当前存 % 值，libass 存比率 | 下游需要做 `/100` | 文档说明 |
| `\fscy(pct)` | ✅ `Scale{y}` | 同上 | 同上 | 同上 | 同上 |
| `\fax` | ✅ `Shear{x}` | fax | 一致 | 无 | 无 |
| `\fay` | ✅ `Shear{y}` | fay | 一致 | 无 | 无 |
| `\fsp` | ✅ `Spacing(f64)` | hspacing | 一致 | 无 | 无 |

## 边框/阴影标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\bord(w)` | ✅ `Border(w)` | border_x = border_y = w | 一致 | 无 | 无 |
| `\xbord` | ✅ `BorderX(w)` | border_x | 一致 | 无 | 无 |
| `\ybord` | ✅ `BorderY(w)` | border_y | 一致 | 无 | 无 |
| `\shad(d)` | ✅ `Shadow(d)` | shadow_x = shadow_y = d | ✅ 当前存单个值 | 下游需要分别 apply | 无 |
| `\xshad` | ✅ `ShadowX(d)` | shadow_x | 一致 | 无 | 无 |
| `\yshad` | ✅ `ShadowY(d)` | shadow_y | 一致 | 无 | 无 |
| `\be` | ✅ `Blur(f64)` | `dtoi32(val + 0.5)`, clamp [0, MAX_BE] | ⚠️ **+0.5 取整** | 亚像素差异，边缘情况 | 添加 +0.5 取整 |
| `\blur` | ✅ `GaussianBlur(f64)` | `dtoi32`, clamp [0, BLUR_MAX] | libass 把 \\blur 当别名 | 行为一致 | 确认 MAX 值匹配 |

## 对齐标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\an1`-`\an9` | ✅ `AlignmentNumpad(u8)` | `numpad2align(val)`, PARSED_A | 一致 | 无 | 无 |
| `\a1`-`\a11` | ✅ `Alignment(u8)` | `((val&3)==0)?5:val` | **❌ 未做 VSFilter 映射** | 罕见但 `\a4`/`\a8` 应映射到中心对齐 | 添加 VSFilter quirk |

## 其他标签

| 标签 | 当前行为 | libass 行为 | 差异 | 影响 | 修改 |
|------|---------|-------------|------|------|------|
| `\q0`-`\q3` | ✅ `WrapStyle(u8)` | wrap_style | 一致 | 无 | 无 |
| `\p0`-`\pN` | ⚠️ `DrawingMode(u8)` | `val<0?0:val` | ❌ 负值解析失败 | 罕见畸形文件 | 加 clamp |
| `\pbo` | ✅ `BaselineOffset(f64)` | pbo | 一致 | 无 | 无 |
| `\r[name]` | ✅ `Reset(String)` | `lookup_style_strict` | 一致 | 无 | 无 |
| `\r`（无参） | ✅ `ResetAll` | `ass_reset_render_context(NULL)` | 一致 | 无 | 无 |
| `\kt` | ✅ karaoke timing | effect_skip_timing | 一致 | 无 | 无 |
| `\k` | ✅ `Karaoke{Instant}` | EF_KARAOKE + timing | 一致 | 无 | 无 |
| `\kf` | ✅ `Karaoke{Fill}` | EF_KARAOKE_KF | 一致 | 无 | 无 |
| `\K` | **❌ 未识别** | **`tag("K")` = \kf 别名** | **大写 K 等同 \\kf** | **所有用大写 K 标记卡拉OK的文件** | 添加 |
| `\ko` | ✅ `Karaoke{Outline}` | EF_KARAOKE_KO | 一致 | 无 | 无 |
| `\unknown` | ✅ `Unknown(String)` | 忽略 | 一致 | 无 | 无 |
| `\!`（强制） | ✅ `AnimationSkip` | 无（libass 无此扩展） | 合理差异 | 无 | 无 |

## 总结

```
安全（无差异）:  36 个标签
需小修改:       5 个标签（\b0, \i0, \K, \a, \be +0.5）
需中修改:       3 个标签（\fs 相对值, \clip 缺括号容错, \p 负值）
需确认:         2 个标签（\u0/\s0 在 event.rs 的状态）
               1 个新概念（\fn 的 @ 垂直标记）
```
