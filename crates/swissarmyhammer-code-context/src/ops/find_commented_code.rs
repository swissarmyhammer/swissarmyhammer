//! Report the comment blocks that re-parse as code.
//!
//! The verdict itself belongs to the grammar roster and lives beside it, in
//! [`swissarmyhammer_sem::parser::plugins::code::commented_code_blocks`]. This
//! op is the file-list wrapper around it: read each path, ask the roster, and
//! shape the answer into the one line per block the review engine's tool-rule
//! contract parses.

use std::fmt;
use std::path::Path;

use swissarmyhammer_sem::parser::plugins::code::commented_code_blocks;

/// One block of commented-out code, ready to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentedCodeFinding {
    /// The path exactly as the caller spelled it, so the review engine matches
    /// the finding to the changed file it passed in.
    pub file: String,
    /// The one-based line the block starts on.
    pub line: usize,
    /// How many lines the block spans.
    pub lines: usize,
    /// The language the block re-parsed as.
    pub language: &'static str,
}

/// Renders as `path:line: message`, the shape the tool-rule contract parses.
impl fmt::Display for CommentedCodeFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: commented-out code ({} lines parse as {})",
            self.file, self.line, self.lines, self.language
        )
    }
}

/// Every block of commented-out code in `files`.
///
/// `files` are paths as the caller spelled them; a relative one resolves
/// against `working_dir`. A file the roster has no grammar for, a file with no
/// comment-block verdict for its language, and a file that cannot be read are
/// each skipped without a finding — the same silence
/// [`query_ast`](crate::ops::query_ast) keeps, and the reason the tool rule
/// narrows its `match` to the extensions the verdict covers rather than
/// relying on this op to say "not measured".
pub fn find_commented_code(working_dir: &Path, files: &[String]) -> Vec<CommentedCodeFinding> {
    files
        .iter()
        .filter_map(|file| findings_in_file(working_dir, file))
        .flatten()
        .collect()
}

/// The findings in one file, `None` when the file yields no verdict at all.
fn findings_in_file(working_dir: &Path, file: &str) -> Option<Vec<CommentedCodeFinding>> {
    let source = std::fs::read_to_string(working_dir.join(file)).ok()?;
    let blocks = commented_code_blocks(file, &source)?;
    Some(
        blocks
            .into_iter()
            .map(|block| CommentedCodeFinding {
                file: file.to_string(),
                line: block.line,
                lines: block.lines,
                language: block.language,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Rust file whose only defect is a commented-out function.
    const COMMENTED_OUT_FUNCTION_RS: &str = concat!(
        "//! A probe module.\n\n",
        "// fn disabled(limit: i32) -> i32 {\n",
        "//     let mut total = 0;\n",
        "//     for value in 0..limit {\n",
        "//         total += value;\n",
        "//     }\n",
        "//     total\n",
        "// }\n\n",
        "/// The live entry point.\n",
        "pub fn live() {}\n",
    );

    /// Write `contents` at `name` under a fresh temporary directory.
    fn workspace_with(name: &str, contents: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create a scratch workspace");
        std::fs::write(dir.path().join(name), contents).expect("write the probe file");
        dir
    }

    #[test]
    fn a_commented_out_function_is_reported_once() {
        let dir = workspace_with("probe.rs", COMMENTED_OUT_FUNCTION_RS);

        let findings = find_commented_code(dir.path(), &["probe.rs".to_string()]);

        assert_eq!(
            findings,
            vec![CommentedCodeFinding {
                file: "probe.rs".to_string(),
                line: 3,
                lines: 7,
                language: "rust",
            }]
        );
    }

    #[test]
    fn a_finding_renders_as_the_tool_rule_contract_line() {
        let finding = CommentedCodeFinding {
            file: "src/lib.rs".to_string(),
            line: 12,
            lines: 7,
            language: "rust",
        };

        assert_eq!(
            finding.to_string(),
            "src/lib.rs:12: commented-out code (7 lines parse as rust)"
        );
    }

    #[test]
    fn a_file_the_roster_does_not_claim_reports_nothing() {
        let dir = workspace_with("notes.txt", "// fn disabled() {}\n");

        assert!(find_commented_code(dir.path(), &["notes.txt".to_string()]).is_empty());
    }

    #[test]
    fn a_missing_file_reports_nothing_and_does_not_break_the_run() {
        let dir = tempfile::tempdir().expect("create a scratch workspace");

        assert!(find_commented_code(dir.path(), &["gone.rs".to_string()]).is_empty());
    }

    #[test]
    fn every_named_file_is_read() {
        let dir = workspace_with("first.rs", COMMENTED_OUT_FUNCTION_RS);
        std::fs::write(dir.path().join("second.rs"), COMMENTED_OUT_FUNCTION_RS)
            .expect("write the second probe file");

        let findings = find_commented_code(
            dir.path(),
            &["first.rs".to_string(), "second.rs".to_string()],
        );

        let files: Vec<&str> = findings.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(files, ["first.rs", "second.rs"]);
    }

    #[test]
    fn an_absolute_path_is_read_where_it_lies() {
        let dir = workspace_with("probe.rs", COMMENTED_OUT_FUNCTION_RS);
        let absolute = dir.path().join("probe.rs").to_string_lossy().to_string();

        let findings =
            find_commented_code(Path::new("/nonexistent"), std::slice::from_ref(&absolute));

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, absolute);
    }
}
