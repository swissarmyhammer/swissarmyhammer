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

use crate::ops::workspace_path::resolve_within;

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
/// `files` are paths as the caller spelled them, and every one of them is
/// resolved inside `working_dir` by
/// [`resolve_within`](crate::ops::workspace_path::resolve_within): a path that
/// climbs out of the workspace names no file this op will read. A file the
/// roster has no grammar for, a file with no comment-block verdict for its
/// language, and a file that cannot be read are each skipped without a
/// finding — the same silence [`query_ast`](crate::ops::query_ast) keeps, and
/// the reason the tool rule narrows its `match` to the extensions the verdict
/// covers rather than relying on this op to say "not measured".
pub fn find_commented_code(working_dir: &Path, files: &[&str]) -> Vec<CommentedCodeFinding> {
    files
        .iter()
        .filter_map(|file| findings_in_file(working_dir, file))
        .flatten()
        .collect()
}

/// The findings in one file, `None` when the file yields no verdict at all.
fn findings_in_file(working_dir: &Path, file: &str) -> Option<Vec<CommentedCodeFinding>> {
    let source = std::fs::read_to_string(resolve_within(working_dir, file)?).ok()?;
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

        let findings = find_commented_code(dir.path(), &["probe.rs"]);

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

        assert!(find_commented_code(dir.path(), &["notes.txt"]).is_empty());
    }

    #[test]
    fn a_missing_file_reports_nothing_and_does_not_break_the_run() {
        let dir = tempfile::tempdir().expect("create a scratch workspace");

        assert!(find_commented_code(dir.path(), &["gone.rs"]).is_empty());
    }

    #[test]
    fn every_named_file_is_read() {
        let dir = workspace_with("first.rs", COMMENTED_OUT_FUNCTION_RS);
        std::fs::write(dir.path().join("second.rs"), COMMENTED_OUT_FUNCTION_RS)
            .expect("write the second probe file");

        let findings = find_commented_code(dir.path(), &["first.rs", "second.rs"]);

        let files: Vec<&str> = findings.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(files, ["first.rs", "second.rs"]);
    }

    #[test]
    fn an_absolute_path_inside_the_working_directory_is_read() {
        let dir = workspace_with("probe.rs", COMMENTED_OUT_FUNCTION_RS);
        let absolute = dir.path().join("probe.rs").to_string_lossy().to_string();

        let findings = find_commented_code(dir.path(), &[absolute.as_str()]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, absolute);
    }

    /// A scratch directory holding a workspace and, beside it, a file the
    /// workspace must not reach.
    fn workspace_beside_an_outside_file() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("create a scratch directory");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir(&workspace).expect("create the workspace directory");
        std::fs::write(dir.path().join("outside.rs"), COMMENTED_OUT_FUNCTION_RS)
            .expect("write the file outside the workspace");
        (dir, workspace)
    }

    #[test]
    fn a_relative_path_that_climbs_out_of_the_working_directory_is_refused() {
        let (_dir, workspace) = workspace_beside_an_outside_file();

        assert!(find_commented_code(&workspace, &["../outside.rs"]).is_empty());
    }

    #[test]
    fn an_absolute_path_outside_the_working_directory_is_refused() {
        let (dir, workspace) = workspace_beside_an_outside_file();
        let outside = dir.path().join("outside.rs").to_string_lossy().to_string();

        assert!(find_commented_code(&workspace, &[outside.as_str()]).is_empty());
    }
}
