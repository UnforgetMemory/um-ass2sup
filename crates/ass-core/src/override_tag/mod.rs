//! ASS override tag parsing — libass/VSFilter compatible.
//!
//! Tag parsing is split into per-category sub-modules for maintainability.
//! Each module handles a family of related tags:
//!
//! | Module | Tags |
//! |--------|------|
//! | `position` | `\pos`, `\move`, `\org` |
//! | `color` | `\1c`-`\4c`, `\1a`-`\4a`, `\alpha` |
//! | `font` | `\fn`, `\fs`, `\b`, `\i`, `\u`, `\s`, `\fe`, alignment, wrap, drawing |
//! | `geometry` | `\fscx`, `\fscy`, `\fax`, `\fay`, `\frx`, `\fry`, `\frz`, `\fsp` |
//! | `border` | `\bord`, `\shad`, `\be`, `\blur` + X/Y variants |
//! | `clip` | `\clip`, `\iclip` (rect, vector, drawing) |
//! | `effect` | `\fad`, `\fade`, `\t` (transform) |
//! | `karaoke` | `\k`, `\kf`, `\K`, `\ko`, `\kt` |
//! | `parse` | dispatch logic + `parse_tags()` |

mod border;
mod clip;
mod color;
mod effect;
mod font;
mod geometry;
mod karaoke;
mod parse;
mod position;
pub(super) mod util;

/// Parse a single override tag string (without the `\` prefix).
pub use parse::parse_one_tag;

/// Parse override tags and karaoke segments from `{...}` blocks in text.
pub use parse::parse_tags;
