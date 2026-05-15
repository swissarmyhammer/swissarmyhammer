//! Helpers for emitting MCP server diagnostics into the global tracing
//! subscriber (which writes `.avp/log` for AVP, `.sah/mcp.log` for `sah serve`).
//!
//! All helpers are designed to be cheap when logging is disabled — callers
//! should still gate JSON serialization with `tracing::enabled!` to avoid
//! allocating arguments that will be discarded by the level filter.
//!
//! # Truncation guarantees
//!
//! [`truncate_utf8_for_log`] truncates strings to a byte budget while
//! respecting UTF-8 character boundaries. This is the safe substitute for the
//! still-unstable `str::floor_char_boundary` referenced by the original task
//! description — we walk back from the requested cut point until we find a
//! byte that starts a valid UTF-8 sequence. The returned [`Cow`] borrows when
//! the input already fits in the budget and only allocates on truncation.
//!
//! # Bounded JSON serialization
//!
//! [`serialize_json_bounded`] streams a `serde::Serialize` value into a buffer
//! that stops accepting bytes once a soft cap is reached, sparing the validator
//! hot path from allocating multi-KB JSON only to throw most of it away. The
//! returned `(string, total_bytes)` reports both the truncated preview and the
//! number of bytes that *would* have been written, so callers can include a
//! `...[+N more bytes]` marker without buffering the full payload.
//!
//! # Default budgets
//!
//! - [`MAX_ARGS_BYTES_INFO`] (512) — tool-call arguments at info level
//! - [`MAX_PREVIEW_BYTES_INFO`] (256) — tool-call response previews at info level
//!
//! Trace-level callers should pass [`usize::MAX`] (or simply not call this
//! helper at all) to capture full payloads.

use std::borrow::Cow;

/// Default byte budget for tool-call argument JSON at info level.
///
/// Larger argument blobs will be truncated to this many bytes (at a UTF-8
/// boundary) and decorated with a `...[+N more bytes]` suffix. Trace-level
/// runs can opt into the full payload by gating with
/// `tracing::enabled!(Level::TRACE)` and skipping truncation.
pub const MAX_ARGS_BYTES_INFO: usize = 512;

/// Default byte budget for tool-call response previews at info level.
///
/// Smaller than [`MAX_ARGS_BYTES_INFO`] because we are showing the *first*
/// bytes of the response — enough to recognize what the model received without
/// dumping potentially large file contents into every log line.
pub const MAX_PREVIEW_BYTES_INFO: usize = 256;

/// Find the largest byte index `<= max_bytes` that lies on a UTF-8 character
/// boundary in `s`.
///
/// Returns `s.len()` when the string already fits. Walks back at most three
/// bytes from `max_bytes` (UTF-8 sequences are at most four bytes long, so any
/// non-boundary index is within three bytes of a boundary). Replacement for
/// the unstable `str::floor_char_boundary`.
fn floor_char_boundary(s: &str, max_bytes: usize) -> usize {
    if max_bytes >= s.len() {
        return s.len();
    }
    let mut idx = max_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Truncate `s` to at most `max_bytes` bytes, respecting UTF-8 boundaries,
/// and append a `...[+N more bytes]` marker when the input was longer.
///
/// Returns the input as-is (no allocation) when it already fits in the budget
/// — the common case for short tool-call previews. Allocates a new `String`
/// only when truncation is required.
///
/// # Example
///
/// ```ignore
/// use swissarmyhammer_tools::mcp::tracing_util::truncate_utf8_for_log;
///
/// // Below the budget: borrowed.
/// assert_eq!(truncate_utf8_for_log("hello", 10), "hello");
///
/// // Over the budget: cut at a UTF-8 boundary, marker appended.
/// let long = "a".repeat(20);
/// let out = truncate_utf8_for_log(&long, 8);
/// assert!(out.starts_with("aaaaaaaa"));
/// assert!(out.contains("...[+12 more bytes]"));
///
/// // Multi-byte: never splits a character.
/// let s = "héllo wörld";
/// let out = truncate_utf8_for_log(s, 5);
/// // 'é' is two bytes; cut at boundary 4 ("hé"+"l"=4 bytes is at boundary)
/// assert!(out.is_char_boundary(out.find("...").unwrap_or(out.len())));
/// ```
pub fn truncate_utf8_for_log(s: &str, max_bytes: usize) -> Cow<'_, str> {
    if s.len() <= max_bytes {
        return Cow::Borrowed(s);
    }
    let cut = floor_char_boundary(s, max_bytes);
    let remaining = s.len() - cut;
    Cow::Owned(format!("{}...[+{} more bytes]", &s[..cut], remaining))
}

/// `std::io::Write` adapter that stops accepting bytes once a soft cap is
/// reached, while still counting how many bytes were attempted.
///
/// The buffer keeps appending until `cap` bytes have been written, then
/// silently discards every subsequent byte. `total_bytes` records the
/// would-be write size — that is what callers report as `bytes` when they
/// want to print the full payload size in the truncated log line.
struct BoundedWriter {
    buf: Vec<u8>,
    cap: usize,
    total_bytes: usize,
}

impl BoundedWriter {
    fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap.min(1024)),
            cap,
            total_bytes: 0,
        }
    }
}

impl std::io::Write for BoundedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.total_bytes = self.total_bytes.saturating_add(buf.len());
        if self.buf.len() < self.cap {
            let remaining = self.cap - self.buf.len();
            let take = remaining.min(buf.len());
            self.buf.extend_from_slice(&buf[..take]);
        }
        // We accept *all* bytes from the caller's perspective so serde_json
        // does not retry the discarded tail. The bytes beyond `cap` are
        // intentionally dropped on the floor.
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Serialize `value` as JSON into a buffer bounded by `max_bytes`, returning
/// `(json_preview, total_bytes_written)`.
///
/// The returned preview is at most `max_bytes` bytes (truncated at a UTF-8
/// boundary) and `total_bytes_written` is the size the full JSON would have
/// taken. When the value fits, the two are equal and no truncation marker is
/// needed.
///
/// This avoids the multi-KB allocation pattern of
/// `serde_json::to_string(...)` followed by [`truncate_utf8_for_log`] — for
/// validator hot paths with `code_context` payloads, the buffer never grows
/// past the byte budget.
pub fn serialize_json_bounded<T: ?Sized + serde::Serialize>(
    value: &T,
    max_bytes: usize,
) -> (String, usize) {
    let mut writer = BoundedWriter::new(max_bytes);
    if serde_json::to_writer(&mut writer, value).is_err() {
        // Mirror the existing `<unserializable>` fallback so callers do not
        // need to differentiate.
        let placeholder = "<unserializable>";
        return (placeholder.to_string(), placeholder.len());
    }
    let total = writer.total_bytes;

    // The buffer may have stopped mid-multibyte; truncate to the last UTF-8
    // boundary so the preview is always valid UTF-8. Any tail bytes past the
    // boundary are dropped — they would not have produced a coherent suffix
    // anyway.
    let buf = writer.buf;
    let valid_end = match std::str::from_utf8(&buf) {
        Ok(_) => buf.len(),
        Err(e) => e.valid_up_to(),
    };
    // SAFETY: valid_end is by construction a valid UTF-8 boundary in `buf`.
    let preview = String::from_utf8(buf[..valid_end].to_vec())
        .expect("valid_end is a valid UTF-8 boundary by construction");

    (preview, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_under_budget_borrows() {
        let out = truncate_utf8_for_log("hello", 10);
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, "hello");
    }

    #[test]
    fn at_exact_budget_borrows() {
        let out = truncate_utf8_for_log("hello", 5);
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, "hello");
    }

    #[test]
    fn over_budget_truncates_with_marker() {
        let out = truncate_utf8_for_log("abcdefghij", 4);
        assert_eq!(out, "abcd...[+6 more bytes]");
    }

    #[test]
    fn empty_string_borrows() {
        let out = truncate_utf8_for_log("", 0);
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, "");
    }

    #[test]
    fn zero_budget_truncates_to_empty_with_marker() {
        let out = truncate_utf8_for_log("hello", 0);
        assert_eq!(out, "...[+5 more bytes]");
    }

    #[test]
    fn never_splits_multibyte_char() {
        // 'é' = 0xC3 0xA9 — boundary at 0 and 2, not 1.
        // Asking for 1-byte budget must walk back to 0.
        let out = truncate_utf8_for_log("é", 1);
        assert_eq!(out, "...[+2 more bytes]");
    }

    #[test]
    fn never_splits_three_byte_char() {
        // '日' = 3 bytes (0xE6 0x97 0xA5)
        // Budget of 2 must walk back to 0 (no valid boundary in {1, 2}).
        let out = truncate_utf8_for_log("日本", 2);
        assert_eq!(out, "...[+6 more bytes]");
    }

    #[test]
    fn cuts_at_boundary_after_multibyte_chars() {
        // "héllo" — 'h' (1) + 'é' (2) + 'l' (1) + 'l' (1) + 'o' (1) = 6 bytes
        // Budget 4 should cut after "hél" (4 bytes).
        let out = truncate_utf8_for_log("héllo", 4);
        assert_eq!(out, "hél...[+2 more bytes]");
    }

    #[test]
    fn floor_char_boundary_basic() {
        assert_eq!(floor_char_boundary("hello", 3), 3);
        assert_eq!(floor_char_boundary("hello", 100), 5);
        assert_eq!(floor_char_boundary("héllo", 2), 1); // boundary at 'h' or after 'é'
        assert_eq!(floor_char_boundary("é", 1), 0);
        assert_eq!(floor_char_boundary("", 0), 0);
    }

    #[test]
    fn floor_char_boundary_emoji() {
        // "👍" = 4 bytes (0xF0 0x9F 0x91 0x8D)
        assert_eq!(floor_char_boundary("👍", 0), 0);
        assert_eq!(floor_char_boundary("👍", 3), 0);
        assert_eq!(floor_char_boundary("👍", 4), 4);
    }

    #[test]
    fn serialize_json_bounded_short_value_fits_in_full() {
        let value = serde_json::json!({"a": 1, "b": "two"});
        let (preview, total) = serialize_json_bounded(&value, 1024);
        // Roundtrip parse: the preview should be valid JSON of the same shape.
        let parsed: serde_json::Value = serde_json::from_str(&preview).unwrap();
        assert_eq!(parsed, value);
        // total reflects the full serialization length and matches preview.
        assert_eq!(total, preview.len());
    }

    #[test]
    fn serialize_json_bounded_long_value_truncates() {
        // 2 KB string — well past the 256 cap.
        let big = "x".repeat(2048);
        let value = serde_json::json!({"big": big});
        let (preview, total) = serialize_json_bounded(&value, 256);
        assert!(preview.len() <= 256, "preview must respect cap");
        assert!(total > preview.len(), "total reports full size");
        // The value is large; a 2 KB string plus JSON overhead is > 256 B.
        assert!(total >= 2048);
    }

    #[test]
    fn serialize_json_bounded_truncated_is_valid_utf8() {
        // Build a value whose JSON is mostly multi-byte chars so the
        // truncation point lands inside a multi-byte sequence by raw bytes.
        let cjk = "日本語".repeat(50); // each char = 3 bytes
        let value = serde_json::json!({"text": cjk});
        let (preview, _total) = serialize_json_bounded(&value, 30);
        // Preview must be valid UTF-8 (no panics, parses as string).
        assert!(std::str::from_utf8(preview.as_bytes()).is_ok());
        // Preview length should be <= cap (UTF-8 trim only reduces it).
        assert!(preview.len() <= 30);
    }

    #[test]
    fn serialize_json_bounded_zero_cap_returns_empty_preview() {
        let value = serde_json::json!({"a": 1});
        let (preview, total) = serialize_json_bounded(&value, 0);
        assert_eq!(preview, "");
        assert!(
            total > 0,
            "total still reflects what would have been written"
        );
    }
}
