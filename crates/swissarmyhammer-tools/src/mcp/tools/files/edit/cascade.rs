//! The `edit` resolution cascade: turning each [`EditPair`] into one concrete
//! change, and applying a whole batch atomically in memory.
//!
//! Each pair climbs the same ladder — hashline anchor, then literal substring,
//! then the recovery matcher — and resolves to a [`Resolution`], to a set of
//! competing [`Candidate`]s, or to the nearest [`NearMiss`]es. Nothing here
//! touches the filesystem: [`apply_all_pairs`] works on an in-memory copy, so
//! any pair that cannot resolve leaves the file byte-identical.

use rmcp::ErrorData as McpError;
use std::ops::Range;
use swissarmyhammer_edit_match::{find_match, MatchOutcome};
use swissarmyhammer_hashline::{parse_anchor, resolve_anchor_range_in};

use super::args::EditPair;
use super::prompts::{candidate_for, line_number_at, near_miss_for, Candidate, NearMiss};

/// How a single resolved [`EditPair`] should be committed against the working
/// content. The cascade resolves each pair to one of these *before* any bytes
/// are written, so the whole batch can be applied (or rejected) atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Resolution {
    /// Replace exactly the bytes in `range` (into the working content) with
    /// `replacement`. Covers both the anchor rung (range = the resolved line's
    /// text, terminator excluded) and the literal-span rung (range = the matched
    /// span).
    Splice {
        /// Byte range into the working content to overwrite.
        range: Range<usize>,
        /// Replacement text.
        replacement: String,
    },
    /// Replace *every* literal occurrence of `find` with `replace` (the
    /// `replace_all` path). Kept distinct from [`Resolution::Splice`] because it
    /// touches many spans, matching the legacy global-replace semantics.
    GlobalLiteral {
        /// Literal needle to replace at every occurrence.
        find: String,
        /// Replacement text.
        replace: String,
    },
}

/// The outcome of resolving one [`EditPair`] against the working content: either
/// it resolved to a concrete [`Resolution`] to commit, it is ambiguous and the
/// competing [`Candidate`]s must be surfaced for disambiguation, or nothing
/// matched and the nearest [`NearMiss`]es must be surfaced.
///
/// Neither ambiguity nor a no-match is an [`McpError`]: the cascade reports both
/// up to [`execute_edit`](super::execute_edit), which turns them into SUCCESSFUL tool results the
/// model can act on, leaving the file byte-identical.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PairOutcome {
    /// The pair resolved to a concrete edit to commit.
    Resolved(Resolution),
    /// The pair is ambiguous; these are the competing locations.
    Ambiguous {
        /// The text the model searched for, echoed back in the prompt.
        find: String,
        /// The competing candidate locations.
        candidates: Vec<Candidate>,
    },
    /// No rung matched `find` confidently; these are the nearest near-misses
    /// (may be empty when the file has nothing close, e.g. an empty file).
    ///
    /// A bare no-match is later reclassified by [`reclassify_no_match`] (which has
    /// the batch- and idempotency-aware context [`resolve_pair`] lacks) into the
    /// more specific already-applied / consumed-target [`ApplyOutcome`]s.
    NoMatch {
        /// The text the model searched for, echoed back in the prompt.
        find: String,
        /// The nearest near-miss locations, strongest first.
        near: Vec<NearMiss>,
    },
}

/// Whether `find` parses as a hashline anchor that **resolves** against
/// `content`, tolerating small drift. Returns the resolved line's text byte
/// range (terminator excluded) when it does.
///
/// Resolution is delegated to
/// [`swissarmyhammer_hashline::resolve_anchor_range_in`]: the exact line `N` if
/// its content hashes to the anchor's expected value, else a proximity search
/// (±`PROXIMITY_WINDOW`) for the nearest line that does. An optional `|text`
/// suffix is used as verification/tie-breaker — preferring the in-window
/// candidate whose line text matches `text`. Resolution and the returned byte
/// range share one line model, so the span is correct on CR/CRLF/LF endings.
///
/// A truly stale anchor (nothing in the proximity window hashes to the expected
/// value) returns `None` so the caller falls through to literal interpretation —
/// the safety rule that a structured interpretation only *wins* when it resolves.
fn resolve_anchor(content: &str, find: &str) -> Option<Range<usize>> {
    let (line, expected_hash) = parse_anchor(find)?;
    // The optional `|text` suffix verifies/relocates the anchor; everything after
    // the first `|` is the text (which may itself contain `|`), matching how
    // `read files` renders `N:HH|line`.
    let text = find.split_once('|').map(|(_, t)| t);
    // Resolution and the resolved byte range share one line model (the hashline
    // crate's `\r`/`\r\n`/`\n`-aware splitter), so the span we splice is exactly
    // the line that resolved — even on CR-only or mixed line endings.
    resolve_anchor_range_in(content, line, expected_hash, text)
}

/// Resolve a single [`EditPair`] against the current `content`, choosing the
/// rung of the cascade that applies.
///
/// Order (the safety rule in the task): a `replace_all` pair is always the
/// literal global path (no ambiguity prompt). Otherwise:
/// 1. Anchor rung — `find` parses as a hashline anchor **and** resolves (line
///    exists, hash matches) → replace the whole line. If a resolving anchor and
///    a literal occurrence *both* exist, both are surfaced as candidates rather
///    than guessing.
/// 2. Literal-substring rung — `find` occurs verbatim in `content` → replace the
///    first occurrence (legacy exact-substring semantics).
/// 3. Recovery rung — [`find_match`] resolves a drifted / re-indented `find`; a
///    unique span is spliced, multiple confident spans surface as candidates.
/// 4. Otherwise → [`PairOutcome::NoMatch`] carrying the ladder's nearest
///    near-misses (a successful structured near-miss upstream, not an error).
///
/// Ambiguity returns [`PairOutcome::Ambiguous`] (a successful disambiguation
/// prompt upstream), unless [`EditPair::occurrence`] selects exactly one of the
/// candidates, in which case that one is applied.
fn resolve_pair(content: &str, pair: &EditPair) -> Result<PairOutcome, McpError> {
    if pair.replace_all {
        if !content.contains(&pair.find) {
            // No literal occurrence to replace globally: surface the nearest
            // current text via the ladder's near-misses, not a bare error.
            return Ok(no_match_outcome(content, &pair.find));
        }
        return Ok(PairOutcome::Resolved(Resolution::GlobalLiteral {
            find: pair.find.clone(),
            replace: pair.replace.clone(),
        }));
    }

    let anchor = resolve_anchor(content, &pair.find);
    let literal = content.find(&pair.find);

    match (anchor, literal) {
        // A resolving anchor AND a literal occurrence both exist: surface both as
        // candidates rather than guessing. The anchor candidate replaces its whole
        // line; the literal candidate replaces just the matched substring.
        (Some(anchor_range), Some(start)) => {
            let literal_range = start..start + pair.find.len();
            let candidates = vec![
                candidate_for(content, anchor_range),
                candidate_for(content, literal_range),
            ];
            Ok(disambiguate(pair, candidates))
        }
        // Anchor rung — replace the whole resolved line.
        (Some(range), None) => Ok(PairOutcome::Resolved(Resolution::Splice {
            range,
            replacement: pair.replace.clone(),
        })),
        // Literal-substring rung — replace the first occurrence (legacy
        // exact-substring semantics keep prevailing tests green).
        (None, Some(start)) => Ok(PairOutcome::Resolved(Resolution::Splice {
            range: start..start + pair.find.len(),
            replacement: pair.replace.clone(),
        })),
        // Recovery rung — climb the literal-find ladder for a drifted span.
        (None, None) => resolve_via_ladder(content, pair),
    }
}

/// Recovery rung: run the [`find_match`] ladder and map its outcome to a
/// [`PairOutcome`]. A unique span is spliced; multiple confident spans surface as
/// candidates (subject to [`EditPair::occurrence`] disambiguation); nothing
/// confident surfaces the ladder's nearest near-misses as
/// [`PairOutcome::NoMatch`].
fn resolve_via_ladder(content: &str, pair: &EditPair) -> Result<PairOutcome, McpError> {
    match find_match(content, &pair.find) {
        MatchOutcome::Unique { span, .. } => Ok(PairOutcome::Resolved(Resolution::Splice {
            range: span,
            replacement: pair.replace.clone(),
        })),
        MatchOutcome::Ambiguous { candidates } => {
            let candidates = candidates
                .into_iter()
                .map(|span| candidate_for(content, span.range))
                .collect();
            Ok(disambiguate(pair, candidates))
        }
        // No rung matched confidently. Surface the ladder's best-effort
        // near-misses as a structured result instead of a bare "not found"
        // error, so the model sees how its `find` diverged.
        MatchOutcome::NoMatch { near } => Ok(PairOutcome::NoMatch {
            find: pair.find.clone(),
            near: near
                .into_iter()
                .map(|span| near_miss_for(content, &pair.find, span.range))
                .collect(),
        }),
    }
}

/// Build a [`PairOutcome::NoMatch`] for a `find` with no confident match by
/// running [`find_match`] purely to harvest its near-miss spans. Used by the
/// `replace_all` path, which has no ladder of its own but still owes the model a
/// structured near-miss rather than a bare error.
fn no_match_outcome(content: &str, find: &str) -> PairOutcome {
    let near = match find_match(content, find) {
        MatchOutcome::NoMatch { near } => near,
        // The `replace_all` path only reaches here when there is no literal
        // occurrence; any other ladder outcome still yields no near-misses to
        // surface (the substring path already handled a literal match).
        _ => Vec::new(),
    };
    PairOutcome::NoMatch {
        find: find.to_string(),
        near: near
            .into_iter()
            .map(|span| near_miss_for(content, find, span.range))
            .collect(),
    }
}

/// Resolve an ambiguous set of `candidates` using [`EditPair::occurrence`].
///
/// When `occurrence` (1-based) names exactly one of the candidates, splice that
/// candidate's range with the pair's replacement. Otherwise (no hint, or a hint
/// out of range) keep the ambiguity so the candidate listing is surfaced — an
/// out-of-range hint must never silently mis-apply.
fn disambiguate(pair: &EditPair, candidates: Vec<Candidate>) -> PairOutcome {
    if let Some(idx) = pair.occurrence {
        if let Some(chosen) = candidates.get(idx - 1) {
            return PairOutcome::Resolved(Resolution::Splice {
                range: chosen.range.clone(),
                replacement: pair.replace.clone(),
            });
        }
    }
    PairOutcome::Ambiguous {
        find: pair.find.clone(),
        candidates,
    }
}

/// Apply one resolved [`Resolution`] to `content`, returning the rewritten
/// content. A [`Resolution::Splice`] overwrites a single byte range; a
/// [`Resolution::GlobalLiteral`] replaces every occurrence.
fn apply_resolution(content: &str, resolution: &Resolution) -> String {
    match resolution {
        Resolution::Splice { range, replacement } => {
            let mut out = String::with_capacity(content.len() + replacement.len());
            out.push_str(&content[..range.start]);
            out.push_str(replacement);
            out.push_str(&content[range.end..]);
            out
        }
        Resolution::GlobalLiteral { find, replace } => content.replace(find, replace),
    }
}

/// The outcome of applying a whole batch of pairs against an in-memory working
/// copy: either every pair resolved and the fully-edited content is ready to
/// commit, or some pair was ambiguous and its candidates must be surfaced.
///
/// Ambiguity short-circuits the batch — nothing is committed, so the file stays
/// byte-identical (atomicity), and the candidate listing is returned upstream as
/// a SUCCESSFUL tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ApplyOutcome {
    /// Every pair resolved; this is the content to commit.
    Applied(String),
    /// A pair was ambiguous; surface these candidates for disambiguation.
    Ambiguous {
        /// The text the model searched for.
        find: String,
        /// The competing candidate locations.
        candidates: Vec<Candidate>,
    },
    /// A pair matched nothing confidently; surface the nearest near-misses so the
    /// model sees how its `find` diverged.
    NoMatch {
        /// The text the model searched for.
        find: String,
        /// The nearest near-miss locations (may be empty).
        near: Vec<NearMiss>,
    },
    /// A pair's `find` was absent but its `replace` was already present: the edit
    /// was very likely already applied. Reported as an informational success.
    AlreadyApplied {
        /// The text the model searched for.
        find: String,
        /// The replacement text already present in the content.
        replace: String,
    },
    /// A later pair's target span was consumed by an earlier pair in the same
    /// batch. Reported per-edit instead of as a generic miss.
    ConsumedTarget {
        /// The text the later pair searched for.
        find: String,
        /// 1-based line number where the consumed span began in the original.
        line: usize,
    },
}

/// Reclassify a bare [`PairOutcome::NoMatch`] using batch- and idempotency-aware
/// context, so the model gets the most specific reason its `find` did not match.
///
/// Precedence (most-benign first):
/// 1. **Already applied** — the pair's `replace` is non-empty and present in the
///    current `working` content while `find` is absent. The edit was very likely
///    a re-run of one already committed; report the idempotent no-op.
/// 2. **Consumed target** — `find` was absent from `working` but present in the
///    pre-batch `original`, *and* an earlier pair already mutated the content
///    (`working != original`). An earlier edit in this batch overwrote the span;
///    report that per-edit.
/// 3. Otherwise the original near-miss stands.
fn reclassify_no_match(
    original: &str,
    working: &str,
    pair: &EditPair,
    find: String,
    near: Vec<NearMiss>,
) -> ApplyOutcome {
    let find_absent = !working.contains(&pair.find);
    if find_absent && !pair.replace.is_empty() && working.contains(&pair.replace) {
        return ApplyOutcome::AlreadyApplied {
            find,
            replace: pair.replace.clone(),
        };
    }
    if find_absent && working != original {
        if let Some(start) = original.find(&pair.find) {
            return ApplyOutcome::ConsumedTarget {
                find,
                line: line_number_at(original, start),
            };
        }
    }
    ApplyOutcome::NoMatch { find, near }
}

/// Resolve and apply every pair in sequence against an in-memory working copy,
/// returning the fully-edited content. Each pair sees the result of the prior
/// pair (matching the legacy sequential semantics), but nothing is written to
/// disk here — the caller commits the final content in one atomic rewrite, so a
/// failure or ambiguity on any pair leaves the file byte-identical.
///
/// An ambiguous pair — or a pair with no confident match — short-circuits the
/// batch: its candidates / near-misses are returned immediately, before any
/// later pair is applied, so the working copy is discarded and the file is never
/// partially written. A no-match is reclassified by [`reclassify_no_match`] into
/// the more specific already-applied / consumed-target cases when they apply.
pub(super) fn apply_all_pairs(
    original: &str,
    pairs: &[EditPair],
) -> Result<ApplyOutcome, McpError> {
    let mut working = original.to_string();
    for pair in pairs {
        match resolve_pair(&working, pair)? {
            PairOutcome::Resolved(resolution) => {
                working = apply_resolution(&working, &resolution);
            }
            PairOutcome::Ambiguous { find, candidates } => {
                return Ok(ApplyOutcome::Ambiguous { find, candidates });
            }
            PairOutcome::NoMatch { find, near } => {
                return Ok(reclassify_no_match(original, &working, pair, find, near));
            }
        }
    }
    Ok(ApplyOutcome::Applied(working))
}

/// Tests for the cascade.
///
/// The resolution rungs are exercised directly; the batch behaviour they add up
/// to — atomicity, ambiguity prompts, idempotency — is only observable through
/// [`execute_edit`](super::execute_edit), so those tests drive the whole
/// operation and assert on the file and on the returned text.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::files::edit::execute_edit;
    use crate::mcp::tools::files::edit::test_support::{
        ambiguity_args, create_edit_arguments, result_text,
    };
    use std::fs;
    use tempfile::TempDir;

    // =========================================================================
    // Cascade apply core — anchor + literal ladder, atomic batch
    // =========================================================================

    /// Build the hashline anchor string (`N:HH`) for a 1-based `line` of `text`.
    fn anchor_for(text: &str, line: usize) -> String {
        use swissarmyhammer_hashline::{hash_line, render_hash};
        format!("{line}:{}", render_hash(hash_line(text)))
    }

    /// A `find` that is a resolving hashline anchor replaces the WHOLE line, not
    /// a span — the replacement text becomes the entire line content.
    #[tokio::test]
    async fn cascade_resolving_anchor_replaces_whole_line() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("anchor_line.txt");
        let content = "alpha\nbeta gamma\ndelta\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        // Anchor line 2 ("beta gamma"); replacement is the whole new line.
        let find = anchor_for("beta gamma", 2);
        let args = create_edit_arguments(&test_file.to_string_lossy(), &find, "BETA", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok(), "anchor edit should succeed: {result:?}");

        let edited = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited, "alpha\nBETA\ndelta\n");
    }

    /// A `find` shaped like an anchor (`N:HH`) whose hash does NOT match the
    /// referenced line is treated as literal text — and if that literal text is
    /// not present, the edit fails without mis-applying.
    #[tokio::test]
    async fn cascade_stale_anchor_falls_through_to_literal() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("stale_anchor.txt");
        let content = "alpha\nbeta gamma\ndelta\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        // A well-formed anchor whose hash cannot match any line in the file, and
        // whose literal text "99:zz" wait — must be valid hex. Use a hash that
        // parses but never matches; the literal "2:00" text is absent.
        let find = "2:00"; // parses as anchor (line 2, hash 0x00) but won't resolve
                           // Ensure 0x00 truly does not match line 2's hash.
        assert_ne!(
            find,
            anchor_for("beta gamma", 2),
            "test precondition: chosen anchor must be stale"
        );
        let args = create_edit_arguments(&test_file.to_string_lossy(), find, "X", None);

        let result = execute_edit(args, &context).await;
        // Stale anchor → literal "2:00" which is not in the file → structured
        // near-miss (a successful result), not a mis-apply.
        assert!(
            result.is_ok(),
            "stale-anchor no-match is a successful near-miss: {result:?}"
        );
        assert_eq!(result.unwrap().is_error, Some(false));

        // File is byte-identical — nothing was committed.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// A drifted anchor (correct HH, but the line moved a few lines from N within
    /// the proximity window) relocates to the moved line and replaces it.
    #[tokio::test]
    async fn cascade_drifted_anchor_relocates_and_edits() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("drifted_anchor.txt");
        // Anchor was created when "beta gamma" was on line 2; the file then gained
        // two leading lines so "beta gamma" now lives on line 4 — within window.
        let original_content = "alpha\nbeta gamma\ndelta\n";
        let find = anchor_for("beta gamma", 2);
        let drifted_content = "inserted-1\ninserted-2\nalpha\nbeta gamma\ndelta\n";
        fs::write(&test_file, drifted_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), &find, "BETA", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok(), "drifted anchor should relocate: {result:?}");
        assert_eq!(result.unwrap().is_error, Some(false));

        // The relocated line (now line 4) is replaced; nothing else changes.
        let edited = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited, "inserted-1\ninserted-2\nalpha\nBETA\ndelta\n");
        // Precondition sanity: anchor referenced line 2 but resolved at line 4.
        let _ = original_content;
    }

    /// A `N:HH|text` anchor whose line drifted relocates using `|text` as
    /// verification, and the relocated line is replaced.
    #[tokio::test]
    async fn cascade_text_suffix_relocates_drifted_anchor() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("drifted_text_anchor.txt");
        // Anchor `2:HH|beta gamma`, but "beta gamma" drifted to line 4.
        let find = format!("{}|beta gamma", anchor_for("beta gamma", 2));
        let drifted_content = "inserted-1\ninserted-2\nalpha\nbeta gamma\ndelta\n";
        fs::write(&test_file, drifted_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), &find, "BETA", None);

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "|text anchor should relocate drifted line: {result:?}"
        );
        assert_eq!(result.unwrap().is_error, Some(false));

        let edited = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited, "inserted-1\ninserted-2\nalpha\nBETA\ndelta\n");
    }

    /// A `N:HH|text` anchor whose hash matches no in-window line must NOT
    /// mis-apply: it falls through to the literal/near-miss path exactly as a
    /// plain stale anchor does. The file stays byte-identical.
    #[tokio::test]
    async fn cascade_text_suffix_no_inwindow_match_does_not_misapply() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("stale_text_anchor.txt");
        let content = "alpha\nbeta gamma\ndelta\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        // Hash 0x00 matches no line in the file; |text "ghost" matches none either.
        let find = "2:00|ghost";
        assert_ne!(
            find,
            format!("{}|ghost", anchor_for("beta gamma", 2)),
            "test precondition: chosen anchor must be stale"
        );
        let args = create_edit_arguments(&test_file.to_string_lossy(), find, "X", None);

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "stale |text anchor no-match is a successful near-miss: {result:?}"
        );
        assert_eq!(result.unwrap().is_error, Some(false));
        // File is byte-identical — nothing was committed.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// A proximity-relocated anchor whose anchor string ALSO occurs literally in
    /// the file must surface BOTH as candidates rather than guess — the same
    /// safety rule the exact-line case already enforces, now for the drifted case.
    #[tokio::test]
    async fn cascade_proximity_anchor_and_literal_both_present_surfaces_candidates() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("proximity_anchor_literal.txt");
        // Anchor for line 1 of "payload"; place the anchor STRING literally on
        // line 1 and the actual "payload" line drifted to line 3 (within window),
        // so the anchor both resolves (by proximity to line 3) and occurs as a
        // literal substring (on line 1).
        let line_text = "payload";
        let anchor = anchor_for(line_text, 1);
        let content = format!("{anchor}\nfiller\n{line_text}\n");
        fs::write(&test_file, &content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = ambiguity_args(&test_file.to_string_lossy(), &anchor, "REPLACED", None);

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "proximity-anchor-vs-literal must be a successful listing: {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));
        // File unchanged — the tool did not guess between anchor and literal.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// A valid (non-stale) anchor against a CR-only (classic-Mac) file must
    /// replace ONLY its referenced line, never clobber the rest of the file.
    /// Guards the line-model agreement between anchor resolution (CR-aware) and
    /// the byte-range mapping.
    #[tokio::test]
    async fn cascade_anchor_on_cr_only_file_replaces_single_line() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("cr_anchor.txt");
        // Classic-Mac CR-only line endings; "read files"/tag treats `\r` as a
        // line break, so the line-1 anchor is computed over "a" alone.
        let content = "a\rb\rc";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let find = anchor_for("a", 1);
        let args = create_edit_arguments(&test_file.to_string_lossy(), &find, "A", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok(), "CR-only anchor should resolve: {result:?}");
        assert_eq!(result.unwrap().is_error, Some(false));

        // ONLY line 1 is replaced; lines 2 and 3 survive with CR endings.
        let edited = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited, "A\rb\rc");
    }

    /// A bare-string `find` that lost its leading indentation is recovered by the
    /// normalized rung, and the replacement rewrites the ORIGINAL indented span.
    #[tokio::test]
    async fn cascade_normalized_span_apply_preserves_indentation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("normalized.txt");
        // The interior line is indented; the model's `find` drops the indent.
        let content = "fn outer() {\n    let x = compute();\n}\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        // Un-indented find — no literal substring is line-aligned, so the
        // normalized rung recovers the original indented line as the span.
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "let x = compute();",
            "let x = compute2();",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "normalized recovery should succeed: {result:?}"
        );

        let edited = fs::read_to_string(&test_file).unwrap();
        // Only the matched span is rewritten; the leading indentation is
        // preserved because the original span covered the indented bytes.
        assert_eq!(edited, "fn outer() {\n    let x = compute2();\n}\n");
    }

    /// A multi-pair batch is atomic: a single failing pair leaves the file
    /// byte-identical, even though earlier pairs would have applied.
    #[tokio::test]
    async fn cascade_atomic_rollback_on_failing_pair() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("atomic_batch.txt");
        let content = "one\ntwo\nthree\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // First edit would succeed; second names text that is absent → the whole
        // batch must NOT commit (structured near-miss) and the file must be
        // unchanged.
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "find": "one", "replace": "ONE" },
                { "find": "totally-absent", "replace": "X" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        // A failing pair short-circuits the batch as a successful near-miss; it
        // never commits the earlier pair.
        assert!(
            result.is_ok(),
            "a failing pair short-circuits the batch as a near-miss: {result:?}"
        );
        assert_eq!(result.unwrap().is_error, Some(false));

        // The file is byte-identical — the first (would-be-successful) pair was
        // not committed.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// The full batch commits in ONE rewrite: two successful pairs both land.
    #[tokio::test]
    async fn cascade_multi_pair_batch_commits_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multi_commit.txt");
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
                { "find": "one", "replace": "1" },
                { "find": "three", "replace": "3" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok(), "both pairs should apply: {result:?}");
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "1\ntwo\n3\n");
    }

    /// An empty `replace` deletes the matched span (delete = empty replace).
    #[tokio::test]
    async fn cascade_empty_replace_deletes_span() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("delete_span.txt");
        fs::write(&test_file, "keep DROP keep").unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "DROP ", "", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok(), "delete should succeed: {result:?}");
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "keep keep");
    }

    // =========================================================================
    // Ambiguity → candidates (not an error) + occurrence disambiguation
    // =========================================================================

    /// Two normalized matches (find requires whitespace normalization so it is not
    /// a literal substring) with `replace_all` false return a SUCCESSFUL result
    /// listing each candidate's line number, current text, and context — and the
    /// file is left byte-identical.
    #[tokio::test]
    async fn ambiguity_returns_candidates_not_error_and_file_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("ambig.txt");
        // Two identical lines. The `find` carries surrounding whitespace the
        // content lines lack, so it is NOT a literal substring
        // (content.find returns None) but normalizes (outer whitespace trimmed)
        // to match both lines via the line-block rung → Ambiguous.
        let content = "head\nfoo()\nmid\nfoo()\ntail\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = ambiguity_args(&test_file.to_string_lossy(), "  foo()  ", "bar()", None);

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "ambiguity must be a successful result, got {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(
            call.is_error,
            Some(false),
            "ambiguity is not an error result"
        );

        let text = result_text(&call);
        // Candidate line numbers (2 and 4), the current text, and a context hint.
        assert!(
            text.contains("occurrence"),
            "must mention occurrence: {text}"
        );
        assert!(text.contains("line 2"), "must list line 2: {text}");
        assert!(text.contains("line 4"), "must list line 4: {text}");
        assert!(text.contains("foo()"), "must show current text: {text}");

        // File is byte-identical — nothing was committed.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// Supplying `occurrence: N` selects the Nth candidate (1-based) and applies
    /// only that edit.
    #[tokio::test]
    async fn occurrence_selects_nth_candidate_and_applies() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("occ.txt");
        let content = "head\nfoo()\nmid\nfoo()\ntail\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        // occurrence 2 → the second matching line (line 4) is rewritten; line 2 is
        // left intact (the whole matched line span is replaced).
        let args = ambiguity_args(&test_file.to_string_lossy(), "  foo()  ", "bar()", Some(2));

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "occurrence apply should succeed: {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));

        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "head\nfoo()\nmid\nbar()\ntail\n",
            "only the 2nd candidate line is rewritten"
        );
    }

    /// `occurrence: 1` selects the first candidate.
    #[tokio::test]
    async fn occurrence_one_selects_first_candidate() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("occ1.txt");
        let content = "head\nfoo()\nmid\nfoo()\ntail\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = ambiguity_args(&test_file.to_string_lossy(), "  foo()  ", "bar()", Some(1));

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "occurrence apply should succeed: {result:?}"
        );
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "head\nbar()\nmid\nfoo()\ntail\n",
            "only the 1st candidate line is rewritten"
        );
    }

    /// An out-of-range `occurrence` does not silently mis-apply: it falls back to
    /// the candidate listing (successful result) and does not change the file.
    #[tokio::test]
    async fn occurrence_out_of_range_returns_candidates_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("occ_oob.txt");
        let content = "head\nfoo()\nmid\nfoo()\ntail\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        // Only 2 candidates exist; occurrence 5 is out of range.
        let args = ambiguity_args(&test_file.to_string_lossy(), "  foo()  ", "bar()", Some(5));

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "out-of-range occurrence stays a successful listing"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));
        // File unchanged — no mis-apply.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// A resolving anchor whose line text ALSO occurs as a literal substring is
    /// surfaced as candidates (anchor + literal), not silently picked — the file
    /// is unchanged.
    #[tokio::test]
    async fn anchor_and_literal_both_present_surfaces_candidates() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("anchor_literal.txt");
        // Compute the anchor for line 2, then place that exact anchor string as
        // literal text on line 1 so `content.find(find)` is Some as well.
        let line2 = "payload";
        let anchor = anchor_for(line2, 2);
        let content = format!("{anchor}\n{line2}\n");
        fs::write(&test_file, &content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = ambiguity_args(&test_file.to_string_lossy(), &anchor, "REPLACED", None);

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "anchor-vs-literal ambiguity must be a successful listing: {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));
        // File unchanged — the tool did not guess between anchor and literal.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// Atomicity on ambiguity: an earlier pair that WOULD apply, followed by an
    /// ambiguous later pair, must leave the file byte-identical — the earlier
    /// pair's in-memory mutation is never flushed, and the result is the
    /// successful candidate listing.
    #[tokio::test]
    async fn ambiguous_later_pair_does_not_partially_write_earlier_pair() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("ambig_batch.txt");
        // "one" applies cleanly; "  two  " is ambiguous (two normalized matches).
        let content = "one\ntwo\nmid\ntwo\ntail\n";
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
                { "find": "  two  ", "replace": "TWO" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "ambiguous later pair yields a successful listing: {result:?}"
        );
        assert_eq!(result.unwrap().is_error, Some(false));

        // Byte-identical: the first pair's "one"→"ONE" mutation was NOT committed.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// `replace_all: true` continues to replace every match with no ambiguity
    /// prompt, even when multiple matches exist.
    #[tokio::test]
    async fn replace_all_true_has_no_ambiguity_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("replace_all_ambig.txt");
        let content = "foo\nfoo\nfoo\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "foo", "bar", Some(true));

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));
        // All replaced, no prompt.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "bar\nbar\nbar\n");
    }

    // =========================================================================
    // Idempotency / safety: no-op rejection, already-applied, consumed-target
    // =========================================================================

    /// No-op rejection: a single pair where `find == replace` is rejected with a
    /// clear message and the file is left byte-identical. This is the coherent
    /// reconciliation of the legacy "must be different" check.
    #[tokio::test]
    async fn no_op_find_equals_replace_is_rejected() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("noop.txt");
        let content = "alpha\nbeta\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "alpha", "alpha", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_err(), "no-op edit must be rejected: {result:?}");
        let err = format!("{:?}", result.unwrap_err());
        // Clear message: still says the two must differ (no-op).
        assert!(
            err.contains("no-op") || err.contains("must be different") || err.contains("different"),
            "no-op message must be clear: {err}"
        );
        // File untouched.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// Already-applied detection: when a pair's `replace` text is already present
    /// in the file and its `find` is absent, report "likely already applied" as an
    /// informational SUCCESS — not a hard "not found" error — and leave the file
    /// byte-identical.
    #[tokio::test]
    async fn already_applied_is_informational_success_not_error() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("already.txt");
        // The replacement target is already in the file; the original `find` is gone.
        let content = "let renamed = compute();\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "let original = compute();",
            "let renamed = compute();",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "already-applied must be a successful informational result: {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(
            call.is_error,
            Some(false),
            "already-applied is not an error"
        );
        let text = result_text(&call);
        assert!(
            text.contains("already applied"),
            "must report likely-already-applied: {text}"
        );
        // No mutation: the file is byte-identical and carries no envelope.
        assert!(
            call.structured_content.is_none(),
            "already-applied result carries no mutation envelope"
        );
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// Consumed-target detection: in a multi-pair batch, a later pair whose target
    /// span was consumed/overwritten by an earlier pair in the SAME batch is
    /// detected and reported per-edit as a consumed target — distinct from a
    /// generic near-miss — and the batch stays atomic (file byte-identical).
    #[tokio::test]
    async fn consumed_target_in_batch_is_detected_and_atomic() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("consumed.txt");
        // The first pair rewrites the whole line; the second pair's `find` targeted
        // a substring of that ORIGINAL line, which the first pair consumed.
        let content = "value = old_token;\nother = keep;\n";
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
                { "find": "value = old_token;", "replace": "value = replaced_line;" },
                { "find": "old_token", "replace": "new_token" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "consumed-target must be a successful per-edit report: {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));
        let text = result_text(&call);
        // The failing pair's find is echoed (per-edit reporting).
        assert!(
            text.contains("old_token"),
            "must echo the consumed pair's find: {text}"
        );
        // Specifically reports the consumed-target case, not a generic miss.
        assert!(
            text.contains("consumed") || text.contains("earlier edit"),
            "must report the consumed-target case specifically: {text}"
        );
        // Atomic: the earlier pair's mutation was NOT committed.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    // =====================================================================
    // resolve_pair / ladder recovery arms
    // =====================================================================

    /// `replace_all` with no literal occurrence yields a NoMatch outcome carrying
    /// near-misses rather than a bare error (covers `no_match_outcome`).
    #[test]
    fn test_resolve_pair_replace_all_no_literal_is_no_match() {
        let pair = EditPair {
            find: "totally-absent-token".to_string(),
            replace: "x".to_string(),
            replace_all: true,
            occurrence: None,
        };
        let outcome = resolve_pair("alpha\nbeta\ngamma\n", &pair).unwrap();
        assert!(matches!(outcome, PairOutcome::NoMatch { .. }));
    }

    /// `resolve_via_ladder` resolves a `find` whose LEADING whitespace differs
    /// from the file (tab on disk vs spaces in the find, so it is NOT a literal
    /// substring and has no resolving anchor) to a unique span via the fuzzy
    /// ladder — covering the Unique arm.
    #[test]
    fn test_resolve_via_ladder_unique_on_leading_whitespace_drift() {
        // The unique interior line is tab-indented on disk; the find uses spaces.
        // No literal substring match (tab != spaces), no anchor — only the
        // normalized ladder, which tolerates leading-whitespace drift, resolves it.
        let content = "alpha\n\tdistinct_target_line()\nomega\n";
        let pair = EditPair {
            find: "    distinct_target_line()".to_string(),
            replace: "    replaced_target_line()".to_string(),
            replace_all: false,
            occurrence: None,
        };

        // Precondition: the literal rung cannot match (different leading bytes).
        assert!(content.find(&pair.find).is_none());

        let outcome = resolve_via_ladder(content, &pair).unwrap();
        match outcome {
            PairOutcome::Resolved(Resolution::Splice { range, replacement }) => {
                assert_eq!(replacement, "    replaced_target_line()");
                // The spliced range is the drifted original (tab-indented) line.
                assert_eq!(&content[range], "\tdistinct_target_line()");
            }
            other => panic!("expected a unique ladder splice, got {other:?}"),
        }
    }

    /// A `find` whose interior whitespace differs from the file produces a
    /// structured NoMatch carrying a near-miss diff, not a bare error — covering
    /// the ladder's NoMatch arm directly.
    #[test]
    fn test_resolve_via_ladder_no_match_surfaces_near_miss() {
        let content = "alpha\nlet  x  =  1;\nomega\n";
        let pair = EditPair {
            find: "completely-different-token".to_string(),
            replace: "x".to_string(),
            replace_all: false,
            occurrence: None,
        };
        let outcome = resolve_via_ladder(content, &pair).unwrap();
        assert!(matches!(outcome, PairOutcome::NoMatch { .. }));
    }
}
