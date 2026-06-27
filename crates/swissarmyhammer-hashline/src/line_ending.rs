//! Line-ending detection.
//!
//! Ported (copy, not dependency) from
//! `swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` to keep this crate
//! dependency-light. Detects the primary line-ending convention in a string so
//! that [`crate::apply`] can preserve it when rewriting content.

/// Line ending conventions that may appear in file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix line endings: `\n`.
    Lf,
    /// Windows line endings: `\r\n`.
    CrLf,
    /// Classic Mac line endings: `\r`.
    Cr,
    /// Multiple distinct line-ending conventions present in the same content.
    Mixed,
}

impl LineEnding {
    /// Detect the primary line ending convention in `content`.
    ///
    /// Counts each convention independently (CRLF is excluded from the bare LF
    /// and bare CR counts). Empty or single-line content with no line endings
    /// defaults to [`LineEnding::Lf`]. Content containing more than one
    /// convention is reported as [`LineEnding::Mixed`].
    pub fn detect(content: &str) -> Self {
        let crlf_count = content.matches("\r\n").count();
        let lf_count = content.matches('\n').count() - crlf_count; // Exclude CRLF \n
        let cr_count = content.matches('\r').count() - crlf_count; // Exclude CRLF \r

        match (lf_count > 0, crlf_count > 0, cr_count > 0) {
            (false, false, false) => LineEnding::Lf, // Default for empty/no line endings
            (true, false, false) => LineEnding::Lf,
            (false, true, false) => LineEnding::CrLf,
            (false, false, true) => LineEnding::Cr,
            _ => LineEnding::Mixed,
        }
    }

    /// The string this line ending writes between lines.
    ///
    /// [`LineEnding::Mixed`] normalizes to `\n` when reconstructing content,
    /// matching the most common single convention.
    pub fn as_terminator(&self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
            LineEnding::Cr => "\r",
            LineEnding::Mixed => "\n",
        }
    }
}
