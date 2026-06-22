/// Source location span for error reporting.
///
/// Tracks the byte offset position of parsed elements in the source text.
/// All fields are 1-based for human-readable error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column (byte offset from line start).
    pub column: u32,
    /// Length of the spanned region in bytes.
    pub length: u32,
}

impl Span {
    /// Create a new span.
    #[inline]
    pub const fn new(line: u32, column: u32, length: u32) -> Self {
        Self {
            line,
            column,
            length,
        }
    }

    /// Format as `"line {line}:{col}"` for error messages.
    pub fn display(&self) -> String {
        format!("line {}:{}", self.line, self.column)
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}:{}", self.line, self.column)
    }
}
