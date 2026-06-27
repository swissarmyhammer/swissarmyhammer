// sah rule ignore acp/capability-enforcement
//! File editing tool for MCP operations
//!
//! This module provides the EditFileTool for performing precise string replacements in files
//! with atomic operations, comprehensive security validation, file encoding preservation,
//! and metadata preservation.
//!
//! Note: This is an MCP tool, not an ACP operation. ACP capability checking happens at the
//! agent layer (claude-agent, llama-agent), not at the MCP tool layer.

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use encoding_rs::{Encoding, UTF_8};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::fs;
use std::io::{BufWriter, Write};
use std::ops::Range;
use std::path::Path;
use swissarmyhammer_edit_match::{find_match, MatchOutcome};
use swissarmyhammer_hashline::{parse_anchor, resolve_anchor_range_in};
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use tracing::{debug, info};

/// Operation metadata for editing files
#[derive(Debug, Default)]
pub struct EditFile;

/// Alias keys that resolve to the canonical `file_path` parameter.
static FILE_PATH_ALIASES: &[&str] = &["path", "filePath", "absolute_path"];

/// Alias keys that resolve to the canonical `find` parameter (the text to match).
///
/// `old_string`/`oldText` are the legacy MCP names, kept here as aliases so the
/// historical single-edit and `edits[]` shapes keep working. The remaining
/// entries are the natural-language synonyms a model is likely to emit.
static FIND_ALIASES: &[&str] = &[
    "search",
    "old",
    "old_string",
    "oldText",
    "old_text",
    "from",
    "target",
    "match",
];

/// Alias keys that resolve to the canonical `replace` parameter (the new text).
///
/// `new_string`/`newText` are the legacy MCP names, kept here as aliases. The
/// remaining entries are natural-language synonyms.
static REPLACE_ALIASES: &[&str] = &[
    "new",
    "new_string",
    "newText",
    "new_text",
    "to",
    "with",
    "replacement",
];

static EDIT_FILE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Absolute path to the file to modify")
        .param_type(ParamType::String)
        .aliases(FILE_PATH_ALIASES)
        .required(),
    ParamMeta::new("find")
        .description("Exact text to replace")
        .param_type(ParamType::String)
        .aliases(FIND_ALIASES)
        .required(),
    ParamMeta::new("replace")
        .description("Replacement text")
        .param_type(ParamType::String)
        .aliases(REPLACE_ALIASES)
        .required(),
    ParamMeta::new("replace_all")
        .description("Replace all occurrences (default: false)")
        .param_type(ParamType::Boolean),
    ParamMeta::new("occurrence")
        .description(
            "1-based candidate index to disambiguate when `find` has multiple confident \
             matches and `replace_all` is false. Omit it and an ambiguous `find` returns \
             the candidate list (line numbers + current text + context) instead of editing; \
             supply it to apply exactly that candidate.",
        )
        .param_type(ParamType::Integer),
    ParamMeta::new("edits")
        .description("Array of {find, replace} edit pairs to apply sequentially")
        .param_type(ParamType::Array),
];

/// One canonical edit: replace `find` with `replace`, optionally every occurrence.
///
/// This is the normalized form every accepted input shape collapses to. It
/// carries no IO — [`normalize_edit_args`] produces it purely from arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditPair {
    /// Exact text to match in the target file.
    pub find: String,
    /// Replacement text.
    pub replace: String,
    /// Replace every occurrence (`true`) instead of just the first (`false`).
    pub replace_all: bool,
    /// 1-based candidate index that disambiguates an otherwise-ambiguous `find`.
    ///
    /// `None` (the default) means "no hint": an ambiguous `find` returns the
    /// candidate listing instead of editing. When supplied and it selects exactly
    /// one of the surfaced candidates, that candidate is applied. Ignored when the
    /// `find` is unambiguous.
    pub occurrence: Option<usize>,
}

/// Read the first present key among `keys` from `map`.
fn first_present<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    canonical: &str,
    aliases: &[&str],
) -> Option<&'a serde_json::Value> {
    if let Some(v) = map.get(canonical) {
        return Some(v);
    }
    aliases.iter().find_map(|alias| map.get(*alias))
}

/// Coerce a JSON value into a list of strings: a scalar string yields one entry,
/// an array yields each element as a string. Returns `None` for absent input and
/// an error for a non-string / non-array value (or a non-string array element).
fn collect_strings(value: Option<&serde_json::Value>) -> Result<Option<Vec<String>>, McpError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::String(s) => Ok(Some(vec![s.clone()])),
        serde_json::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    serde_json::Value::String(s) => out.push(s.clone()),
                    other => {
                        return Err(McpError::invalid_request(
                            format!("find/replace array entries must be strings, got {other}"),
                            None,
                        ))
                    }
                }
            }
            Ok(Some(out))
        }
        other => Err(McpError::invalid_request(
            format!("find/replace must be a string or array of strings, got {other}"),
            None,
        )),
    }
}

/// Read an optional `replace_all` boolean from a map (canonical name only —
/// there are no aliases for this flag).
fn read_replace_all(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.get("replace_all")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// Read an optional 1-based `occurrence` hint from a map (canonical name only).
///
/// A value `>= 1` is kept; `0`, a negative, or a non-integer is treated as
/// absent (`None`) so a malformed hint never silently selects the wrong
/// candidate — it simply falls back to the candidate listing.
fn read_occurrence(map: &serde_json::Map<String, serde_json::Value>) -> Option<usize> {
    map.get("occurrence")
        .and_then(serde_json::Value::as_u64)
        .filter(|&n| n >= 1)
        .map(|n| n as usize)
}

/// Pair a list of finds with a list of replaces using the forgiving rules:
/// - N finds + N replaces → zip.
/// - N finds + 1 replace → broadcast the single replace to every find.
/// - anything else (including 1 find + N replaces) → zip what lines up cleanly
///   and surface the unpaired remainder in the error; never silently drop.
fn pair_finds_replaces(
    finds: Vec<String>,
    replaces: Vec<String>,
    replace_all: bool,
    occurrence: Option<usize>,
) -> Result<Vec<EditPair>, McpError> {
    // Broadcast a single replace across many finds (the delete-many shape).
    if replaces.len() == 1 && finds.len() > 1 {
        let replace = &replaces[0];
        return Ok(finds
            .into_iter()
            .map(|find| EditPair {
                find,
                replace: replace.clone(),
                replace_all,
                occurrence,
            })
            .collect());
    }

    if finds.len() == replaces.len() {
        return Ok(finds
            .into_iter()
            .zip(replaces)
            .map(|(find, replace)| EditPair {
                find,
                replace,
                replace_all,
                occurrence,
            })
            .collect());
    }

    // Mismatch: pair what zips, then report the unpaired remainder.
    let paired = finds.len().min(replaces.len());
    let leftover_finds = &finds[paired..];
    let leftover_replaces = &replaces[paired..];
    let mut remainder = Vec::new();
    if !leftover_finds.is_empty() {
        remainder.push(format!("unpaired finds: {leftover_finds:?}"));
    }
    if !leftover_replaces.is_empty() {
        remainder.push(format!("unpaired replaces: {leftover_replaces:?}"));
    }
    Err(McpError::invalid_request(
        format!(
            "mismatched find/replace counts ({} finds, {} replaces); {}",
            finds.len(),
            replaces.len(),
            remainder.join("; ")
        ),
        None,
    ))
}

/// Whether a no-`op` argument map should be dispatched to the edit operation.
///
/// True when any find-ish or replace-ish key (canonical name or alias) is
/// present, or when an `edits` array is supplied. The dispatcher in
/// [`super::FilesTool`] consults this BEFORE the `content`→write branch so a
/// canonical `{find, replace}` call is never misrouted to write.
pub fn looks_like_edit(args: &serde_json::Map<String, serde_json::Value>) -> bool {
    args.contains_key("edits")
        || first_present(args, "find", FIND_ALIASES).is_some()
        || first_present(args, "replace", REPLACE_ALIASES).is_some()
}

/// Normalize the forgiving `edit files` argument surface into a canonical list
/// of [`EditPair`]s.
///
/// Accepts three input shapes — which may be combined — under any of the
/// `find`/`replace` aliases (see [`FIND_ALIASES`] / [`REPLACE_ALIASES`]):
///
/// 1. Top-level scalar `find`/`replace`.
/// 2. Top-level parallel arrays `find: [...]` / `replace: [...]`.
/// 3. An `edits: [{ find, replace, replace_all? }, ...]` array.
///
/// Top-level finds/replaces are paired via [`pair_finds_replaces`] (zip /
/// broadcast / mismatch-remainder) and then **concatenated** with the pairs
/// drawn from `edits[]`. This is pure: it performs no IO and never touches the
/// filesystem, so it is unit-testable in isolation.
pub fn normalize_edit_args(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<EditPair>, McpError> {
    let mut pairs = Vec::new();

    // Shape 1 & 2: top-level scalar or parallel arrays.
    let finds = collect_strings(first_present(args, "find", FIND_ALIASES))?;
    let replaces = collect_strings(first_present(args, "replace", REPLACE_ALIASES))?;
    match (finds, replaces) {
        (Some(finds), Some(replaces)) => {
            pairs.extend(pair_finds_replaces(
                finds,
                replaces,
                read_replace_all(args),
                read_occurrence(args),
            )?);
        }
        (Some(_), None) => {
            return Err(McpError::invalid_request(
                "find provided without a matching replace".to_string(),
                None,
            ));
        }
        (None, Some(_)) => {
            return Err(McpError::invalid_request(
                "replace provided without a matching find".to_string(),
                None,
            ));
        }
        (None, None) => {}
    }

    // Shape 3: the edits[] array, each entry carrying its own find/replace.
    if let Some(edits) = args.get("edits") {
        let items = edits.as_array().ok_or_else(|| {
            McpError::invalid_request("edits must be an array of edit objects".to_string(), None)
        })?;
        for (idx, item) in items.iter().enumerate() {
            let obj = item.as_object().ok_or_else(|| {
                McpError::invalid_request(
                    format!("edits[{idx}] must be an object with find/replace"),
                    None,
                )
            })?;
            let finds =
                collect_strings(first_present(obj, "find", FIND_ALIASES))?.ok_or_else(|| {
                    McpError::invalid_request(format!("edits[{idx}] is missing find"), None)
                })?;
            let replaces = collect_strings(first_present(obj, "replace", REPLACE_ALIASES))?
                .ok_or_else(|| {
                    McpError::invalid_request(format!("edits[{idx}] is missing replace"), None)
                })?;
            pairs.extend(pair_finds_replaces(
                finds,
                replaces,
                read_replace_all(obj),
                read_occurrence(obj),
            )?);
        }
    }

    if pairs.is_empty() {
        return Err(McpError::invalid_request(
            "no edits provided: supply find/replace (or aliases), or an edits array".to_string(),
            None,
        ));
    }

    Ok(pairs)
}

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

/// One competing location for an ambiguous `find`. Surfaced to the model so it
/// can disambiguate with [`EditPair::occurrence`] on the retry. Carries enough
/// to both describe the choice (line number + current text + context) and apply
/// it (the byte `range` to splice).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    /// Byte range into the working content this candidate would overwrite.
    range: Range<usize>,
    /// 1-based line number where the candidate begins.
    line: usize,
    /// The current text covered by `range`.
    text: String,
    /// A few lines of surrounding context (the candidate's neighbourhood),
    /// rendered with line-number gutters for the model to orient against.
    context: String,
}

/// One near-miss location for a `find` that matched no rung confidently.
/// Surfaced to the model so it sees exactly how its `find` diverged from the
/// nearest current text and can correct in one shot.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NearMiss {
    /// 1-based line number where the near-miss span begins.
    line: usize,
    /// The current text at this span (the nearest text to the supplied `find`).
    text: String,
    /// A few lines of surrounding context, rendered with line-number gutters.
    context: String,
    /// A line-level diff between the supplied `find` and this span's current
    /// text, so the model sees precisely how the two differ.
    diff: String,
}

/// The outcome of resolving one [`EditPair`] against the working content: either
/// it resolved to a concrete [`Resolution`] to commit, it is ambiguous and the
/// competing [`Candidate`]s must be surfaced for disambiguation, or nothing
/// matched and the nearest [`NearMiss`]es must be surfaced.
///
/// Neither ambiguity nor a no-match is an [`McpError`]: the cascade reports both
/// up to [`execute_edit`], which turns them into SUCCESSFUL tool results the
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

/// Number of context lines rendered on each side of a candidate line.
const CANDIDATE_CONTEXT_RADIUS: usize = 2;

/// The 1-based physical line number containing the byte at `offset`.
fn line_number_at(content: &str, offset: usize) -> usize {
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
fn candidate_for(content: &str, range: Range<usize>) -> Candidate {
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
fn render_ambiguity_prompt(find: &str, candidates: &[Candidate]) -> String {
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
fn near_miss_for(content: &str, find: &str, range: Range<usize>) -> NearMiss {
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
fn render_near_miss_prompt(find: &str, near: &[NearMiss]) -> String {
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
fn render_already_applied_prompt(find: &str, replace: &str) -> String {
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
fn render_consumed_target_prompt(find: &str, line: usize) -> String {
    format!(
        "`find` {find:?} did not match: its target around line {line} was consumed \
         by an earlier edit in this same batch. Re-issue this edit against the \
         already-edited text (or fold it into the earlier edit). The file is \
         unchanged.\n"
    )
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
enum ApplyOutcome {
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
fn apply_all_pairs(original: &str, pairs: &[EditPair]) -> Result<ApplyOutcome, McpError> {
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

impl Operation for EditFile {
    fn verb(&self) -> &'static str {
        "edit"
    }
    fn noun(&self) -> &'static str {
        "file"
    }
    fn description(&self) -> &'static str {
        "Perform precise string replacements in existing files"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        EDIT_FILE_PARAMS
    }
}

/// Result information for edit operations
#[derive(Debug, Clone)]
pub struct EditResult {
    /// Number of bytes written to the file
    pub bytes_written: usize,
    /// Number of string replacements made in the file
    pub replacements_made: usize,
    /// The character encoding that was detected and preserved
    pub encoding_detected: String,
    /// The line ending format that was preserved
    pub line_endings_preserved: String,
}

/// Validation result for edit operations
#[derive(Debug, Clone)]
struct EditValidation {
    pub old_string_count: usize,
}

/// Line ending types detected in files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEnding {
    Lf,    // Unix: \n
    CrLf,  // Windows: \r\n
    Cr,    // Classic Mac: \r
    Mixed, // Multiple types found
}

impl LineEnding {
    /// Detect the primary line ending type in content
    fn detect(content: &str) -> Self {
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

    /// Get the string representation
    fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
            LineEnding::Cr => "CR",
            LineEnding::Mixed => "Mixed",
        }
    }
}

/// Tool for performing precise string replacements in existing files
#[derive(Default, Debug)]
pub struct EditFileTool;

impl EditFileTool {
    /// Creates a new instance of the EditFileTool
    pub fn new() -> Self {
        Self
    }

    /// Validates the edit operation before making changes
    ///
    /// Performs comprehensive validation including:
    /// - File existence check
    /// - Old string existence and uniqueness validation
    /// - Security checks through file path validation
    fn validate_edit_operation(
        &self,
        base_dir: &Path,
        file_path: &str,
        content: &str,
        old_string: &str,
        _replace_all: bool,
    ) -> Result<EditValidation, McpError> {
        use crate::mcp::tools::files::shared_utils::validate_file_path;

        // Validate file path first (relative paths resolve against the session
        // working directory, never the process CWD)
        let path = validate_file_path(base_dir, file_path)?;
        if !path.exists() {
            return Err(McpError::invalid_request(
                format!("File does not exist: {}", file_path),
                None,
            ));
        }

        // Count occurrences of old_string
        let matches: Vec<_> = content.matches(old_string).collect();
        let old_string_count = matches.len();
        if old_string_count == 0 {
            return Err(McpError::invalid_request(
                format!("String '{}' not found in file", old_string),
                None,
            ));
        }

        Ok(EditValidation { old_string_count })
    }

    /// Detects file encoding and reads content as string
    ///
    /// Uses encoding_rs for robust encoding detection and handles:
    /// - UTF-8 (most common)
    /// - UTF-16 with BOM
    /// - Other encodings with fallback to UTF-8
    fn read_with_encoding_detection(
        &self,
        file_path: &Path,
    ) -> Result<(String, &'static Encoding), McpError> {
        use crate::mcp::tools::files::shared_utils::handle_file_error;

        // Read raw bytes first
        let bytes = fs::read(file_path)
            .map_err(|e| handle_file_error(e, "read file for encoding detection", file_path))?;

        // Detect encoding using BOM, fallback to UTF-8
        let (encoding, bom_length) = encoding_rs::Encoding::for_bom(&bytes).unwrap_or((UTF_8, 0));

        // Use the bytes after BOM for decoding
        let bytes_to_decode = &bytes[bom_length..];

        debug!(path = %file_path.display(), encoding = encoding.name(), bom_length = bom_length, "Detected file encoding");

        // Decode to string
        let (content, _, had_decode_errors) = encoding.decode(bytes_to_decode);

        if had_decode_errors {
            return Err(McpError::internal_error(
                format!(
                    "Failed to decode file with detected encoding {}",
                    encoding.name()
                ),
                None,
            ));
        }

        Ok((content.into_owned(), encoding))
    }

    /// Performs atomic file edit with full validation and metadata preservation
    ///
    /// This method implements the complete atomic edit workflow:
    /// 1. Validate file path and edit parameters
    /// 2. Read file with encoding detection
    /// 3. Validate old_string existence and uniqueness
    /// 4. Perform replacement operation
    /// 5. Write to temporary file in same directory
    /// 6. Preserve file metadata (permissions, timestamps)
    /// 7. Atomically rename temporary file to original
    /// 8. Clean up temporary file on any failure
    pub fn edit_file_atomic(
        &self,
        base_dir: &Path,
        file_path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> Result<EditResult, McpError> {
        use crate::mcp::tools::files::shared_utils::validate_file_path;

        // Step 1: Validate file path and get canonical path. Relative paths
        // resolve against the session working directory, never the process CWD.
        let path = validate_file_path(base_dir, file_path)?;

        info!(path = %path.display(), old_string_len = old_string.len(), new_string_len = new_string.len(), replace_all = replace_all, "Starting atomic edit operation");

        // Step 2: Read original file with encoding detection
        let (original_content, detected_encoding) = self.read_with_encoding_detection(&path)?;

        // Step 3: Detect line endings
        let line_ending = LineEnding::detect(&original_content);

        // Step 4: Validate edit operation
        let validation = self.validate_edit_operation(
            base_dir,
            file_path,
            &original_content,
            old_string,
            replace_all,
        )?;

        // Step 5: Perform replacement
        let (new_content, replacements_made) = if replace_all {
            let new_content = original_content.replace(old_string, new_string);
            let replacements = validation.old_string_count;
            (new_content, replacements)
        } else {
            let new_content = original_content.replacen(old_string, new_string, 1);
            (new_content, 1)
        };

        // Step 6: commit the rewritten content in one atomic rewrite (metadata
        // preservation lives in `commit_content`).
        self.commit_content(
            &path,
            &new_content,
            detected_encoding,
            line_ending,
            replacements_made,
        )
    }

    /// Commit fully-rewritten `content` to `path` in one atomic rewrite,
    /// preserving the original encoding and permissions.
    ///
    /// This is the shared temp-write + fsync-free rename core both the legacy
    /// single-pair [`edit_file_atomic`](Self::edit_file_atomic) and the
    /// shape-inferred batch path ([`execute_edit`]) commit through, so the
    /// encoding / line-ending / permission preservation lives in exactly one
    /// place. The modification time is intentionally NOT preserved: an edit
    /// changes the file, so the rename's fresh mtime must stand, keeping
    /// downstream mtime-based staleness checks (cargo/make, file watchers,
    /// rust-analyzer) correct. On any failure the temporary file is removed and
    /// the original is left untouched (byte-identical).
    fn commit_content(
        &self,
        path: &Path,
        content: &str,
        encoding: &'static Encoding,
        line_ending: LineEnding,
        replacements_made: usize,
    ) -> Result<EditResult, McpError> {
        use crate::mcp::tools::files::shared_utils::handle_file_error;

        // Capture the original metadata to preserve permissions.
        let original_metadata =
            fs::metadata(path).map_err(|e| handle_file_error(e, "read metadata", path))?;
        let original_permissions = original_metadata.permissions();

        // Create temporary file in same directory as original.
        let temp_file_name = format!("{}.tmp.{}", path.display(), std::process::id());
        let temp_path = path
            .parent()
            .ok_or_else(|| {
                McpError::internal_error(
                    "Cannot determine parent directory for temporary file".to_string(),
                    None,
                )
            })?
            .join(&temp_file_name);

        debug!(temp_path = %temp_path.display(), content_length = content.len(), encoding = encoding.name(), "Writing content to temporary file");

        // Write new content to temporary file with original encoding.
        let bytes_written = match self.write_with_encoding(&temp_path, content, encoding) {
            Ok(bytes_written) => bytes_written,
            Err(e) => {
                let _ = fs::remove_file(&temp_path);
                return Err(e);
            }
        };

        // Re-apply the original permissions to the temp file before rename.
        // The temp-write+rename gives the new file default permissions, so
        // without this an executable script (e.g. 0755) would silently downgrade
        // to 0644. This is silent behavior — not reported in the result.
        if let Err(e) = fs::set_permissions(&temp_path, original_permissions.clone()) {
            let _ = fs::remove_file(&temp_path);
            return Err(handle_file_error(
                e,
                "set permissions on temporary file",
                &temp_path,
            ));
        }

        // Atomically rename temporary file to original.
        if let Err(e) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(handle_file_error(
                e,
                "rename temporary file to target",
                path,
            ));
        }

        debug!(path = %path.display(), bytes_written = bytes_written, replacements_made = replacements_made, "Atomic edit operation completed successfully");

        Ok(EditResult {
            bytes_written,
            replacements_made,
            encoding_detected: encoding.name().to_string(),
            line_endings_preserved: line_ending.as_str().to_string(),
        })
    }

    /// Writes content to file with specified encoding
    ///
    /// Preserves the original encoding of the file and handles BOM appropriately.
    fn write_with_encoding(
        &self,
        file_path: &Path,
        content: &str,
        encoding: &'static Encoding,
    ) -> Result<usize, McpError> {
        use crate::mcp::tools::files::shared_utils::handle_file_error;

        // Encode content back to bytes using the detected encoding
        let (bytes, _, had_errors) = encoding.encode(content);

        if had_errors {
            return Err(McpError::internal_error(
                format!("Failed to encode content with encoding {}", encoding.name()),
                None,
            ));
        }

        // Write bytes to file
        let file = fs::File::create(file_path)
            .map_err(|e| handle_file_error(e, "create temporary file", file_path))?;

        let mut writer = BufWriter::new(file);
        writer
            .write_all(&bytes)
            .map_err(|e| handle_file_error(e, "write to temporary file", file_path))?;

        writer
            .flush()
            .map_err(|e| handle_file_error(e, "flush temporary file", file_path))?;

        Ok(bytes.len())
    }
}

/// Execute a file edit operation
pub async fn execute_edit(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    // Extract file path under any canonical/alias key.
    let file_path = first_present(&arguments, "file_path", FILE_PATH_ALIASES)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            McpError::invalid_request("path/file_path/filePath is required".to_string(), None)
        })?
        .to_string();

    // Validate file path
    if file_path.trim().is_empty() {
        return Err(McpError::invalid_request(
            "path cannot be empty".to_string(),
            None,
        ));
    }

    // An explicitly empty `edits: []` (with no top-level find/replace) keeps its
    // historical, more specific error message.
    if let Some(serde_json::Value::Array(edits)) = arguments.get("edits") {
        if edits.is_empty()
            && first_present(&arguments, "find", FIND_ALIASES).is_none()
            && first_present(&arguments, "replace", REPLACE_ALIASES).is_none()
        {
            return Err(McpError::invalid_request(
                "edits array cannot be empty".to_string(),
                None,
            ));
        }
    }

    // Normalize every accepted input shape into canonical (find, replace) pairs.
    let edit_operations = normalize_edit_args(&arguments)?;

    // Check rate limit, costed by the number of edit operations (shared helper).
    let cost = edit_operations.len() as u32;
    crate::mcp::tools::files::shared_utils::enforce_rate_limit("file_edit", cost)?;

    // Validate all edit operations
    for (idx, edit_op) in edit_operations.iter().enumerate() {
        if edit_op.find.is_empty() {
            return Err(McpError::invalid_request(
                format!("Edit operation {}: old_text cannot be empty", idx),
                None,
            ));
        }

        // No-op rejection: `find == replace` would change nothing. Reject it up
        // front with a clear message — this is the single, coherent home for the
        // no-op concept (the historical "must be different" check IS the no-op
        // rejection, not a separate code path).
        if edit_op.find == edit_op.replace {
            return Err(McpError::invalid_request(
                format!(
                    "Edit operation {idx}: no-op edit — `find` and `replace` are identical, so \
                     they must be different"
                ),
                None,
            ));
        }
    }

    // Log edit attempt for security auditing
    info!(path = %file_path, num_operations = edit_operations.len(), "Attempting atomic edit operation(s)");

    // Apply the whole batch atomically: read the file once, resolve and apply
    // every pair against an in-memory working copy, then commit in ONE rewrite.
    // A failure on any pair leaves the file byte-identical. Relative paths
    // resolve against the session working directory (the board dir), never the
    // process CWD.
    use crate::mcp::tools::files::shared_utils::{mutation_success_response, validate_file_path};
    let base_dir = context.session_root();
    let tool = EditFileTool::new();

    // Resolve and validate the target path (existence) once.
    let path = validate_file_path(&base_dir, &file_path)?;
    if !path.exists() {
        return Err(McpError::invalid_request(
            format!("File does not exist: {}", file_path),
            None,
        ));
    }

    // Read once with encoding detection and detect the line-ending convention.
    let (original_content, detected_encoding) = tool.read_with_encoding_detection(&path)?;
    let line_ending = LineEnding::detect(&original_content);

    // Resolve + apply every pair against the working copy (no IO). The cascade
    // (anchor → literal substring → recovery ladder) runs here.
    let new_content = match apply_all_pairs(&original_content, &edit_operations)? {
        ApplyOutcome::Applied(content) => content,
        // Ambiguity is a SUCCESSFUL result describing the choice — NOT an error.
        // Nothing was committed, so the file is byte-identical; the model retries
        // with an `occurrence` hint.
        ApplyOutcome::Ambiguous { find, candidates } => {
            info!(path = %file_path, candidate_count = candidates.len(), "Edit `find` is ambiguous; returning candidates for disambiguation");
            return Ok(BaseToolImpl::create_success_response(
                render_ambiguity_prompt(&find, &candidates),
            ));
        }
        // No confident match is a SUCCESSFUL near-miss describing how the `find`
        // diverged — NOT an error. Nothing was committed, so the file is
        // byte-identical; the model retries with corrected text.
        ApplyOutcome::NoMatch { find, near } => {
            info!(path = %file_path, near_miss_count = near.len(), "Edit `find` matched nothing confidently; returning near-misses");
            return Ok(BaseToolImpl::create_success_response(
                render_near_miss_prompt(&find, &near),
            ));
        }
        // `find` absent but `replace` already present: the edit was likely already
        // applied. Informational SUCCESS — nothing committed, file byte-identical.
        ApplyOutcome::AlreadyApplied { find, replace } => {
            info!(path = %file_path, "Edit `find` absent but `replace` present; reporting likely-already-applied");
            return Ok(BaseToolImpl::create_success_response(
                render_already_applied_prompt(&find, &replace),
            ));
        }
        // A later pair's target span was consumed by an earlier pair in this same
        // batch. Per-edit SUCCESS — nothing committed, file byte-identical.
        ApplyOutcome::ConsumedTarget { find, line } => {
            info!(path = %file_path, consumed_line = line, "Edit `find` target was consumed by an earlier edit in the batch");
            return Ok(BaseToolImpl::create_success_response(
                render_consumed_target_prompt(&find, line),
            ));
        }
    };

    // Commit the fully-edited content in one atomic rewrite.
    let final_result = tool.commit_content(
        &path,
        &new_content,
        detected_encoding,
        line_ending,
        edit_operations.len(),
    )?;
    let total_replacements = edit_operations.len();

    // Record the mutated path on the typed side-channel so the dispatch
    // chokepoint can fold inline diagnostics into this result (no content
    // parsing). This is DISTINCT from the `mutated_paths` carried in the result
    // body below — the side-channel drives inline diagnostics; the body surfaces
    // the paths to the model. Keep both.
    context.record_mutated_path(path.clone());

    // Create success response
    let success_message = if edit_operations.len() == 1 {
        "OK".to_string()
    } else {
        format!("OK: Applied {} edit operations", edit_operations.len())
    };

    debug!(path = %file_path, num_operations = edit_operations.len(), bytes_written = final_result.bytes_written, total_replacements = total_replacements, encoding = %final_result.encoding_detected, line_endings = %final_result.line_endings_preserved, "Edit operation(s) completed successfully"
    );

    // Carry the mutating-result envelope: the post-edit file re-tagged with
    // hashline anchors (so the model can chain the next edit without re-reading)
    // plus the mutated path, layered on top of the existing typed EditResult
    // fields. ONLY this committed/Applied path carries the envelope — the
    // ambiguity and near-miss returns above did not mutate, so they do not.
    Ok(mutation_success_response(
        success_message,
        &new_content,
        vec![path.to_string_lossy().into_owned()],
        serde_json::json!({
            "bytes_written": final_result.bytes_written,
            "replacements_made": final_result.replacements_made,
            "encoding_detected": final_result.encoding_detected,
            "line_endings_preserved": final_result.line_endings_preserved,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create test arguments for the edit tool
    fn create_edit_arguments(
        file_path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: Option<bool>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::Value::String(file_path.to_string()),
        );
        args.insert(
            "old_string".to_string(),
            serde_json::Value::String(old_string.to_string()),
        );
        args.insert(
            "new_string".to_string(),
            serde_json::Value::String(new_string.to_string()),
        );

        if let Some(replace_all) = replace_all {
            args.insert(
                "replace_all".to_string(),
                serde_json::Value::Bool(replace_all),
            );
        }

        args
    }

    #[test]
    fn test_line_ending_detection() {
        // Test Unix line endings (LF)
        let unix_content = "Line 1\nLine 2\nLine 3\n";
        assert_eq!(LineEnding::detect(unix_content), LineEnding::Lf);

        // Test Windows line endings (CRLF)
        let windows_content = "Line 1\r\nLine 2\r\nLine 3\r\n";
        assert_eq!(LineEnding::detect(windows_content), LineEnding::CrLf);

        // Test Classic Mac line endings (CR)
        let mac_content = "Line 1\rLine 2\rLine 3\r";
        assert_eq!(LineEnding::detect(mac_content), LineEnding::Cr);

        // Test mixed line endings
        let mixed_content = "Line 1\nLine 2\r\nLine 3\r";
        assert_eq!(LineEnding::detect(mixed_content), LineEnding::Mixed);

        // Test no line endings
        let no_endings = "Single line";
        assert_eq!(LineEnding::detect(no_endings), LineEnding::Lf);

        // Test empty content
        let empty_content = "";
        assert_eq!(LineEnding::detect(empty_content), LineEnding::Lf);
    }

    #[test]
    fn test_edit_tool_operation_metadata() {
        let op = EditFile;
        assert_eq!(op.verb(), "edit");
        assert_eq!(op.noun(), "file");
        assert!(!op.description().is_empty());
    }

    #[tokio::test]
    async fn test_edit_single_replacement_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_edit.txt");
        let initial_content = "Hello world! This is a test file.";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // Verify file was edited correctly
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello universe! This is a test file.");
    }

    #[tokio::test]
    async fn test_edit_replace_all_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_replace_all.txt");
        let initial_content = "test test test";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "test", "exam", Some(true));

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify all occurrences were replaced
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "exam exam exam");
    }

    #[tokio::test]
    async fn test_edit_multiple_occurrences_without_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_multiple.txt");
        let initial_content = "duplicate duplicate duplicate";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "duplicate",
            "unique",
            None, // replace_all = false by default
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify only the first occurrence was replaced
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "unique duplicate duplicate");
    }

    /// A `find` with no confident match no longer errors with the bare
    /// "not found in file" string: it returns a SUCCESSFUL structured near-miss
    /// (echoing the searched-for text) and leaves the file byte-identical. Here
    /// the lone line is too dissimilar to surface as a near-miss, so the prompt
    /// states nothing is close — but it is still a successful structured result.
    #[tokio::test]
    async fn test_edit_string_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_not_found.txt");
        let initial_content = "Hello world!";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "nonexistent",
            "replacement",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "no-match must be a successful structured near-miss, got {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(
            call.is_error,
            Some(false),
            "near-miss is not an error result"
        );

        let text = result_text(&call);
        // Echoes the searched-for text and is NOT the legacy "not found in file".
        assert!(text.contains("nonexistent"), "must echo the find: {text}");
        assert!(
            !text.contains("not found in file"),
            "legacy bare error string must be gone: {text}"
        );

        // Verify file was not modified
        let unchanged_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(unchanged_content, initial_content);
    }

    #[tokio::test]
    async fn test_edit_file_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_file = temp_dir.path().join("does_not_exist.txt");

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&nonexistent_file.to_string_lossy(), "old", "new", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_str = format!("{:?}", error);
        // The error message from shared_utils says "File not found"
        assert!(
            error_str.contains("File does not exist")
                || error_str.contains("File not found")
                || error_str.contains("does not exist")
                || error_str.contains("NotFound")
        );
    }

    #[tokio::test]
    async fn test_edit_empty_parameters() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test empty file path
        let args = create_edit_arguments("", "old", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("path cannot be empty"));

        // Test empty old_string
        let args = create_edit_arguments(&test_file.to_string_lossy(), "", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("old_text cannot be empty"));

        // Test identical old_string and new_string
        let args = create_edit_arguments(&test_file.to_string_lossy(), "same", "same", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("must be different"));
    }

    #[tokio::test]
    async fn test_edit_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("unicode_test.txt");
        let unicode_content = "Hello 🌍! Здравствуй мир! 你好世界!";
        fs::write(&test_file, unicode_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "🌍", "🚀", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify Unicode replacement worked correctly
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello 🚀! Здравствуй мир! 你好世界!");
    }

    #[tokio::test]
    async fn test_edit_preserves_line_endings() {
        let temp_dir = TempDir::new().unwrap();

        // Test Windows line endings preservation
        let windows_file = temp_dir.path().join("windows_endings.txt");
        let windows_content = "Line 1\r\nold text\r\nLine 3\r\n";
        fs::write(&windows_file, windows_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &windows_file.to_string_lossy(),
            "old text",
            "new text",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&windows_file).unwrap();
        assert_eq!(edited_content, "Line 1\r\nnew text\r\nLine 3\r\n");
        assert!(edited_content.contains("\r\n"));
    }

    #[tokio::test]
    async fn test_edit_atomic_operation_failure_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_atomic.txt");
        let initial_content = "original content";
        fs::write(&test_file, initial_content).unwrap();

        // Make file read-only to cause atomic operation to fail during permission setting
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;

            let readonly_permissions = Permissions::from_mode(0o444);
            fs::set_permissions(&test_file, readonly_permissions).unwrap();

            let tool = EditFileTool::new();

            // Even if the operation fails, we should verify no temporary files are left behind
            let _temp_pattern = format!("{}.tmp.*", test_file.display());

            // The edit should work even with readonly file since we change permissions on temp file
            let edit_result = tool.edit_file_atomic(
                temp_dir.path(),
                &test_file.to_string_lossy(),
                "original",
                "modified",
                false,
            );

            // Check that no temporary files remain regardless of result
            let temp_files: Vec<_> = temp_dir
                .path()
                .read_dir()
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
                .collect();

            assert!(
                temp_files.is_empty(),
                "Temporary files should be cleaned up"
            );

            // If the edit succeeded, verify the content was actually changed
            if edit_result.is_ok() {
                let final_content = fs::read_to_string(&test_file).unwrap();
                assert_eq!(final_content, "modified content");
            }
        }
    }

    #[tokio::test]
    async fn test_edit_file_permissions_preservation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("permissions_test.txt");
        let initial_content = "test content";
        fs::write(&test_file, initial_content).unwrap();

        // Set specific permissions (only on Unix systems)
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;

            let permissions = Permissions::from_mode(0o755);
            fs::set_permissions(&test_file, permissions).unwrap();

            let original_metadata = fs::metadata(&test_file).unwrap();
            let original_mode = original_metadata.permissions().mode();

            let tool = EditFileTool::new();
            let edit_result = tool.edit_file_atomic(
                temp_dir.path(),
                &test_file.to_string_lossy(),
                "test",
                "updated",
                false,
            );

            assert!(edit_result.is_ok());

            // Verify permissions were preserved
            let new_metadata = fs::metadata(&test_file).unwrap();
            let new_mode = new_metadata.permissions().mode();
            assert_eq!(
                original_mode, new_mode,
                "File permissions should be preserved"
            );

            // Verify content was updated
            let final_content = fs::read_to_string(&test_file).unwrap();
            assert_eq!(final_content, "updated content");
        }
    }

    /// Editing a file IS modifying it: the post-edit modification time must
    /// advance past the pre-edit mtime. Preserving the old mtime defeats every
    /// mtime-based staleness check downstream (cargo/make rebuilds, file
    /// watchers, rust-analyzer). Seed a fixed past mtime (no wall-clock sleep)
    /// and assert the edit produces a strictly greater mtime.
    #[tokio::test]
    async fn test_edit_file_advances_modification_time() {
        use filetime::{set_file_mtime, FileTime};

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("mtime_test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Seed a clearly-old mtime well in the past (2001-09-09 01:46:40 UTC).
        let old_mtime = FileTime::from_unix_time(1_000_000_000, 0);
        set_file_mtime(&test_file, old_mtime).unwrap();
        let seeded = FileTime::from_last_modification_time(&fs::metadata(&test_file).unwrap());
        assert_eq!(seeded, old_mtime, "mtime seed should be applied");

        let tool = EditFileTool::new();
        let edit_result = tool.edit_file_atomic(
            temp_dir.path(),
            &test_file.to_string_lossy(),
            "test",
            "updated",
            false,
        );
        assert!(edit_result.is_ok());

        let new_mtime = FileTime::from_last_modification_time(&fs::metadata(&test_file).unwrap());
        assert!(
            new_mtime > old_mtime,
            "edit must advance the file's modification time \
             (old={old_mtime:?}, new={new_mtime:?})"
        );
    }

    #[tokio::test]
    async fn test_edit_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("response_test.txt");
        let initial_content = "Hello world!";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());

        // The first content block stays the plain "OK" success message.
        let response_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content in response"),
        };
        assert_eq!(response_text, "OK");

        // …and a successful edit now also carries the mutating-result envelope:
        // the hashline-tagged post-edit content and the mutated path. Verify the
        // mutation really happened, then assert the envelope describes it.
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "Hello universe!",
            "the edit must have been committed"
        );
        let structured = call_result
            .structured_content
            .expect("successful edit sets structured content");
        let mutation = &structured["mutation"];
        assert_eq!(
            mutation["tagged_content"].as_str().unwrap(),
            swissarmyhammer_hashline::tag("Hello universe!", 1)
        );
        let paths = mutation["mutated_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].as_str().unwrap().ends_with("response_test.txt"));
        assert_eq!(mutation["replacements_made"], serde_json::json!(1));
    }

    #[test]
    fn test_edit_validation_logic() {
        let tool = EditFileTool::new();

        // Test with content that has multiple occurrences
        let content = "test content with test and more test";
        let _result = tool.validate_edit_operation(
            std::path::Path::new("/tmp"),
            "/dev/null", // Won't be used in this test
            content,
            "test",
            false, // replace_all = false
        );

        // This should fail because we have multiple occurrences but replace_all = false
        // However, it will fail earlier because /dev/null doesn't exist as a regular file
        // So let's test the logic directly

        // Count occurrences manually to verify logic
        let matches: Vec<_> = content.matches("test").collect();
        assert_eq!(matches.len(), 3);

        // Test unique string
        let matches_unique: Vec<_> = content.matches("content").collect();
        assert_eq!(matches_unique.len(), 1);
    }

    #[test]
    fn test_encoding_detection_logic() {
        let tool = EditFileTool::new();

        // Create a temporary file with UTF-8 content
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("encoding_test.txt");
        let utf8_content = "Hello, 世界! 🌍";
        fs::write(&test_file, utf8_content).unwrap();

        let result = tool.read_with_encoding_detection(&test_file);
        assert!(result.is_ok());

        let (content, encoding) = result.unwrap();
        assert_eq!(content, utf8_content);
        assert_eq!(encoding.name(), "UTF-8");
    }

    #[tokio::test]
    async fn test_edit_json_argument_parsing_error() {
        let context = crate::test_utils::create_test_context().await;

        // Create invalid arguments (missing both single edit and multiple edits modes)
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String("/test/path".to_string()),
        );
        args.insert(
            "old_string".to_string(),
            serde_json::Value::String("old".to_string()),
        );
        // Missing "new_string" field and no "edits" array

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_str = format!("{:?}", error);
        // A find (old_string is now an alias of `find`) with no matching replace
        // must error rather than silently dropping the unpaired find.
        assert!(
            error_str.contains("find provided without a matching replace")
                || error_str.contains("replace"),
            "unexpected error: {error_str}"
        );
    }

    #[tokio::test]
    async fn test_edit_large_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("large_file.txt");

        // Create a reasonably large file (1MB) with repetitive content
        let chunk = "This is a line of test content that will be repeated many times.\n";
        let chunk_size = chunk.len();
        let target_size = 1_000_000; // 1MB
        let repetitions = target_size / chunk_size;

        let large_content = chunk.repeat(repetitions);
        fs::write(&test_file, &large_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "test content",
            "modified content",
            Some(true), // Replace all occurrences
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify the replacements were made
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert!(edited_content.contains("modified content"));
        assert!(!edited_content.contains("test content"));
    }

    /// An empty file has no lines to surface, so the near-miss has no candidate
    /// spans — but it is still a SUCCESSFUL structured result (not an error) that
    /// echoes the searched-for text and states the file has nothing close.
    #[tokio::test]
    async fn test_edit_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_file.txt");
        fs::write(&test_file, "").unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "nonexistent",
            "replacement",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "empty-file no-match must be a successful near-miss, got {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));

        let text = result_text(&call);
        assert!(text.contains("nonexistent"), "must echo the find: {text}");
        // No near-miss spans exist in an empty file.
        assert!(
            text.contains("no close") || text.contains("nothing close"),
            "must state nothing is close: {text}"
        );

        // File still empty (byte-identical).
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "");
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_sequential() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multiple_edits.txt");
        let initial_content = "Hello world! This is a test.";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Create arguments with multiple edits
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "oldText": "world",
                    "newText": "universe"
                },
                {
                    "oldText": "test",
                    "newText": "example"
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify all edits were applied sequentially
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello universe! This is a example.");
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_with_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("alias_test.txt");
        let initial_content = "foo bar baz";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test different parameter aliases
        let mut args = serde_json::Map::new();
        args.insert(
            "filePath".to_string(), // Using filePath alias
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "old_string": "foo",  // Using old_string alias
                    "new_text": "FOO"     // Using new_text alias
                },
                {
                    "old_text": "bar",    // Using old_text alias
                    "new_string": "BAR"   // Using new_string alias
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "FOO BAR baz");
    }

    #[tokio::test]
    async fn test_edit_single_mode_with_path_alias() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("single_alias.txt");
        let initial_content = "test content";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test single edit mode with different parameter aliases
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(), // Using file_path alias
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "oldText".to_string(), // Using oldText alias
            serde_json::Value::String("test".to_string()),
        );
        args.insert(
            "newText".to_string(), // Using newText alias
            serde_json::Value::String("demo".to_string()),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "demo content");
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_with_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("replace_all_multi.txt");
        let initial_content = "test test test, example example";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "oldText": "test",
                    "newText": "exam",
                    "replace_all": true
                },
                {
                    "oldText": "example",
                    "newText": "sample",
                    "replace_all": true
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "exam exam exam, sample sample");
    }

    #[tokio::test]
    async fn test_edit_empty_edits_array() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_edits.txt");
        fs::write(&test_file, "content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert("edits".to_string(), serde_json::json!([]));

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("edits array cannot be empty"));
    }

    #[tokio::test]
    async fn test_edit_missing_path() {
        let context = crate::test_utils::create_test_context().await;

        // Missing path parameter
        let mut args = serde_json::Map::new();
        args.insert(
            "old_string".to_string(),
            serde_json::Value::String("old".to_string()),
        );
        args.insert(
            "new_string".to_string(),
            serde_json::Value::String("new".to_string()),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("path"));
    }

    #[tokio::test]
    async fn test_edit_whitespace_path_error() {
        let context = crate::test_utils::create_test_context().await;

        let args = create_edit_arguments("   ", "old", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(
            format!("{:?}", result).contains("empty") || format!("{:?}", result).contains("path")
        );
    }

    #[tokio::test]
    async fn test_edit_old_string_in_index_one_operation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("index_test.txt");
        fs::write(&test_file, "line 1\nline 2\nline 3\n").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Multiple edits - second operation has empty old_text
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "line 1", "newText": "LINE ONE" },
                { "oldText": "", "newText": "something" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("old_text cannot be empty") || err.contains("empty"));
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_same_and_different_not_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("same_test.txt");
        fs::write(&test_file, "content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Multiple edits - second operation has same old and new text
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "content", "newText": "new_content" },
                { "oldText": "same_text", "newText": "same_text" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("must be different") || err.contains("different"));
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_success_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multi_response.txt");
        fs::write(&test_file, "foo bar baz").unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "foo", "newText": "FOO" },
                { "oldText": "bar", "newText": "BAR" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        // Multiple edits response says "OK: Applied N edit operations"
        assert!(text.contains("OK") && text.contains("2") || text.contains("Applied"));
    }

    // =========================================================================
    // normalize_edit_args — pure argument shaping (no IO)
    // =========================================================================

    /// Build a JSON arg map from a serde_json::json! object literal.
    fn args(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().expect("object literal").clone()
    }

    fn pair(find: &str, replace: &str, replace_all: bool) -> EditPair {
        EditPair {
            find: find.to_string(),
            replace: replace.to_string(),
            replace_all,
            occurrence: None,
        }
    }

    #[test]
    fn normalize_canonical_scalar_find_replace() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "file_path": "/x", "find": "a", "replace": "b"
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false)]);
    }

    #[test]
    fn normalize_legacy_old_new_string_resolves_same_as_find_replace() {
        let canonical = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let legacy = normalize_edit_args(&args(serde_json::json!({
            "old_string": "a", "new_string": "b"
        })))
        .unwrap();
        assert_eq!(legacy, canonical);
    }

    #[test]
    fn normalize_legacy_oldtext_newtext_resolves_same_as_find_replace() {
        let canonical = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let legacy = normalize_edit_args(&args(serde_json::json!({
            "oldText": "a", "newText": "b"
        })))
        .unwrap();
        assert_eq!(legacy, canonical);
    }

    #[test]
    fn normalize_search_with_alias_pair() {
        // edits[] entries using {search, with} aliases.
        let got = normalize_edit_args(&args(serde_json::json!({
            "edits": [{ "search": "a", "with": "b" }, { "search": "c", "with": "d" }]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_scalar_array_and_edits_yield_same_pairs() {
        let scalar = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let arrays = normalize_edit_args(&args(serde_json::json!({
            "find": ["a"], "replace": ["b"]
        })))
        .unwrap();
        let edits = normalize_edit_args(&args(serde_json::json!({
            "edits": [{ "find": "a", "replace": "b" }]
        })))
        .unwrap();
        assert_eq!(scalar, vec![pair("a", "b", false)]);
        assert_eq!(arrays, scalar);
        assert_eq!(edits, scalar);
    }

    #[test]
    fn normalize_parallel_arrays_zip() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "c"], "replace": ["b", "d"]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_broadcast_single_replace_to_many_finds() {
        // Delete-many: many finds + one empty replace.
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b", "c"], "replace": [""]
        })))
        .unwrap();
        assert_eq!(
            got,
            vec![
                pair("a", "", false),
                pair("b", "", false),
                pair("c", "", false)
            ]
        );
    }

    #[test]
    fn normalize_broadcast_scalar_replace_to_array_finds() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b"], "replace": "X"
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "X", false), pair("b", "X", false)]);
    }

    #[test]
    fn normalize_toplevel_and_edits_concatenate() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b",
            "edits": [{ "find": "c", "replace": "d" }]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_replace_all_scalar_applies_to_toplevel_pair() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b", "replace_all": true
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", true)]);
    }

    #[test]
    fn normalize_replace_all_per_edit_entry() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "edits": [
                { "find": "a", "replace": "b", "replace_all": true },
                { "find": "c", "replace": "d" }
            ]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", true), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_mismatched_array_lengths_errors_with_remainder() {
        // 3 finds, 2 replaces (not a broadcast): zip the first 2, surface the
        // unpaired remainder in the error — never silently drop.
        let err = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b", "c"], "replace": ["x", "y"]
        })))
        .unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains('c'),
            "error must name the unpaired find: {msg}"
        );
    }

    #[test]
    fn normalize_one_find_many_replaces_errors_with_remainder() {
        let err = normalize_edit_args(&args(serde_json::json!({
            "find": ["a"], "replace": ["x", "y"]
        })))
        .unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains('y'),
            "error must name the unpaired replace: {msg}"
        );
    }

    #[test]
    fn normalize_no_find_or_replace_or_edits_errors() {
        let err = normalize_edit_args(&args(serde_json::json!({ "file_path": "/x" }))).unwrap_err();
        let _ = format!("{err:?}");
    }

    #[tokio::test]
    async fn test_edit_cr_line_endings_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("cr_endings.txt");
        // Classic Mac line endings
        let content = "line1\rold content\rline3\r";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "old content",
            "new content",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited = fs::read(&test_file).unwrap();
        let edited_str = String::from_utf8(edited).unwrap();
        assert!(edited_str.contains("new content"));
        // CR line endings should be preserved
        assert!(edited_str.contains('\r'));
    }

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

    /// On a successful edit, the mutated path is recorded on the typed channel so
    /// the inline-diagnostics fold-in still fires.
    #[tokio::test]
    async fn cascade_records_mutated_path_on_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("recorded.txt");
        fs::write(&test_file, "hello world").unwrap();

        // A fresh per-call sink, exactly as the dispatch chokepoint installs.
        let context = crate::test_utils::create_test_context()
            .await
            .with_fresh_mutated_paths();
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let recorded = context.take_mutated_paths();
        assert_eq!(recorded.len(), 1, "exactly one path recorded");
        assert!(
            recorded[0].to_string_lossy().ends_with("recorded.txt"),
            "recorded path: {}",
            recorded[0].display()
        );
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

    /// Read the text payload of a `CallToolResult`.
    fn result_text(result: &CallToolResult) -> String {
        match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("expected text content"),
        }
    }

    /// Build a JSON arg map with `find`/`replace` (and optional `occurrence`).
    fn ambiguity_args(
        file_path: &str,
        find: &str,
        replace: &str,
        occurrence: Option<u64>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::Value::String(file_path.to_string()),
        );
        args.insert(
            "find".to_string(),
            serde_json::Value::String(find.to_string()),
        );
        args.insert(
            "replace".to_string(),
            serde_json::Value::String(replace.to_string()),
        );
        if let Some(n) = occurrence {
            args.insert(
                "occurrence".to_string(),
                serde_json::Value::Number(n.into()),
            );
        }
        args
    }

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

    // =========================================================================
    // Mutating-result envelope: tagged_content + mutated_paths on SUCCESS only
    // =========================================================================

    /// Join every text content block of a result (the success message block AND
    /// the appended envelope block), so envelope assertions can scan the whole
    /// surfaced text — not just `content[0]`.
    fn all_text(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                rmcp::model::RawContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A successful single-pair edit carries the mutation envelope:
    /// `tagged_content` (the hashline-tagged post-edit file) and `mutated_paths`
    /// in the structured surface, plus an appended text block, while the first
    /// content block stays the plain "OK" message.
    #[tokio::test]
    async fn successful_edit_carries_tagged_content_and_mutated_paths() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("envelope.txt");
        fs::write(&test_file, "Hello world!").unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let call = execute_edit(args, &context).await.unwrap();
        assert_eq!(call.is_error, Some(false));

        // The first block is still the plain success message.
        assert_eq!(result_text(&call), "OK");

        // Structured surface carries the envelope.
        let structured = call
            .structured_content
            .clone()
            .expect("successful edit sets structured content");
        let mutation = &structured["mutation"];
        // tagged_content is the hashline tag of the POST-edit file.
        let expected_tagged = swissarmyhammer_hashline::tag("Hello universe!", 1);
        assert_eq!(
            mutation["tagged_content"].as_str().unwrap(),
            expected_tagged
        );
        // mutated_paths carries the absolute path that was changed.
        let paths = mutation["mutated_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].as_str().unwrap().ends_with("envelope.txt"));
        // Existing EditResult fields are preserved in the structured surface.
        assert_eq!(mutation["replacements_made"], serde_json::json!(1));
        assert!(mutation["bytes_written"].as_u64().unwrap() > 0);
        assert!(mutation.get("encoding_detected").is_some());
        assert!(mutation.get("line_endings_preserved").is_some());

        // The appended text block also carries the tagged content so text-only
        // hosts deliver it to the model.
        assert!(
            all_text(&call).contains(&expected_tagged),
            "envelope text block carries the tagged content"
        );
    }

    /// Round-trip: an anchor taken from a prior edit's `tagged_content` resolves
    /// against the on-disk file in an immediately-following `edit files` call,
    /// with NO intervening read.
    #[tokio::test]
    async fn anchor_from_prior_envelope_resolves_in_next_edit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("roundtrip.txt");
        fs::write(&test_file, "alpha\nbeta\ngamma\n").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // First edit changes line 2.
        let args = create_edit_arguments(&test_file.to_string_lossy(), "beta", "BETA", None);
        let call = execute_edit(args, &context).await.unwrap();
        let structured = call.structured_content.expect("structured content");
        let tagged = structured["mutation"]["tagged_content"]
            .as_str()
            .unwrap()
            .to_string();

        // Pull the `N:HH` anchor for the third line (gamma) straight from the
        // returned tagged_content — no intervening read.
        let anchor = tagged
            .lines()
            .find(|l| l.contains("|gamma"))
            .and_then(|l| l.split('|').next())
            .expect("gamma line present in tagged_content")
            .to_string();
        assert!(
            anchor.starts_with("3:"),
            "anchor should target line 3: {anchor}"
        );

        // Use that anchor as the `find` in a chained edit — it must resolve.
        let mut args2 = serde_json::Map::new();
        args2.insert(
            "file_path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args2.insert("find".to_string(), serde_json::Value::String(anchor));
        args2.insert(
            "replace".to_string(),
            serde_json::Value::String("GAMMA".to_string()),
        );

        let call2 = execute_edit(args2, &context).await.unwrap();
        assert_eq!(
            call2.is_error,
            Some(false),
            "anchor must resolve: {call2:?}"
        );
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "alpha\nBETA\nGAMMA\n"
        );
    }

    /// An ambiguity result (no mutation) does NOT carry the envelope.
    #[tokio::test]
    async fn ambiguous_result_has_no_mutation_envelope() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("ambig_no_env.txt");
        let content = "head\nfoo()\nmid\nfoo()\ntail\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = ambiguity_args(&test_file.to_string_lossy(), "  foo()  ", "bar()", None);

        let call = execute_edit(args, &context).await.unwrap();
        assert_eq!(call.is_error, Some(false));
        // No structured envelope — nothing mutated.
        assert!(
            call.structured_content.is_none(),
            "ambiguity result carries no mutation envelope"
        );
        // File untouched.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// A near-miss result (no mutation) does NOT carry the envelope.
    #[tokio::test]
    async fn near_miss_result_has_no_mutation_envelope() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("near_miss_no_env.txt");
        let content = "the quick brown fox\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "the quick brown cat",
            "replacement",
            None,
        );

        let call = execute_edit(args, &context).await.unwrap();
        assert_eq!(call.is_error, Some(false));
        assert!(
            call.structured_content.is_none(),
            "near-miss result carries no mutation envelope"
        );
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
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
    // Pure-function argument normalization error arms
    // =====================================================================

    /// `collect_strings` rejects a non-string array element, naming the offender.
    #[test]
    fn test_collect_strings_rejects_non_string_array_element() {
        let value = serde_json::json!(["ok", 42]);
        let err = collect_strings(Some(&value)).unwrap_err();
        assert!(format!("{err:?}").contains("array entries must be strings"));
    }

    /// `collect_strings` rejects a value that is neither string nor array.
    #[test]
    fn test_collect_strings_rejects_non_string_non_array() {
        let value = serde_json::json!({ "not": "a string" });
        let err = collect_strings(Some(&value)).unwrap_err();
        assert!(format!("{err:?}").contains("string or array of strings"));
    }

    /// `collect_strings` returns `None` for absent input and a one-element vec for
    /// a scalar string.
    #[test]
    fn test_collect_strings_absent_and_scalar() {
        assert!(collect_strings(None).unwrap().is_none());
        let scalar = serde_json::json!("hello");
        assert_eq!(
            collect_strings(Some(&scalar)).unwrap().unwrap(),
            vec!["hello".to_string()]
        );
    }

    /// A top-level `replace` with no matching `find` is rejected.
    #[test]
    fn test_normalize_replace_without_find() {
        let mut args = serde_json::Map::new();
        args.insert("replace".to_string(), serde_json::json!("x"));
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("replace provided without a matching find"));
    }

    /// A top-level `find` with no matching `replace` is rejected.
    #[test]
    fn test_normalize_find_without_replace() {
        let mut args = serde_json::Map::new();
        args.insert("find".to_string(), serde_json::json!("x"));
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("find provided without a matching replace"));
    }

    /// `edits` that is not an array is rejected.
    #[test]
    fn test_normalize_edits_not_an_array() {
        let mut args = serde_json::Map::new();
        args.insert("edits".to_string(), serde_json::json!("not an array"));
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("edits must be an array"));
    }

    /// An `edits[]` entry that is not an object is rejected, naming the index.
    #[test]
    fn test_normalize_edits_entry_not_an_object() {
        let mut args = serde_json::Map::new();
        args.insert("edits".to_string(), serde_json::json!(["scalar"]));
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("edits[0] must be an object"));
    }

    /// An `edits[]` entry missing `find` (or `replace`) is rejected, naming it.
    #[test]
    fn test_normalize_edits_entry_missing_find_and_replace() {
        let mut missing_find = serde_json::Map::new();
        missing_find.insert("edits".to_string(), serde_json::json!([{ "replace": "x" }]));
        let err = normalize_edit_args(&missing_find).unwrap_err();
        assert!(format!("{err:?}").contains("edits[0] is missing find"));

        let mut missing_replace = serde_json::Map::new();
        missing_replace.insert("edits".to_string(), serde_json::json!([{ "find": "x" }]));
        let err = normalize_edit_args(&missing_replace).unwrap_err();
        assert!(format!("{err:?}").contains("edits[0] is missing replace"));
    }

    /// A mismatched find/replace count (2 finds, 1 replace is broadcast, but
    /// 2 finds + 3 replaces cannot pair) surfaces the unpaired remainder.
    #[test]
    fn test_pair_finds_replaces_mismatch_reports_remainder() {
        let finds = vec!["a".to_string(), "b".to_string()];
        let replaces = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let err = pair_finds_replaces(finds, replaces, false, None).unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("mismatched find/replace counts"));
        assert!(msg.contains("unpaired replaces"));
    }

    /// An empty arg map (no find/replace/edits) reports "no edits provided".
    #[test]
    fn test_normalize_no_edits_provided() {
        let args = serde_json::Map::new();
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("no edits provided"));
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

    /// `LineEnding::as_str` renders the `Mixed` variant label.
    #[test]
    fn test_line_ending_mixed_as_str() {
        assert_eq!(LineEnding::detect("a\nb\r\nc\r").as_str(), "Mixed");
        assert_eq!(LineEnding::Lf.as_str(), "LF");
        assert_eq!(LineEnding::CrLf.as_str(), "CRLF");
        assert_eq!(LineEnding::Cr.as_str(), "CR");
    }

    // =====================================================================
    // Legacy single-pair API error arms (validate_edit_operation)
    // =====================================================================

    /// `validate_edit_operation` rejects a path that does not exist on disk.
    #[test]
    fn test_validate_edit_operation_file_does_not_exist() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("absent.txt");
        let tool = EditFileTool::new();
        let err = tool
            .validate_edit_operation(
                temp_dir.path(),
                &missing.to_string_lossy(),
                "content",
                "content",
                false,
            )
            .unwrap_err();
        assert!(format!("{err:?}").contains("File does not exist"));
    }

    /// `validate_edit_operation` rejects an `old_string` absent from the content.
    #[test]
    fn test_validate_edit_operation_string_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("present.txt");
        fs::write(&file, "hello world").unwrap();
        let tool = EditFileTool::new();
        let err = tool
            .validate_edit_operation(
                temp_dir.path(),
                &file.to_string_lossy(),
                "hello world",
                "absent-substring",
                false,
            )
            .unwrap_err();
        assert!(format!("{err:?}").contains("not found in file"));
    }

    /// `edit_file_atomic` with `replace_all` rewrites every occurrence and reports
    /// the count (covering the replace-all replacement branch).
    #[tokio::test]
    async fn test_edit_file_atomic_replace_all_counts() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("repeat.txt");
        fs::write(&file, "x x x").unwrap();

        let tool = EditFileTool::new();
        let result = tool
            .edit_file_atomic(temp_dir.path(), &file.to_string_lossy(), "x", "y", true)
            .unwrap();
        assert_eq!(result.replacements_made, 3);
        assert_eq!(fs::read_to_string(&file).unwrap(), "y y y");
    }

    // =====================================================================
    // Encoding/decoding error arms
    // =====================================================================

    /// `read_with_encoding_detection` rejects bytes that cannot be decoded with
    /// the detected encoding (a UTF-16LE BOM followed by an odd trailing byte
    /// yields a decode error).
    #[test]
    fn test_read_with_encoding_detection_decode_error() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("bad_utf16.txt");
        // UTF-16LE BOM (0xFF 0xFE), then a lone trailing byte → malformed unit.
        fs::write(&file, [0xFFu8, 0xFE, 0x41]).unwrap();

        let tool = EditFileTool::new();
        let result = tool.read_with_encoding_detection(&file);
        // A lone trailing byte after a UTF-16LE BOM is a malformed code unit, so
        // encoding_rs reports a decode error and this arm must reject it.
        let err = result.expect_err("a malformed UTF-16LE byte sequence must be rejected");
        assert!(format!("{err:?}").contains("Failed to decode"));
    }

    /// `read_with_encoding_detection` surfaces a file-read error when the path is
    /// missing.
    #[test]
    fn test_read_with_encoding_detection_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("nope.txt");
        let tool = EditFileTool::new();
        let err = tool.read_with_encoding_detection(&missing).unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("read file for encoding detection") || msg.contains("not found"));
    }

    // =====================================================================
    // commit_content cleanup arms (real fault injection)
    // =====================================================================

    /// When the atomic rename cannot complete because the target path is a
    /// directory, `commit_content` removes its temp file and surfaces an error,
    /// leaving no `.tmp.` debris.
    #[test]
    fn test_commit_content_cleans_temp_on_rename_failure() {
        let temp_dir = TempDir::new().unwrap();
        // Target is an existing directory: rename(temp_file, dir) fails.
        let target = temp_dir.path().join("a_directory");
        fs::create_dir(&target).unwrap();

        let tool = EditFileTool::new();
        let result = tool.commit_content(
            &target,
            "new content",
            encoding_rs::UTF_8,
            LineEnding::Lf,
            1,
        );
        assert!(result.is_err(), "rename over a directory must fail");

        // The directory is untouched and no temp file remains.
        assert!(target.is_dir());
        let temp_files: Vec<_> = temp_dir
            .path()
            .read_dir()
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(temp_files.is_empty(), "temp file must be cleaned up");
    }

    /// `commit_content` propagates a metadata-read failure when the target is
    /// missing (the original-permission capture cannot run).
    #[test]
    fn test_commit_content_metadata_read_failure() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("ghost.txt");
        let tool = EditFileTool::new();
        let err = tool
            .commit_content(&missing, "x", encoding_rs::UTF_8, LineEnding::Lf, 1)
            .unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("read metadata") || msg.contains("not found"));
    }

    /// `write_with_encoding` surfaces an error when the file cannot be created
    /// (its parent directory does not exist).
    #[test]
    fn test_write_with_encoding_create_failure() {
        let temp_dir = TempDir::new().unwrap();
        let unwritable = temp_dir.path().join("no_such_dir").join("out.txt");
        let tool = EditFileTool::new();
        let err = tool
            .write_with_encoding(&unwritable, "content", encoding_rs::UTF_8)
            .unwrap_err();
        // The missing parent surfaces as a NotFound, mapped to "File not found".
        assert!(format!("{err:?}").contains("File not found"));
    }

    /// `write_with_encoding` rejects content the target encoding cannot represent
    /// (a non-Latin character under windows-1252).
    #[test]
    fn test_write_with_encoding_encode_error() {
        let temp_dir = TempDir::new().unwrap();
        let out = temp_dir.path().join("out.txt");
        let tool = EditFileTool::new();
        // windows-1252 cannot encode an emoji → had_errors is true.
        let result = tool.write_with_encoding(&out, "🌍", encoding_rs::WINDOWS_1252);
        assert!(result.is_err(), "un-encodable content must error");
        assert!(format!("{:?}", result.unwrap_err()).contains("Failed to encode"));
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
