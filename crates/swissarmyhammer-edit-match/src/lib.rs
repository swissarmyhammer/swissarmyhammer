//! Pure, IO-free literal-find ladder for the `edit files` tool.
//!
//! When an `edit files` operation supplies a bare-string `find` (not a hashline
//! anchor), that string is a *description* of a span, not a byte-exact copy: the
//! model may have dropped indentation, normalized line endings, or paraphrased
//! an interior line. [`find_match`] resolves such a description to a concrete
//! byte span in the original content by climbing a four-rung ladder and stopping
//! at the first **unique, confident** match:
//!
//! 1. [`Rung::Exact`] — literal substring match.
//! 2. [`Rung::Normalized`] — match after normalizing line endings and trailing
//!    whitespace, returning the span in the *original* bytes so the caller edits
//!    the original.
//! 3. [`Rung::Anchor`] — match the unique first and last lines of `find` and
//!    span the region between them, tolerating interior drift.
//! 4. [`Rung::Fuzzy`] — similarity-scored match, accepted only when it clears
//!    [`FUZZY_ACCEPT_THRESHOLD`] *and* beats the runner-up by at least
//!    [`FUZZY_RUNNER_UP_MARGIN`]. A fuzzy match is never applied silently.
//!
//! The crate performs no IO; it operates purely on `&str`.
//!
//! # Example
//!
//! A `find` that lost its leading indentation misses the [`Rung::Exact`] rung
//! but is recovered by [`Rung::Normalized`], and the returned span covers the
//! *original* indented bytes — so the caller rewrites the real text on disk:
//!
//! ```
//! use swissarmyhammer_edit_match::{find_match, MatchOutcome, Rung};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let content = "fn outer() {\n    let x = compute();\n}\n";
//!     let find = "let x = compute();"; // the model dropped the 4-space indent
//!
//!     let outcome = find_match(content, find);
//!     let MatchOutcome::Unique { span, rung, .. } = outcome else {
//!         return Err(format!("expected a unique match, got {outcome:?}").into());
//!     };
//!
//!     assert_eq!(rung, Rung::Normalized);
//!     // The span covers the original indented line, not the un-indented find.
//!     assert_eq!(&content[span], "    let x = compute();");
//!     Ok(())
//! }
//! ```

use std::ops::Range;

/// Which rung of the ladder produced a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rung {
    /// Literal substring match — the `find` occurs verbatim in the content.
    Exact,
    /// Match after normalizing line endings and trailing whitespace.
    Normalized,
    /// Match keyed on the unique first and last lines of `find`, spanning the
    /// (possibly drifted) interior between them.
    Anchor,
    /// Similarity-scored match accepted under the fuzzy thresholds.
    Fuzzy,
}

/// A located span in the original content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Byte range into the original content.
    pub range: Range<usize>,
    /// 1-based first line of the span.
    pub start_line: usize,
    /// 1-based last line of the span.
    pub end_line: usize,
    /// The original text covered by `range`.
    pub text: String,
}

/// The result of running the literal-find ladder.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchOutcome {
    /// Exactly one confident match was found.
    Unique {
        /// Byte range into the original content the caller should edit.
        span: Range<usize>,
        /// Which rung produced the match.
        rung: Rung,
        /// Confidence in `[0.0, 1.0]` (1.0 for [`Rung::Exact`]).
        confidence: f32,
    },
    /// Two or more candidates tied with no confident winner. The caller must not
    /// pick one silently.
    Ambiguous {
        /// The competing candidate spans.
        candidates: Vec<Span>,
    },
    /// No candidate cleared the bar for its rung.
    NoMatch {
        /// Best-effort near-miss spans, surfaced for diagnostics. May be empty.
        near: Vec<Span>,
    },
}

/// Minimum similarity (inclusive) for a fuzzy candidate to be accepted as
/// [`MatchOutcome::Unique`]. Provisional; the boundary tests assert against this
/// exact value, so any change is a deliberate contract change.
pub const FUZZY_ACCEPT_THRESHOLD: f32 = 0.85;

/// Minimum similarity gap the best fuzzy candidate must hold over the runner-up
/// to be accepted as the winner; a smaller gap yields [`MatchOutcome::Ambiguous`].
/// Provisional; asserted directly by the boundary tests.
pub const FUZZY_RUNNER_UP_MARGIN: f32 = 0.10;

/// Number of fuzzy near-miss candidates retained in a [`MatchOutcome::NoMatch`]
/// for diagnostics. Keeping only the strongest few avoids returning every line.
const MAX_NEAR_MISSES: usize = 3;

/// Tolerance for the floating-point threshold and margin comparisons.
///
/// Similarities are derived from integer edit counts (`1 - edits / len`), so a
/// value that is mathematically *equal* to a constant can land an ULP below it
/// in `f32` — for example `0.95 - 0.85` evaluates to `0.099999964`, not exactly
/// `0.10`. Comparing with this epsilon makes a candidate sitting *exactly on* the
/// threshold or margin count as meeting it, matching the deterministic boundary
/// tests.
pub const FUZZY_BOUNDARY_EPSILON: f32 = 1e-4;

/// Normalized similarity between two strings in `[0.0, 1.0]`.
///
/// Computed as `1 - levenshtein(a, b) / max(chars(a), chars(b))` over Unicode
/// scalar values: identical strings (including two empty strings) score `1.0`,
/// and strings sharing no aligned content score `0.0`. The fuzzy rung and its
/// boundary tests build on this scale.
pub fn similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    let distance = levenshtein(&a, &b) as f32;
    1.0 - distance / max_len as f32
}

/// Classic two-row Levenshtein edit distance over character slices.
fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Split `content` into the byte ranges of its physical lines (newline
/// excluded; a trailing `\r` before `\n` is also excluded). A trailing newline
/// does not produce a phantom empty final line.
fn physical_lines(content: &str) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (idx, ch) in content.char_indices() {
        if ch == '\n' {
            let end = if idx > start && content.as_bytes()[idx - 1] == b'\r' {
                idx - 1
            } else {
                idx
            };
            lines.push(start..end);
            start = idx + 1;
        }
    }
    if start < content.len() {
        lines.push(start..content.len());
    }
    lines
}

/// Normalize a line for whitespace-insensitive comparison: strip the trailing
/// `\r` (handled by [`physical_lines`] already, but cheap to be safe) and trim
/// leading and trailing horizontal whitespace. Interior whitespace is preserved.
fn normalize_line(s: &str) -> &str {
    s.trim_matches([' ', '\t', '\r'])
}

/// Build a [`Span`] from a byte range over `content`.
fn span_of(content: &str, range: Range<usize>) -> Span {
    let start_line = line_number_at(content, range.start);
    // The end is exclusive; the last covered byte is `range.end - 1`. For an
    // empty range, fall back to the start line.
    let end_line = if range.end > range.start {
        line_number_at(content, range.end - 1)
    } else {
        start_line
    };
    Span {
        range: range.clone(),
        start_line,
        end_line,
        text: content[range].to_string(),
    }
}

/// 1-based line number of the byte at `offset` (count of newlines before it,
/// plus one).
fn line_number_at(content: &str, offset: usize) -> usize {
    content[..offset].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Run the literal-find ladder, returning the first unique confident match or,
/// failing that, an ambiguity / no-match verdict. Pure: `(content, find)` in,
/// [`MatchOutcome`] out, no IO.
pub fn find_match(content: &str, find: &str) -> MatchOutcome {
    if let Some(outcome) = try_exact(content, find) {
        return outcome;
    }
    if let Some(outcome) = try_line_block(content, find, Rung::Normalized) {
        return outcome;
    }
    if let Some(outcome) = try_anchor(content, find) {
        return outcome;
    }
    try_fuzzy(content, find)
}

/// Rung 1 — literal substring match. `None` means "no exact occurrence, descend".
///
/// A *single-line* `find` is a line description, so its literal occurrences must
/// be **line-aligned** (bounded by start/end of content or `\n`) to count as
/// Exact; a mid-line substring (for example the un-indented form of an indented
/// line) is deliberately rejected here so the Normalized rung can recover the
/// full original line. A *multi-line* `find` is treated as a verbatim block and
/// matched as a raw substring.
fn try_exact(content: &str, find: &str) -> Option<MatchOutcome> {
    if find.is_empty() {
        return None;
    }
    let single_line = !find.contains('\n');
    let offsets: Vec<usize> = byte_offsets_of(content, find)
        .into_iter()
        .filter(|&start| !single_line || is_line_aligned(content, start, find.len()))
        .collect();
    match offsets.len() {
        0 => None,
        1 => {
            let start = offsets[0];
            Some(MatchOutcome::Unique {
                span: start..start + find.len(),
                rung: Rung::Exact,
                confidence: 1.0,
            })
        }
        _ => Some(MatchOutcome::Ambiguous {
            candidates: offsets
                .into_iter()
                .map(|start| span_of(content, start..start + find.len()))
                .collect(),
        }),
    }
}

/// Whether the byte range `start..start+len` sits on physical line boundaries:
/// preceded by the start of content or a `\n`, and followed by the end of
/// content or a `\n` (a trailing `\r` before the newline is tolerated).
fn is_line_aligned(content: &str, start: usize, len: usize) -> bool {
    let end = start + len;
    let left_ok = start == 0 || content.as_bytes()[start - 1] == b'\n';
    let right_ok = end == content.len()
        || content.as_bytes()[end] == b'\n'
        || (content.as_bytes()[end] == b'\r'
            && end + 1 < content.len()
            && content.as_bytes()[end + 1] == b'\n')
        || (content.as_bytes()[end] == b'\r' && end + 1 == content.len());
    left_ok && right_ok
}

/// All byte offsets where `needle` occurs in `haystack` (overlapping not
/// required; we advance past each full match).
///
/// `needle` is always non-empty here — the sole caller [`try_exact`] returns
/// early on an empty `find` — so advancing by `needle.len()` never exceeds
/// `haystack.len()` and no overflow guard is needed.
fn byte_offsets_of(haystack: &str, needle: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut from = 0;
    while let Some(rel) = haystack[from..].find(needle) {
        let at = from + rel;
        offsets.push(at);
        from = at + needle.len();
    }
    offsets
}

/// Rung 2 — whitespace-normalized, whole-line-block match.
///
/// Normalizes both the content's physical lines and the `find`'s lines, then
/// locates runs of consecutive content lines whose normalized forms equal the
/// normalized `find` lines. The returned span covers the **original** bytes from
/// the start of the first matched line to the end of the last matched line.
fn try_line_block(content: &str, find: &str, rung: Rung) -> Option<MatchOutcome> {
    let content_lines = physical_lines(content);
    let find_norm: Vec<&str> = find.lines().map(normalize_line).collect();
    // Drop trailing empty normalized lines from `find` (e.g. a trailing newline)
    // so the block length matches the intended content.
    let find_norm = trim_trailing_empty(&find_norm);
    if find_norm.is_empty() {
        return None;
    }
    let content_norm: Vec<&str> = content_lines
        .iter()
        .map(|range| normalize_line(&content[range.clone()]))
        .collect();

    let mut matches: Vec<Range<usize>> = Vec::new();
    if find_norm.len() > content_norm.len() {
        return None;
    }
    for window_start in 0..=(content_norm.len() - find_norm.len()) {
        let window = &content_norm[window_start..window_start + find_norm.len()];
        if window == find_norm {
            let first = &content_lines[window_start];
            let last = &content_lines[window_start + find_norm.len() - 1];
            matches.push(first.start..last.end);
        }
    }

    finalize_block_matches(content, matches, rung)
}

/// Turn a list of matched byte ranges into an outcome: one → Unique, many →
/// Ambiguous, none → descend (`None`).
fn finalize_block_matches(
    content: &str,
    matches: Vec<Range<usize>>,
    rung: Rung,
) -> Option<MatchOutcome> {
    match matches.len() {
        0 => None,
        1 => Some(MatchOutcome::Unique {
            span: matches.into_iter().next().expect("len checked == 1"),
            rung,
            confidence: 1.0,
        }),
        _ => Some(MatchOutcome::Ambiguous {
            candidates: matches
                .into_iter()
                .map(|range| span_of(content, range))
                .collect(),
        }),
    }
}

/// Strip trailing all-whitespace (normalized-empty) lines from a slice.
fn trim_trailing_empty<'a>(lines: &'a [&'a str]) -> Vec<&'a str> {
    let mut end = lines.len();
    while end > 0 && lines[end - 1].is_empty() {
        end -= 1;
    }
    lines[..end].to_vec()
}

/// Rung 3 — first/last-line anchor match.
///
/// Requires `find` to have at least two distinct anchor lines. The first
/// normalized line of `find` must occur on exactly one content line, the last on
/// exactly one content line at or after it; the span runs from the start of the
/// first to the end of the last, covering any interior drift.
fn try_anchor(content: &str, find: &str) -> Option<MatchOutcome> {
    let find_norm: Vec<&str> = find.lines().map(normalize_line).collect();
    let find_norm = trim_trailing_empty(&find_norm);
    if find_norm.len() < 2 {
        return None;
    }
    let first = find_norm.first().copied()?;
    let last = find_norm.last().copied()?;
    if first.is_empty() || last.is_empty() {
        return None;
    }

    let content_lines = physical_lines(content);
    let norm_at = |range: &Range<usize>| normalize_line(&content[range.clone()]).to_string();

    let first_hits: Vec<usize> = content_lines
        .iter()
        .enumerate()
        .filter(|(_, range)| norm_at(range) == first)
        .map(|(i, _)| i)
        .collect();
    let last_hits: Vec<usize> = content_lines
        .iter()
        .enumerate()
        .filter(|(_, range)| norm_at(range) == last)
        .map(|(i, _)| i)
        .collect();

    if first_hits.len() != 1 || last_hits.len() != 1 {
        return None;
    }
    let (start_idx, end_idx) = (first_hits[0], last_hits[0]);
    if end_idx <= start_idx {
        return None;
    }
    let range = content_lines[start_idx].start..content_lines[end_idx].end;
    Some(MatchOutcome::Unique {
        span: range,
        rung: Rung::Anchor,
        confidence: 1.0,
    })
}

/// Rung 4 — fuzzy, similarity-scored match over physical lines.
///
/// Scores every content line against `find`, then applies the threshold and
/// runner-up margin: the best candidate wins only if it clears
/// [`FUZZY_ACCEPT_THRESHOLD`] and beats the runner-up by at least
/// [`FUZZY_RUNNER_UP_MARGIN`]. Multiple above-threshold candidates within the
/// margin are [`MatchOutcome::Ambiguous`]; nothing above threshold is
/// [`MatchOutcome::NoMatch`] with the strongest near-misses retained.
fn try_fuzzy(content: &str, find: &str) -> MatchOutcome {
    let find_norm = normalize_multiline(find);
    let content_lines = physical_lines(content);

    let mut scored: Vec<(f32, Range<usize>)> = content_lines
        .iter()
        .map(|range| {
            let line_norm = normalize_line(&content[range.clone()]);
            (similarity(&find_norm, line_norm), range.clone())
        })
        .collect();
    // Sort by descending similarity; ties keep source order (stable sort).
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).expect("similarity is never NaN"));

    let above: Vec<&(f32, Range<usize>)> = scored
        .iter()
        .filter(|(score, _)| *score >= FUZZY_ACCEPT_THRESHOLD - FUZZY_BOUNDARY_EPSILON)
        .collect();

    match above.len() {
        0 => MatchOutcome::NoMatch {
            near: scored
                .iter()
                .take(MAX_NEAR_MISSES)
                .filter(|(score, _)| *score > 0.0)
                .map(|(_, range)| span_of(content, range.clone()))
                .collect(),
        },
        1 => MatchOutcome::Unique {
            span: above[0].1.clone(),
            rung: Rung::Fuzzy,
            confidence: above[0].0,
        },
        _ => {
            let best = above[0].0;
            let runner_up = above[1].0;
            if best - runner_up >= FUZZY_RUNNER_UP_MARGIN - FUZZY_BOUNDARY_EPSILON {
                MatchOutcome::Unique {
                    span: above[0].1.clone(),
                    rung: Rung::Fuzzy,
                    confidence: best,
                }
            } else {
                MatchOutcome::Ambiguous {
                    candidates: above
                        .iter()
                        .map(|(_, range)| span_of(content, range.clone()))
                        .collect(),
                }
            }
        }
    }
}

/// Normalize a possibly-multiline `find` into a single comparison string by
/// trimming each line and rejoining with `\n`, so fuzzy scoring is insensitive
/// to indentation and line-ending style.
fn normalize_multiline(find: &str) -> String {
    find.lines()
        .map(normalize_line)
        .collect::<Vec<_>>()
        .join("\n")
}
