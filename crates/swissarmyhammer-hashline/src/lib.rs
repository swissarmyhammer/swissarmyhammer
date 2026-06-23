//! Pure, IO-free hashline anchor primitives.
//!
//! A *hashline anchor* tags a line of text with its 1-based line number and a
//! short content hash, rendered as `N:HH` (for example `42:a3`). The `read
//! files` tool tags content so the model can reference specific lines; the
//! `edit files` tool resolves those anchors back to lines, tolerating small
//! drift (a few lines moved) and rejecting stale edits (the referenced line's
//! content changed).
//!
//! This crate performs no file IO. It operates purely on `&str` content.
//!
//! # Example
//!
//! Tag content, read an anchor back off a tagged line, build an edit keyed by
//! that anchor, and apply it:
//!
//! ```
//! use swissarmyhammer_hashline::{apply, parse_anchor, tag, AnchorOp};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let content = "hello\nworld\n";
//!
//!     // `read files` tags content so the model can reference lines as `N:HH|text`.
//!     let tagged = tag(content, 1);
//!     assert_eq!(tagged.lines().next(), Some("1:86|hello"));
//!
//!     // `edit files` parses the anchor the model referenced (here, line 2).
//!     let line_2 = tagged.lines().nth(1).ok_or("expected a second line")?;
//!     let (line, hash) = parse_anchor(line_2).ok_or("line 2 is not a valid anchor")?;
//!     assert_eq!(line, 2);
//!
//!     // Build an edit keyed by that anchor and apply it to the original content.
//!     let op = AnchorOp {
//!         line,
//!         hash,
//!         replacement: "earth".to_string(),
//!     };
//!     let applied = apply(content, &[op])?;
//!     assert_eq!(applied.content, "hello\nearth\n");
//!
//!     Ok(())
//! }
//! ```

mod line_ending;

/// Line-ending mode detection and rendering.
///
/// Re-exported so callers can detect and preserve a file's line-ending
/// convention (see [`LineEnding::detect`]) without depending on this crate's
/// internal module layout.
pub use line_ending::LineEnding;

/// Compute the staleness hash of a single line.
///
/// The line is hashed with leading and trailing *horizontal* whitespace
/// (spaces and tabs) stripped but interior whitespace preserved, then reduced
/// `mod 256`. The result is a coarse fingerprint: 256 distinct values are
/// enough to detect that a line's content changed, not to uniquely identify it.
/// The line number disambiguates hash collisions.
///
/// Re-indenting a line (changing only leading/trailing horizontal whitespace)
/// yields the same hash; changing interior content yields a different hash.
pub fn hash_line(line: &str) -> u8 {
    let trimmed = line.trim_matches([' ', '\t']);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(trimmed.as_bytes());
    (hasher.finalize() & 0xff) as u8
}

/// Render a hash byte as two lowercase hexadecimal characters (e.g. `0xa3` ->
/// `"a3"`).
pub fn render_hash(hash: u8) -> String {
    format!("{hash:02x}")
}

/// Capacity-estimation divisor for [`tag`]: tagging adds roughly one
/// `1/TAGGING_OVERHEAD_RATIO` (~25%) of the original content length in anchor
/// prefixes (`N:HH|`). Used only to pre-size the output buffer; a wrong estimate
/// costs a reallocation, never correctness.
const TAGGING_OVERHEAD_RATIO: usize = 4;

/// Capacity-estimation constant offset for [`tag`]: a small fixed allowance
/// (in bytes) for anchor prefixes and delimiters on short content, where the
/// proportional [`TAGGING_OVERHEAD_RATIO`] estimate rounds down to near zero.
/// Used only to pre-size the output buffer.
const TAGGING_OVERHEAD_BYTES: usize = 16;

/// Annotate each line of `content` with a hashline anchor.
///
/// Each line becomes `N:HH|line`, where `N` is the absolute 1-based line number
/// (the first line is `start_line`) and `HH` is [`render_hash`] of
/// [`hash_line`]. Line endings present in `content` are preserved.
pub fn tag(content: &str, start_line: usize) -> String {
    let mut out = String::with_capacity(
        content.len() + content.len() / TAGGING_OVERHEAD_RATIO + TAGGING_OVERHEAD_BYTES,
    );
    for (offset, (text, terminator)) in split_lines(content).enumerate() {
        let n = start_line + offset;
        out.push_str(&format!(
            "{n}:{}|{text}{terminator}",
            render_hash(hash_line(text))
        ));
    }
    out
}

/// Split `content` into `(line_text, terminator)` pairs, preserving each line's
/// original terminator (`\n`, `\r\n`, `\r`, or `""` for a final unterminated
/// line). Empty content yields no pairs.
///
/// Unlike [`str::lines`], the terminator is reported so callers can reconstruct
/// content with its original line endings intact (including a mix).
fn split_lines(content: &str) -> impl Iterator<Item = (&str, &str)> {
    let mut rest = content;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        // Find the next line terminator.
        match rest.find(['\n', '\r']) {
            None => {
                let line = rest;
                rest = "";
                Some((line, ""))
            }
            Some(idx) => {
                let line = &rest[..idx];
                let after = &rest[idx..];
                let (terminator, remaining) = if let Some(stripped) = after.strip_prefix("\r\n") {
                    ("\r\n", stripped)
                } else if let Some(stripped) = after.strip_prefix('\r') {
                    ("\r", stripped)
                } else {
                    ("\n", &after[1..])
                };
                rest = remaining;
                Some((line, terminator))
            }
        }
    })
}

/// Parse a hashline anchor `N:HH`, returning the 1-based line number and hash.
///
/// An optional `|text` suffix is tolerated and ignored (the caller uses the
/// text for verification or fallback). Returns `None` for anything that is not
/// a well-formed anchor.
pub fn parse_anchor(s: &str) -> Option<(usize, u8)> {
    // Strip an optional `|text` suffix; the text is ignored here.
    let anchor = s.split_once('|').map(|(a, _)| a).unwrap_or(s);
    let (num, hex) = anchor.split_once(':')?;
    if num.is_empty() || hex.len() != 2 {
        return None;
    }
    let line: usize = num.parse().ok()?;
    let hash = u8::from_str_radix(hex, 16).ok()?;
    Some((line, hash))
}

/// A single edit keyed by a hashline anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorOp {
    /// 1-based line number the anchor originally pointed at.
    pub line: usize,
    /// Expected staleness hash of that line.
    pub hash: u8,
    /// Text to replace the resolved line with.
    pub replacement: String,
}

/// The result of successfully applying a set of [`AnchorOp`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The rewritten content with original line endings preserved.
    pub content: String,
}

/// An error from [`apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashlineError {
    /// An anchor could not be resolved to a line hashing to its expected value,
    /// either at the exact line or anywhere in the proximity window. Carries
    /// the current content re-tagged so the caller can re-anchor and retry.
    Mismatch {
        /// The op that failed to resolve.
        op: AnchorOp,
        /// The current content, freshly re-tagged (each line `N:HH|line`).
        retagged: String,
    },
}

impl std::fmt::Display for HashlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashlineError::Mismatch { op, .. } => write!(
                f,
                "anchor {}:{} did not resolve to a matching line",
                op.line,
                render_hash(op.hash)
            ),
        }
    }
}

impl std::error::Error for HashlineError {}

/// Apply anchor-keyed edits to `content`.
///
/// Each [`AnchorOp`] is resolved to a line by hash: first the exact line `N`,
/// then a proximity search of nearby lines for one hashing to the expected
/// value. A line that cannot be resolved yields [`HashlineError::Mismatch`]
/// carrying the re-tagged current content. Original line endings are preserved.
/// How far from the exact line number proximity search will look for a drifted
/// anchor. The search expands symmetrically outward (1, -1, 2, -2, ...) up to
/// this many lines on each side.
const PROXIMITY_WINDOW: usize = 50;

pub fn apply(content: &str, ops: &[AnchorOp]) -> Result<Applied, HashlineError> {
    // Decompose into (text, terminator) so original line endings survive.
    let mut lines: Vec<(String, String)> = split_lines(content)
        .map(|(text, term)| (text.to_string(), term.to_string()))
        .collect();

    for op in ops {
        match resolve(&lines, op) {
            Some(idx) => lines[idx].0 = op.replacement.clone(),
            None => {
                return Err(HashlineError::Mismatch {
                    op: op.clone(),
                    retagged: tag(content, 1),
                });
            }
        }
    }

    let content = lines.into_iter().map(|(text, term)| text + &term).collect();
    Ok(Applied { content })
}

/// Resolve an [`AnchorOp`] to a 0-based line index whose content hashes to the
/// op's expected hash. Tries the exact 1-based line first, then proximity
/// search expanding outward. Returns `None` if no line in the window matches.
fn resolve(lines: &[(String, String)], op: &AnchorOp) -> Option<usize> {
    let matches = |idx: usize| lines.get(idx).is_some_and(|(t, _)| hash_line(t) == op.hash);

    // Exact line (1-based -> 0-based). `line == 0` is treated as "no exact
    // candidate" and falls through to proximity search from index 0.
    let exact = op.line.checked_sub(1);
    if let Some(idx) = exact {
        if matches(idx) {
            return Some(idx);
        }
    }

    // Proximity search expanding symmetrically outward from the exact line.
    let center = exact.unwrap_or(0) as isize;
    for delta in 1..=PROXIMITY_WINDOW as isize {
        for candidate in [center + delta, center - delta] {
            if candidate >= 0 && matches(candidate as usize) {
                return Some(candidate as usize);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_hash_is_two_lowercase_hex_chars() {
        assert_eq!(render_hash(0xa3), "a3");
        assert_eq!(render_hash(0x00), "00");
        assert_eq!(render_hash(0x0f), "0f");
        assert_eq!(render_hash(0xff), "ff");
    }

    #[test]
    fn hash_line_strips_horizontal_whitespace() {
        // Re-indentation (leading/trailing spaces and tabs) must not change the hash.
        let bare = hash_line("let x = 1;");
        assert_eq!(hash_line("    let x = 1;"), bare);
        assert_eq!(hash_line("\tlet x = 1;"), bare);
        assert_eq!(hash_line("let x = 1;   "), bare);
        assert_eq!(hash_line(" \t let x = 1; \t "), bare);
    }

    #[test]
    fn hash_line_preserves_interior_whitespace() {
        // Interior whitespace is content; changing it changes the hash.
        assert_ne!(hash_line("let x = 1;"), hash_line("let  x = 1;"));
    }

    #[test]
    fn hash_line_detects_interior_content_change() {
        assert_ne!(hash_line("let x = 1;"), hash_line("let x = 2;"));
    }

    #[test]
    fn tag_annotates_each_line_with_number_and_hash() {
        let out = tag("a\nb", 1);
        let expected = format!(
            "1:{}|a\n2:{}|b",
            render_hash(hash_line("a")),
            render_hash(hash_line("b"))
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn tag_honors_start_line() {
        let out = tag("a\nb", 10);
        let expected = format!(
            "10:{}|a\n11:{}|b",
            render_hash(hash_line("a")),
            render_hash(hash_line("b"))
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn tag_empty_content_is_empty() {
        assert_eq!(tag("", 1), "");
    }

    #[test]
    fn tag_preserves_trailing_newline() {
        let out = tag("a\n", 1);
        let expected = format!("1:{}|a\n", render_hash(hash_line("a")));
        assert_eq!(out, expected);
    }

    #[test]
    fn tag_preserves_crlf() {
        let out = tag("a\r\nb", 1);
        let expected = format!(
            "1:{}|a\r\n2:{}|b",
            render_hash(hash_line("a")),
            render_hash(hash_line("b"))
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn parse_anchor_plain() {
        assert_eq!(parse_anchor("42:a3"), Some((42, 0xa3)));
    }

    #[test]
    fn parse_anchor_with_text_suffix() {
        assert_eq!(parse_anchor("42:a3|text"), Some((42, 0xa3)));
    }

    #[test]
    fn parse_anchor_text_suffix_may_contain_pipes() {
        assert_eq!(parse_anchor("42:a3|a|b"), Some((42, 0xa3)));
    }

    #[test]
    fn parse_anchor_rejects_non_anchors() {
        assert_eq!(parse_anchor("hello world"), None);
        assert_eq!(parse_anchor("42"), None);
        assert_eq!(parse_anchor(":a3"), None);
        assert_eq!(parse_anchor("42:"), None);
        assert_eq!(parse_anchor("42:zz"), None);
        assert_eq!(parse_anchor("42:a3a"), None); // hash must be exactly 2 hex chars
        assert_eq!(parse_anchor("x:a3"), None);
        assert_eq!(parse_anchor(""), None);
    }

    #[test]
    fn apply_resolves_exact_line_anchor() {
        let content = "a\nb\nc";
        let op = AnchorOp {
            line: 2,
            hash: hash_line("b"),
            replacement: "B".to_string(),
        };
        let applied = apply(content, &[op]).expect("should resolve exact line");
        assert_eq!(applied.content, "a\nB\nc");
    }

    #[test]
    fn apply_finds_drifted_anchor_by_proximity() {
        // The anchor was created against line 2 ("b"), but content drifted: a
        // line was inserted so "b" now lives on line 3. Proximity search finds it.
        let content = "a\nx\nb\nc";
        let op = AnchorOp {
            line: 2,
            hash: hash_line("b"),
            replacement: "B".to_string(),
        };
        let applied = apply(content, &[op]).expect("proximity should resolve drifted anchor");
        assert_eq!(applied.content, "a\nx\nB\nc");
    }

    #[test]
    fn apply_rejects_true_hash_mismatch_with_retagged_content() {
        // No line anywhere hashes to the expected value -> reject.
        let content = "a\nb\nc";
        let op = AnchorOp {
            line: 2,
            hash: hash_line("totally-different"),
            replacement: "B".to_string(),
        };
        let err = apply(content, std::slice::from_ref(&op)).expect_err("should reject mismatch");
        match err {
            HashlineError::Mismatch { op: got, retagged } => {
                assert_eq!(got, op);
                assert_eq!(retagged, tag(content, 1));
            }
        }
    }

    #[test]
    fn apply_preserves_crlf_line_endings() {
        let content = "a\r\nb\r\nc";
        let op = AnchorOp {
            line: 2,
            hash: hash_line("b"),
            replacement: "B".to_string(),
        };
        let applied = apply(content, &[op]).expect("should resolve");
        assert_eq!(applied.content, "a\r\nB\r\nc");
    }

    #[test]
    fn apply_reindented_line_still_resolves() {
        // The current line is re-indented relative to when the anchor was made;
        // horizontal-whitespace-insensitive hashing means it still matches.
        let content = "a\n    b\nc";
        let op = AnchorOp {
            line: 2,
            hash: hash_line("b"),
            replacement: "B".to_string(),
        };
        let applied = apply(content, &[op]).expect("re-indented line should still resolve");
        assert_eq!(applied.content, "a\nB\nc");
    }

    #[test]
    fn line_ending_detect_covers_each_convention() {
        assert_eq!(LineEnding::detect("a\nb\nc\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("a\r\nb\r\nc\r\n"), LineEnding::CrLf);
        assert_eq!(LineEnding::detect("a\rb\rc\r"), LineEnding::Cr);
        assert_eq!(LineEnding::detect("a\nb\r\nc\r"), LineEnding::Mixed);
        assert_eq!(LineEnding::detect("single line"), LineEnding::Lf);
        assert_eq!(LineEnding::detect(""), LineEnding::Lf);
    }

    #[test]
    fn line_ending_terminators() {
        assert_eq!(LineEnding::Lf.as_terminator(), "\n");
        assert_eq!(LineEnding::CrLf.as_terminator(), "\r\n");
        assert_eq!(LineEnding::Cr.as_terminator(), "\r");
        assert_eq!(LineEnding::Mixed.as_terminator(), "\n");
    }

    #[test]
    fn apply_handles_multiple_ops() {
        let content = "a\nb\nc";
        let ops = vec![
            AnchorOp {
                line: 1,
                hash: hash_line("a"),
                replacement: "A".to_string(),
            },
            AnchorOp {
                line: 3,
                hash: hash_line("c"),
                replacement: "C".to_string(),
            },
        ];
        let applied = apply(content, &ops).expect("both ops should resolve");
        assert_eq!(applied.content, "A\nb\nC");
    }
}
