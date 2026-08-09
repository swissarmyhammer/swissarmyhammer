//! Report the token-identical blocks a set of files repeats.
//!
//! The algorithm is the jscpd Rust engine's own: [`detect_prepared`] is the
//! Rabin-Karp rolling-hash detector `cpd-core` publishes, and it takes the
//! token stream its caller hands it. The tokens come from this workspace's
//! tree-sitter roster rather than from `cpd-tokenizer`, because the same
//! parse decides which blocks are test code and which blocks a marker comment
//! exempts — see
//! [`duplication_source`](swissarmyhammer_sem::parser::plugins::code::duplication_source).
//!
//! A finding is a fact: a run of tokens over the minimum window, spelled the
//! same twice. No similarity threshold, and no judgment.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use cpd_core::detect::{detect_prepared, PreparedSource};
use cpd_core::hash::token_hash;
use cpd_core::models::{CpdClone, Fragment, Location};
use swissarmyhammer_sem::parser::plugins::code::{duplication_source, DuplicationSource};

use crate::ops::workspace_path::resolve_within;

/// The shortest run of tokens that counts as a duplicate.
///
/// Fifty tokens is about a dozen lines of ordinary code. Below it a match is
/// the language's own grammar repeating — a `match` arm, an import block, a
/// struct literal — rather than a block someone pasted.
const MINIMUM_WINDOW_TOKENS: usize = 50;

/// The kind byte every token is hashed under.
///
/// One byte for every token, because the token carries its exact source text
/// and the text alone tells one token from another. `cpd-tokenizer` uses the
/// byte to separate a keyword from an identifier that spells the same word,
/// which its own lexer can produce and a tree-sitter leaf cannot.
const TOKEN_KIND: u8 = 1;

/// The shortest line span cpd rejects a clone on, `0` meaning it rejects
/// none. The token window is the whole gate here, so the line filter is off.
const MINIMUM_CLONE_LINES: usize = 0;

/// One pair of token-identical blocks, ready to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicationFinding {
    /// The path the finding lands on, exactly as the caller spelled it, so
    /// the review engine matches the finding to the changed file it passed
    /// in.
    pub file: String,
    /// The one-based line the repeated block starts on.
    pub line: usize,
    /// The path of the block this one repeats.
    pub other_file: String,
    /// The one-based line the repeated block starts on in `other_file`.
    pub other_line: usize,
    /// How many lines the block spans.
    pub lines: usize,
    /// How many tokens the two blocks share.
    pub tokens: usize,
}

/// Renders as `path:line: message`, the shape the tool-rule contract parses.
impl fmt::Display for DuplicationFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: verbatim duplicate of {}:{} ({} lines / {} tokens)",
            self.file, self.line, self.other_file, self.other_line, self.lines, self.tokens
        )
    }
}

/// Every pair of token-identical blocks in `files`, inside one file and
/// across two.
///
/// `files` are paths as the caller spelled them, and every one of them is
/// resolved inside `working_dir` by
/// [`resolve_within`](crate::ops::workspace_path::resolve_within): a path that
/// climbs out of the workspace names no file this op will read. A file the
/// roster has no grammar for, and a file that cannot be read, are each skipped
/// without a finding — the same silence
/// [`find_commented_code`](crate::ops::find_commented_code) keeps, and the
/// reason the tool rule narrows its `match` to the roster's own extensions.
///
/// Two blocks are paired only when they parse to the same language, so a
/// `.rs` file and a `.py` file are never compared.
pub fn find_duplication(working_dir: &Path, files: &[&str]) -> Vec<DuplicationFinding> {
    let sources: Vec<ReadSource> = files
        .iter()
        .filter_map(|file| read_source(working_dir, file))
        .collect();

    detect_prepared(
        prepared_groups(&sources),
        MINIMUM_WINDOW_TOKENS,
        false,
        MINIMUM_CLONE_LINES,
        &[],
    )
    .iter()
    .filter_map(|clone| finding_of(clone, &sources))
    .collect()
}

/// One file the op read: the path the caller spelled, the text, and the parse.
struct ReadSource {
    /// The path exactly as the caller spelled it.
    path: String,
    /// The file's whole text, which the token spans index into.
    text: String,
    /// The tokens and the exemptions the parse reports.
    parsed: DuplicationSource,
}

/// Read one file, `None` when it cannot be read or the roster does not claim
/// it.
fn read_source(working_dir: &Path, file: &str) -> Option<ReadSource> {
    let text = std::fs::read_to_string(resolve_within(working_dir, file)?).ok()?;
    let parsed = duplication_source(file, &text)?;
    Some(ReadSource {
        path: file.to_string(),
        text,
        parsed,
    })
}

/// The sources grouped by language, which is the grouping `detect_prepared`
/// compares within. The map keeps the groups in a stable order, so a run over
/// the same files reports the same findings in the same order.
fn prepared_groups(sources: &[ReadSource]) -> Vec<Vec<PreparedSource>> {
    let mut groups: BTreeMap<&str, Vec<PreparedSource>> = BTreeMap::new();
    for source in sources {
        groups
            .entry(source.parsed.language)
            .or_default()
            .push(prepared_source(source));
    }
    groups.into_values().collect()
}

/// One file's tokens, hashed and positioned the way the detector reads them.
fn prepared_source(source: &ReadSource) -> PreparedSource {
    let mut hashes = Vec::with_capacity(source.parsed.tokens.len());
    let mut spans = Vec::with_capacity(source.parsed.tokens.len());
    for token in &source.parsed.tokens {
        let text = source
            .text
            .get(token.start.offset..token.end.offset)
            .unwrap_or_default();
        hashes.push(token_hash(TOKEN_KIND, text));
        spans.push((location_of(token.start), location_of(token.end)));
    }
    PreparedSource {
        id: source.path.clone(),
        format: source.parsed.language.to_string(),
        hashes,
        spans,
    }
}

/// The detector's own position type, from ours.
fn location_of(point: swissarmyhammer_sem::parser::plugins::code::TokenPoint) -> Location {
    Location {
        line: narrow(point.line),
        column: narrow(point.column),
        offset: narrow(point.offset),
    }
}

/// `value` as a `u32`, saturating rather than wrapping, so a file too large
/// to address in 32 bits reports a position at the ceiling instead of one
/// that reads as the start of the file.
fn narrow(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The finding `clone` is, `None` when either of its blocks is exempt.
///
/// The finding lands on the second block and names the first, so the message
/// reads the way a reader meets the code: this block repeats that one.
fn finding_of(clone: &CpdClone, sources: &[ReadSource]) -> Option<DuplicationFinding> {
    if is_exempt(&clone.fragment_a, sources) || is_exempt(&clone.fragment_b, sources) {
        return None;
    }
    let copy = &clone.fragment_b;
    let original = &clone.fragment_a;
    Some(DuplicationFinding {
        file: copy.source_id.clone(),
        line: copy.start.line as usize,
        other_file: original.source_id.clone(),
        other_line: original.start.line as usize,
        lines: copy.end.line.saturating_sub(copy.start.line) as usize + 1,
        tokens: clone.token_count as usize,
    })
}

/// Whether the block `fragment` names starts inside exempted code.
fn is_exempt(fragment: &Fragment, sources: &[ReadSource]) -> bool {
    sources.iter().any(|source| {
        source.path == fragment.source_id && source.parsed.exempts(fragment.start.offset as usize)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{workspace_beside_an_outside_file, WORKSPACE_OUTSIDE_RUST_FILE};

    /// A Rust block long enough to clear [`MINIMUM_WINDOW_TOKENS`], written
    /// once with the name the caller passes so two copies differ only in the
    /// function name — which is itself a token, so the shared run is the body.
    fn long_block(name: &str) -> String {
        format!(
            concat!(
                "pub fn {name}(grid: &[Vec<i32>], limit: i32) -> i32 {{\n",
                "    let mut band = 0;\n",
                "    let mut seen = 0;\n",
                "    for row in grid {{\n",
                "        for cell in row {{\n",
                "            if *cell < limit {{\n",
                "                band += *cell;\n",
                "                seen += 1;\n",
                "            }} else {{\n",
                "                band -= *cell;\n",
                "            }}\n",
                "        }}\n",
                "        if seen > limit {{\n",
                "            band = limit;\n",
                "        }}\n",
                "    }}\n",
                "    band\n",
                "}}\n",
            ),
            name = name
        )
    }

    /// One block that is valid JavaScript and valid TypeScript, so the two
    /// files carry token for token the same stream.
    ///
    /// The roster routes `.js` and `.ts` to different grammars, so the pair is
    /// what proves the detector groups by language: put both files in one
    /// group and this block matches itself across them.
    const ECMASCRIPT_BLOCK: &str = concat!(
        "export function foldedBand(grid, limit) {\n",
        "    let band = 0;\n",
        "    let seen = 0;\n",
        "    for (const row of grid) {\n",
        "        for (const cell of row) {\n",
        "            if (cell < limit) {\n",
        "                band += cell;\n",
        "                seen += 1;\n",
        "            } else {\n",
        "                band -= cell;\n",
        "            }\n",
        "        }\n",
        "        if (seen > limit) {\n",
        "            band = limit;\n",
        "        }\n",
        "    }\n",
        "    return band;\n",
        "}\n",
    );

    /// A short Rust block, under [`MINIMUM_WINDOW_TOKENS`].
    fn short_block(name: &str) -> String {
        format!("pub fn {name}(limit: i32) -> i32 {{\n    limit + 1\n}}\n")
    }

    /// Write `contents` at `name` under a fresh temporary directory.
    fn workspace_with(name: &str, contents: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create a scratch workspace");
        std::fs::write(dir.path().join(name), contents).expect("write the probe file");
        dir
    }

    #[test]
    fn two_identical_blocks_in_one_file_are_reported() {
        let source = format!("{}\n{}", long_block("first"), long_block("second"));
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].file, "probe.rs");
        assert_eq!(findings[0].other_file, "probe.rs");
        assert!(
            findings[0].line > findings[0].other_line,
            "the finding lands on the copy: {:?}",
            findings[0]
        );
        assert!(findings[0].tokens >= MINIMUM_WINDOW_TOKENS);
    }

    #[test]
    fn a_finding_renders_as_the_tool_rule_contract_line() {
        let finding = DuplicationFinding {
            file: "src/lib.rs".to_string(),
            line: 40,
            other_file: "src/other.rs".to_string(),
            other_line: 12,
            lines: 15,
            tokens: 96,
        };

        assert_eq!(
            finding.to_string(),
            "src/lib.rs:40: verbatim duplicate of src/other.rs:12 (15 lines / 96 tokens)"
        );
    }

    #[test]
    fn a_repeat_below_the_minimum_window_is_not_reported() {
        let source = format!("{}\n{}", short_block("first"), short_block("second"));
        let dir = workspace_with("probe.rs", &source);

        assert!(find_duplication(dir.path(), &["probe.rs"]).is_empty());
    }

    #[test]
    fn a_marker_suppressed_copy_is_not_reported() {
        let source = format!(
            "{}\n// sah:allow duplication the two shapes fork next week\n{}",
            long_block("first"),
            long_block("second")
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_duplicate_pair_inside_a_test_module_is_not_reported() {
        let source = format!(
            "#[cfg(test)]\nmod tests {{\n{}\n{}\n}}\n",
            long_block("first"),
            long_block("second")
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_block_pasted_into_two_files_is_reported() {
        let dir = workspace_with("first.rs", &long_block("one"));
        std::fs::write(dir.path().join("second.rs"), long_block("two"))
            .expect("write the second probe file");

        let findings = find_duplication(dir.path(), &["first.rs", "second.rs"]);

        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].file, "second.rs");
        assert_eq!(findings[0].other_file, "first.rs");
    }

    #[test]
    fn two_languages_are_never_paired() {
        let dir = workspace_with("probe.js", ECMASCRIPT_BLOCK);
        std::fs::write(dir.path().join("probe.ts"), ECMASCRIPT_BLOCK)
            .expect("write the typescript probe file");

        let findings = find_duplication(dir.path(), &["probe.js", "probe.ts"]);

        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_file_the_roster_does_not_claim_reports_nothing() {
        let source = format!("{}\n{}", long_block("first"), long_block("second"));
        let dir = workspace_with("notes.txt", &source);

        assert!(find_duplication(dir.path(), &["notes.txt"]).is_empty());
    }

    #[test]
    fn a_missing_file_reports_nothing_and_does_not_break_the_run() {
        let dir = tempfile::tempdir().expect("create a scratch workspace");

        assert!(find_duplication(dir.path(), &["gone.rs"]).is_empty());
    }

    #[test]
    fn an_absolute_path_inside_the_working_directory_is_read() {
        let source = format!("{}\n{}", long_block("first"), long_block("second"));
        let dir = workspace_with("probe.rs", &source);
        let absolute = dir.path().join("probe.rs").to_string_lossy().to_string();

        let findings = find_duplication(dir.path(), &[absolute.as_str()]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, absolute);
    }

    #[test]
    fn a_relative_path_that_climbs_out_of_the_working_directory_is_refused() {
        let source = format!("{}\n{}", long_block("first"), long_block("second"));
        let (_dir, workspace) =
            workspace_beside_an_outside_file(WORKSPACE_OUTSIDE_RUST_FILE, &source);
        let climbing = format!("../{WORKSPACE_OUTSIDE_RUST_FILE}");

        assert!(find_duplication(&workspace, &[climbing.as_str()]).is_empty());
    }

    #[test]
    fn an_absolute_path_outside_the_working_directory_is_refused() {
        let source = format!("{}\n{}", long_block("first"), long_block("second"));
        let (dir, workspace) =
            workspace_beside_an_outside_file(WORKSPACE_OUTSIDE_RUST_FILE, &source);
        let outside = dir
            .path()
            .join(WORKSPACE_OUTSIDE_RUST_FILE)
            .to_string_lossy()
            .to_string();

        assert!(find_duplication(&workspace, &[outside.as_str()]).is_empty());
    }
}
