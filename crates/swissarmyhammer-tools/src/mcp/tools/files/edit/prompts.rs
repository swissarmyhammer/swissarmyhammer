//! The prompts `edit` returns in place of an error, and the payloads they are
//! built from.
//!
//! A `find` that is ambiguous, that matches nothing confidently, that was
//! already applied, or whose target an earlier edit in the same batch consumed
//! is NOT an error. Each case returns a successful tool result whose body is
//! one of these prompts, and leaves the file byte-identical, so the model can
//! read what went wrong and retry in one shot.
//!
//! Every renderer here is pure: it reads content and writes text.

use std::ops::Range;

/// One competing location for an ambiguous `find`. Surfaced to the model so it
/// can disambiguate with [`EditPair::occurrence`](super::args::EditPair::occurrence) on the retry. Carries enough
/// to both describe the choice (line number + current text + context) and apply
/// it (the byte `range` to splice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    /// Byte range into the working content this candidate would overwrite.
    pub(super) range: Range<usize>,
    /// 1-based line number where the candidate begins.
    pub(super) line: usize,
    /// The current text covered by `range`.
    pub(super) text: String,
    /// A few lines of surrounding context (the candidate's neighbourhood),
    /// rendered with line-number gutters for the model to orient against.
    pub(super) context: String,
}

/// One near-miss location for a `find` that matched no rung confidently.
/// Surfaced to the model so it sees exactly how its `find` diverged from the
/// nearest current text and can correct in one shot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NearMiss {
    /// 1-based line number where the near-miss span begins.
    pub(super) line: usize,
    /// The current text at this span (the nearest text to the supplied `find`).
    pub(super) text: String,
    /// A few lines of surrounding context, rendered with line-number gutters.
    pub(super) context: String,
    /// A line-level diff between the supplied `find` and this span's current
    /// text, so the model sees precisely how the two differ.
    pub(super) diff: String,
}

/// Number of context lines rendered on each side of a candidate line.
const CANDIDATE_CONTEXT_RADIUS: usize = 2;

/// The 1-based physical line number containing the byte at `offset`.
pub(super) fn line_number_at(content: &str, offset: usize) -> usize {
    content.as_bytes()[..offset.min(content.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// Render `radius` lines of context on each side of `line` (1-based) from
/// `content`, with a `N: ` line-number gutter so the model can orient against
/// the file. The candidate's own line is included.
fn render_context(content: &str, line: usize, radius: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total == 0 || line == 0 {
        return String::new();
    }
    let first = line.saturating_sub(radius).max(1);
    let last = (line + radius).min(total);
    let mut out = String::new();
    for n in first..=last {
        out.push_str(&format!("{n}: {}\n", lines[n - 1]));
    }
    out
}

/// Build a [`Candidate`] for the byte `range` in `content`.
pub(super) fn candidate_for(content: &str, range: Range<usize>) -> Candidate {
    let line = line_number_at(content, range.start);
    Candidate {
        text: content[range.clone()].to_string(),
        context: render_context(content, line, CANDIDATE_CONTEXT_RADIUS),
        line,
        range,
    }
}

/// Render the human-readable disambiguation prompt for an ambiguous `find`.
///
/// Lists each candidate (1-based, matching the `occurrence` param) with its line
/// number, current text, and surrounding context, and instructs the model to
/// re-issue the edit with `occurrence: N`. This is the body of a *successful*
/// tool result — the file is left unchanged.
pub(super) fn render_ambiguity_prompt(find: &str, candidates: &[Candidate]) -> String {
    let mut out = format!(
        "`find` {find:?} matches {} locations; no unique target. Re-issue the edit \
         with `occurrence: N` (1-based) to pick one, or `replace_all: true` to \
         change every match.\n",
        candidates.len()
    );
    for (idx, candidate) in candidates.iter().enumerate() {
        out.push_str(&format!(
            "\noccurrence {} — line {}, current text {:?}:\n{}",
            idx + 1,
            candidate.line,
            candidate.text,
            candidate.context,
        ));
    }
    out
}

/// Render a line-level diff between the supplied `find` and the nearest current
/// `text`, so the model sees precisely how the two diverge. Lines the model
/// supplied that are absent from the current text are prefixed `-`; current
/// lines absent from `find` are prefixed `+`; common lines are prefixed with a
/// space. Built on [`similar::TextDiff`] over lines.
fn render_find_vs_text_diff(find: &str, text: &str) -> String {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(find, text);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        out.push(sign);
        out.push_str(change.value());
        // `change.value()` keeps the line's own terminator; a final line without
        // one still needs a newline so the gutter signs line up.
        if !change.value().ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// Build a [`NearMiss`] for the byte `range` of a near-miss span in `content`,
/// diffed against the supplied `find`.
pub(super) fn near_miss_for(content: &str, find: &str, range: Range<usize>) -> NearMiss {
    let line = line_number_at(content, range.start);
    let text = content[range].to_string();
    NearMiss {
        diff: render_find_vs_text_diff(find, &text),
        context: render_context(content, line, CANDIDATE_CONTEXT_RADIUS),
        text,
        line,
    }
}

/// Render the human-readable near-miss prompt for a `find` that matched no rung.
///
/// Echoes the searched-for text, then lists each near-miss span (line number,
/// current text, surrounding context, and a line-level diff of `find` vs that
/// text). When the file has nothing close (e.g. an empty file), it says so. This
/// is the body of a *successful* tool result — the file is left unchanged.
pub(super) fn render_near_miss_prompt(find: &str, near: &[NearMiss]) -> String {
    if near.is_empty() {
        return format!(
            "`find` {find:?} did not match and there is no close text in the file. \
             Re-read the file and supply text that exists, or a hashline anchor.\n"
        );
    }
    let mut out = format!(
        "`find` {find:?} did not match. Closest current text ({} near-miss{}); \
         re-issue the edit with text that matches one of these (or a hashline \
         anchor).\n",
        near.len(),
        if near.len() == 1 { "" } else { "es" },
    );
    for miss in near {
        out.push_str(&format!(
            "\nline {}, current text {:?}:\n{}\ndiff (find vs current):\n{}",
            miss.line, miss.text, miss.context, miss.diff,
        ));
    }
    out
}

/// Render the human-readable "likely already applied" prompt for a pair whose
/// `find` was absent but whose `replace` is already present in the content.
///
/// This is the body of a *successful* tool result — the file is left unchanged.
/// The edit was very likely a re-run of one already committed, so we report the
/// idempotent no-op rather than failing with "not found".
pub(super) fn render_already_applied_prompt(find: &str, replace: &str) -> String {
    format!(
        "`find` {find:?} did not match, but `replace` {replace:?} is already \
         present — this edit was likely already applied. The file is unchanged; \
         no action is needed.\n"
    )
}

/// Render the human-readable "consumed target" prompt for a later pair whose
/// target span was overwritten by an earlier pair in the same batch.
///
/// This is the body of a *successful* tool result — the file is left unchanged
/// (the batch is atomic). It names the specific consumed-target case per-edit so
/// the model understands the later `find` no longer exists *because an earlier
/// edit in the same call replaced it*, not because the file never contained it.
pub(super) fn render_consumed_target_prompt(find: &str, line: usize) -> String {
    format!(
        "`find` {find:?} did not match: its target around line {line} was consumed \
         by an earlier edit in this same batch. Re-issue this edit against the \
         already-edited text (or fold it into the earlier edit). The file is \
         unchanged.\n"
    )
}

/// Tests for the near-miss payload and every renderer.
///
/// The builders and renderers are pure, so most of these call them directly.
/// The tests that prove a near-miss reaches the caller as a SUCCESSFUL result
/// with the file untouched can only observe that through
/// [`execute_edit`](super::execute_edit), so those drive the whole operation.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::files::edit::execute_edit;
    use crate::mcp::tools::files::edit::test_support::{ambiguity_args, result_text};
    use std::fs;
    use tempfile::TempDir;

    // =========================================================================
    // No confident match → structured near-miss (not a "not found" error)
    // =========================================================================

    /// The near-miss payload built for a span carries the 1-based line number, the
    /// current text at that span, surrounding context with a line-number gutter,
    /// and a line-level diff between the supplied `find` and the current text.
    /// This is the deterministic core, tested directly on the pure builder.
    #[test]
    fn near_miss_payload_has_line_number_context_and_diff() {
        let content = "alpha\nbeta\ngamma\ndelta\nepsilon\n";
        // Span of line 3 ("gamma"): bytes 11..16.
        let range = 11..16;
        assert_eq!(&content[range.clone()], "gamma");

        let miss = near_miss_for(content, "gramma", range);

        // Line number is 1-based.
        assert_eq!(miss.line, 3);
        // Current text at the span.
        assert_eq!(miss.text, "gamma");
        // Context shows the neighbourhood with a line-number gutter.
        assert!(
            miss.context.contains("3: gamma"),
            "context: {}",
            miss.context
        );
        assert!(
            miss.context.contains("2: beta"),
            "context: {}",
            miss.context
        );
        assert!(
            miss.context.contains("4: delta"),
            "context: {}",
            miss.context
        );
        // Line-level diff: the supplied `find` is the removed line, the current
        // text is the added line.
        assert!(
            miss.diff.contains("-gramma"),
            "diff removes the supplied find: {}",
            miss.diff
        );
        assert!(
            miss.diff.contains("+gamma"),
            "diff adds the current text: {}",
            miss.diff
        );
    }

    /// The rendered no-match prompt echoes the searched-for text and the per-span
    /// near-miss details (line, current text, diff). Tested on the pure renderer.
    #[test]
    fn near_miss_prompt_renders_find_and_per_span_details() {
        let content = "alpha\nbeta\ngamma\n";
        let near = vec![near_miss_for(content, "gramma", 11..16)];
        let prompt = render_near_miss_prompt("gramma", &near);

        assert!(prompt.contains("gramma"), "echoes find: {prompt}");
        assert!(prompt.contains("line 3"), "names the line: {prompt}");
        assert!(prompt.contains("\"gamma\""), "shows current text: {prompt}");
        assert!(prompt.contains("-gramma"), "diff: {prompt}");
        assert!(prompt.contains("+gamma"), "diff: {prompt}");
    }

    /// The empty-near-miss prompt is still a structured message (echoes the find,
    /// states nothing is close) rather than a bare error.
    #[test]
    fn near_miss_prompt_with_no_spans_states_nothing_close() {
        let prompt = render_near_miss_prompt("needle", &[]);
        assert!(prompt.contains("needle"), "echoes find: {prompt}");
        assert!(
            prompt.contains("no close") || prompt.contains("nothing close"),
            "states nothing is close: {prompt}"
        );
        assert!(
            !prompt.contains("not found in file"),
            "legacy error string is gone: {prompt}"
        );
    }

    /// End to end: a `find` with no confident match returns a SUCCESSFUL
    /// structured near-miss (echoes the find, not the legacy error) and leaves the
    /// file byte-identical.
    #[tokio::test]
    async fn near_miss_no_match_is_successful_and_file_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("near_miss.txt");
        let content = "alpha\nbeta\ngamma\ndelta\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = ambiguity_args(
            &test_file.to_string_lossy(),
            "zzz no such needle anywhere zzz",
            "ignored",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "no-match must be a successful structured near-miss: {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));

        let text = result_text(&call);
        assert!(
            text.contains("zzz no such needle anywhere zzz"),
            "must echo the find: {text}"
        );
        assert!(
            !text.contains("not found in file"),
            "legacy bare error string must be gone: {text}"
        );

        // File untouched.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// End to end through the real ladder: a `find` that drifted into the fuzzy
    /// near-miss band (below the accept threshold but above zero similarity)
    /// surfaces the nearest current line with a populated line-level diff in the
    /// rendered prompt. Guards that `MatchOutcome::NoMatch { near }` actually
    /// flows from `find_match` through to the model-facing result.
    #[tokio::test]
    async fn near_miss_populated_diff_flows_through_real_ladder() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("near_miss_fuzzy.txt");
        // "the quick brown fox" vs the find "the quick brown cat" share the long
        // common prefix, so similarity (~0.84) lands just under the fuzzy accept
        // threshold (0.85): no rung accepts it, but it is retained as a near-miss.
        let content = "intro line\nthe quick brown fox\noutro line\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = ambiguity_args(
            &test_file.to_string_lossy(),
            "the quick brown cat",
            "ignored",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "fuzzy near-miss must be a successful result: {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));

        let text = result_text(&call);
        // The nearest line (line 2) is surfaced with its current text and a diff.
        assert!(text.contains("line 2"), "names the nearest line: {text}");
        assert!(
            text.contains("the quick brown fox"),
            "shows nearest current text: {text}"
        );
        assert!(
            text.contains("-the quick brown cat"),
            "diff removes the supplied find: {text}"
        );
        assert!(
            text.contains("+the quick brown fox"),
            "diff adds the current text: {text}"
        );

        // File untouched.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// In a multi-pair batch, the failing pair's near-miss is reported and the
    /// batch stays atomic — the earlier pair that WOULD apply is never flushed, so
    /// the file is byte-identical.
    #[tokio::test]
    async fn near_miss_in_batch_is_atomic_and_per_edit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("near_miss_batch.txt");
        // "one" applies cleanly; the second find matches nothing close.
        let content = "one\ntwo\nthree\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "find": "one", "replace": "ONE" },
                { "find": "zzz no such needle anywhere zzz", "replace": "NOPE" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "failing pair yields a successful near-miss listing: {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));

        let text = result_text(&call);
        // The failing pair's find is echoed (per-edit reporting).
        assert!(
            text.contains("zzz no such needle anywhere zzz"),
            "must echo the failing pair's find: {text}"
        );

        // Byte-identical: the first pair's "one"→"ONE" mutation was NOT committed.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    // =====================================================================
    // Pure helpers: context rendering, diff equal-line, line-ending label
    // =====================================================================

    /// `render_context` returns an empty string for empty content or a 0 line.
    #[test]
    fn test_render_context_empty_and_zero_line() {
        assert_eq!(render_context("", 1, 2), "");
        assert_eq!(render_context("a\nb\n", 0, 2), "");
    }

    /// The find-vs-text diff marks common (Equal) lines with a leading space,
    /// deletions with `-`, and insertions with `+`.
    #[test]
    fn test_render_find_vs_text_diff_marks_equal_lines() {
        let diff = render_find_vs_text_diff("same\nold\n", "same\nnew\n");
        assert!(
            diff.contains(" same"),
            "equal line keeps a space sign: {diff}"
        );
        assert!(diff.contains("-old"));
        assert!(diff.contains("+new"));
    }
}
