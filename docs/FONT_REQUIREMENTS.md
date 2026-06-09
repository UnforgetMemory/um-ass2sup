# Font Requirements

This document explains how ass2sup resolves fonts, why CJK (Chinese / Japanese / Korean) text needs extra setup, and how to get it rendering correctly.

## How Font Fallback Works

ass2sup uses a fallback chain when picking a font for a subtitle event. The style font from ASS is tried first. If it is not found, the chain walks through five hardcoded fallback families, then takes any font it can find. The chain is tried in order, and the first match wins:

1. **Style font** — The font family specified in the ASS style (or `\fn` override tag). If the style does not specify one, the value of `config.default_font` (defaults to `"Arial"`) is used instead.
2. **Liberation Sans** — Common Linux fallback, metric-compatible with Arial
3. **DejaVu Sans** — Widely installed on Linux desktops, broad Unicode coverage
4. **Noto Sans** — Google's universal font family
5. **Arial** — Windows and macOS system font
6. **Helvetica** — macOS system font
7. **Any available font** — Whatever fontdb returns first

This chain lives in `FontManager::query_with_fallback_inner()` in `crates/subtitle-renderer/src/font.rs`. Each query goes through `fontdb`, which checks both system-installed fonts and any fonts embedded in the ASS file's `[Fonts]` section.

Results are cached so repeated lookups for the same (family, bold, italic) combination are instant.

## Why CJK Fonts Are Not Included

ass2sup does **not** bundle or ship any CJK fonts. Here is why:

- **Binary size**: A single Noto Sans CJK font file is 15–20 MB. Bundling all four variants (SC, TC, JP, KR) across weights would inflate the binary by over 100 MB.
- **Licensing complexities**: CJK fonts often use the SIL Open Font License or other terms that may not align with the project's MIT / Apache-2.0 dual license.
- **Most users already have them**: Desktop users on Linux, Windows, or macOS who work with subtitles almost always have CJK fonts installed already.

**Without a CJK font installed, CJK characters render as blank boxes (tofu).** That is not a bug — it is a missing system dependency.

## Installing CJK Fonts

### Linux

**Debian / Ubuntu:**

```bash
sudo apt-get install fonts-noto-cjk
```

**Fedora:**

```bash
sudo dnf install google-noto-cjk-fonts
```

**Arch Linux:**

```bash
sudo pacman -S noto-fonts-cjk
```

### macOS

Noto Sans CJK is available via Homebrew:

```bash
brew install font-noto-sans-cjk
```

Or download directly from [Google Fonts](https://fonts.google.com/noto).

### Windows

Download the Noto Sans CJK package from [Google Fonts](https://fonts.google.com/noto) and install it system-wide.

## Embedding Fonts in ASS (Bypasses System Fonts)

The ASS format has a `[Fonts]` section where you can embed font files directly in the subtitle file. When a font is embedded this way, ass2sup loads it from the subtitle file itself — no system installation needed.

```ass
[Fonts]
fontname = NotoSansCJKsc-Regular.ttf
```

The embedded font takes priority over system fonts because fontdb checks loaded font data before querying fontconfig. This is the most reliable way to ensure CJK rendering works across different systems, especially in CI or containerized environments.

The `FontManager::load_font_data()` method handles this — it takes raw TTF/OTF bytes (extracted from the ASS file by the parser) and registers them with fontdb.

## The `--font` CLI Option (SRT Input)

SRT files have no style or font metadata, so ass2sup needs explicit instructions on what font to use.

```bash
ass2sup input.srt -o output.sup --font "Noto Sans CJK SC"
```

This sets `config.default_font` (which defaults to `"Arial"` for SRT input). The fallback chain still applies if the specified font is not found:

- If `"Noto Sans CJK SC"` is installed, it is used directly.
- If not, the chain falls through Liberation Sans, DejaVu Sans, and so on.

This option has no effect on ASS input — ASS files specify their own fonts in `[V4+ Styles]`.

## Debugging Font Resolution with Fontconfig

If CJK characters appear as boxes, these fontconfig commands help diagnose what is happening:

```bash
# Check which font resolves for Chinese
fc-match sans-serif:lang=zh

# Check which font resolves for Japanese
fc-match sans-serif:lang=ja

# List all fonts that support Chinese
fc-list :lang=zh

# List all fonts that support Japanese
fc-list :lang=ja

# List installed Noto fonts specifically
fc-list | grep -i noto

# Check if your ASS-specified font is installed at all
fc-match "MyASSFontName"
```

`fc-match` returns the font that fontconfig *would* return. ass2sup uses fontdb (a Rust font database library), which queries the same fontconfig cache on Linux. So if `fc-match` does not find CJK coverage, ass2sup will not either.

## Minimum Recommended Font Set

For reliable CJK subtitle rendering, install at least one of these:

| Font | Coverage | Size (approx) |
|------|----------|---------------|
| Noto Sans CJK SC | Simplified Chinese | 15 MB |
| Noto Sans CJK TC | Traditional Chinese | 15 MB |
| Noto Sans CJK JP | Japanese | 15 MB |
| Noto Sans CJK KR | Korean | 15 MB |
| WenQuanYi Micro Hei | Chinese (lightweight) | 5 MB |

The `fonts-noto-cjk` package on Debian/Ubuntu installs all variants at once and is the simplest path.

For South Asian scripts (Devanagari, Bengali, etc.), install `fonts-noto` or `fonts-noto-hind` as needed.
