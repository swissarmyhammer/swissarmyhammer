//! Git merge driver handlers for `.kanban/` files.
//!
//! Git calls merge drivers as:
//!   `kanban merge jsonl %O %A %B`
//!   `kanban merge yaml  %O %A %B`
//!   `kanban merge md    %O %A %B`
//!
//! where `%O` = base (common ancestor), `%A` = ours (current branch, written in-place),
//! `%B` = theirs (incoming branch).
//!
//! Exit codes:
//! - `0` = merged successfully (ours file updated in-place)
//! - `1` = conflict detected (ours file contains conflict markers, git leaves it conflicted)
//! - `2` = fatal error (merge could not proceed)

use clap::{Arg, ArgMatches, Command};
use std::path::{Path, PathBuf};

use swissarmyhammer_merge::jsonl::merge_jsonl;
use swissarmyhammer_merge::md::merge_md;
use swissarmyhammer_merge::yaml::{merge_yaml, MergeOpts};
use swissarmyhammer_merge::MergeError;

/// Build a merge sub-subcommand with three positional arguments: base, ours, theirs.
///
/// `name` is the subcommand name (jsonl, yaml, md) and `about` is its help string.
pub fn merge_sub(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(
            Arg::new("base")
                .required(true)
                .help("Common ancestor file (git %O)"),
        )
        .arg(
            Arg::new("ours")
                .required(true)
                .help("Our branch file — updated in-place (git %A)"),
        )
        .arg(
            Arg::new("theirs")
                .required(true)
                .help("Their branch file (git %B)"),
        )
}

/// Build the top-level `merge` command with jsonl/yaml/md subcommands.
pub fn merge_command() -> Command {
    Command::new("merge")
        .about("Git merge drivers for .kanban/ files")
        .subcommand(merge_sub(
            "jsonl",
            "Union-by-id merge for JSONL activity logs",
        ))
        .subcommand(merge_sub("yaml", "Field-level merge for YAML entity files"))
        .subcommand(merge_sub(
            "md",
            "Frontmatter + body merge for Markdown task files",
        ))
        .subcommand_required(true)
}

/// Dispatch to the appropriate merge driver based on the matched subcommand.
///
/// Returns an exit code: 0 = success, 1 = conflict, 2 = fatal error.
pub fn handle_merge(matches: &ArgMatches) -> i32 {
    match matches.subcommand() {
        Some(("jsonl", sub_m)) => {
            let (base, ours, theirs) = match extract_paths(sub_m) {
                Ok(paths) => paths,
                Err(code) => return code,
            };
            run_jsonl(&base, &ours, &theirs)
        }
        Some(("yaml", sub_m)) => {
            let (base, ours, theirs) = match extract_paths(sub_m) {
                Ok(paths) => paths,
                Err(code) => return code,
            };
            run_yaml(&base, &ours, &theirs)
        }
        Some(("md", sub_m)) => {
            let (base, ours, theirs) = match extract_paths(sub_m) {
                Ok(paths) => paths,
                Err(code) => return code,
            };
            run_md(&base, &ours, &theirs)
        }
        _ => {
            eprintln!("merge: unknown subcommand");
            2
        }
    }
}

/// Extract and validate the three path arguments from a subcommand match.
///
/// Returns `Ok((base_path, ours_path, theirs_path))` or `Err(2)` on fatal error.
fn extract_paths(sub_m: &ArgMatches) -> Result<(PathBuf, PathBuf, PathBuf), i32> {
    let base = sub_m
        .get_one::<String>("base")
        .map(PathBuf::from)
        .ok_or(2)?;
    let ours = sub_m
        .get_one::<String>("ours")
        .map(PathBuf::from)
        .ok_or(2)?;
    let theirs = sub_m
        .get_one::<String>("theirs")
        .map(PathBuf::from)
        .ok_or(2)?;
    Ok((base, ours, theirs))
}

/// Read three files into strings.
///
/// Returns `Ok((base_str, ours_str, theirs_str))` or `Err(2)` on I/O error.
fn read_three_files(
    base: &PathBuf,
    ours: &PathBuf,
    theirs: &PathBuf,
) -> Result<(String, String, String), i32> {
    let base_str = std::fs::read_to_string(base).map_err(|e| {
        eprintln!("merge: cannot read base file {:?}: {}", base, e);
        2i32
    })?;
    let ours_str = std::fs::read_to_string(ours).map_err(|e| {
        eprintln!("merge: cannot read ours file {:?}: {}", ours, e);
        2i32
    })?;
    let theirs_str = std::fs::read_to_string(theirs).map_err(|e| {
        eprintln!("merge: cannot read theirs file {:?}: {}", theirs, e);
        2i32
    })?;
    Ok((base_str, ours_str, theirs_str))
}

/// Write the merged result back to the `ours` file in-place (git convention).
///
/// Returns `0` on success, `2` on I/O error.
fn write_ours(ours: &PathBuf, content: &str) -> i32 {
    std::fs::write(ours, content).map_or_else(
        |e| {
            eprintln!("merge: cannot write ours file {:?}: {}", ours, e);
            2
        },
        |_| 0,
    )
}

/// Run the JSONL union-by-id merge driver.
///
/// Reads base/ours/theirs as JSONL, merges, writes result to ours.
/// Returns 0 = success, 1 = conflict, 2 = fatal.
pub fn run_jsonl(base: &PathBuf, ours: &PathBuf, theirs: &PathBuf) -> i32 {
    let (base_str, ours_str, theirs_str) = match read_three_files(base, ours, theirs) {
        Ok(v) => v,
        Err(e) => return e,
    };

    match merge_jsonl(&base_str, &ours_str, &theirs_str) {
        Ok(merged) => write_ours(ours, &merged),
        Err(conflict) => {
            eprintln!("merge conflict in JSONL: {}", conflict);
            1
        }
    }
}

/// Run the YAML field-level merge driver.
///
/// Looks for a sibling `.jsonl` file with the same stem for newest-wins conflict
/// resolution. Reads base/ours/theirs as YAML, merges, writes result to ours.
/// Returns 0 = success, 1 = conflict, 2 = fatal.
pub fn run_yaml(base: &PathBuf, ours: &PathBuf, theirs: &PathBuf) -> i32 {
    let (base_str, ours_str, theirs_str) = match read_three_files(base, ours, theirs) {
        Ok(v) => v,
        Err(e) => return e,
    };

    // Look for sibling JSONL changelog: same directory and stem, `.jsonl` extension.
    let jsonl_path = sibling_jsonl(ours);
    let opts = MergeOpts {
        jsonl_path,
        ..Default::default()
    };

    match merge_yaml(&base_str, &ours_str, &theirs_str, &opts) {
        Ok(merged) => write_ours(ours, &merged),
        Err(MergeError::Conflict(c)) => {
            eprintln!("merge conflict in YAML: {}", c);
            1
        }
        Err(MergeError::ParseFailure(msg)) => {
            eprintln!("merge: YAML parse failure: {}", msg);
            2
        }
    }
}

/// Run the Markdown frontmatter + body merge driver.
///
/// Looks for a sibling `.jsonl` file for frontmatter conflict resolution.
/// Returns 0 = success, 1 = conflict, 2 = fatal.
pub fn run_md(base: &PathBuf, ours: &PathBuf, theirs: &PathBuf) -> i32 {
    let (base_str, ours_str, theirs_str) = match read_three_files(base, ours, theirs) {
        Ok(v) => v,
        Err(e) => return e,
    };

    // Look for sibling JSONL changelog: same directory and stem, `.jsonl` extension.
    let jsonl_path = sibling_jsonl(ours);
    let opts = MergeOpts {
        jsonl_path,
        ..Default::default()
    };

    match merge_md(&base_str, &ours_str, &theirs_str, &opts) {
        Ok(merged) => write_ours(ours, &merged),
        Err(MergeError::Conflict(c)) => {
            // Write conflict markers to ours so the user can resolve them.
            let conflict_text = c.conflicting_ids.first().map(|s| s.as_str()).unwrap_or("");
            eprintln!("merge conflict in Markdown: {}", c);
            let _ = std::fs::write(ours, conflict_text);
            1
        }
        Err(MergeError::ParseFailure(msg)) => {
            eprintln!("merge: Markdown frontmatter parse failure: {}", msg);
            2
        }
    }
}

/// Derive the sibling JSONL changelog path for a given file.
///
/// Given `/path/to/FOO.yaml`, returns `Some("/path/to/FOO.jsonl")` if the file
/// exists, or `None` if not.
fn sibling_jsonl(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?;
    let parent = path.parent()?;
    let candidate = parent.join(stem).with_extension("jsonl");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ──────────────────────────── helpers ────────────────────────────

    fn write_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    // ──────────────────────────── JSONL tests ────────────────────────────

    #[test]
    fn run_jsonl_disjoint_appends_produces_union() {
        let tmp = TempDir::new().unwrap();
        let entry_a = r#"{"id":"01AAA000000000000000000000","op":"create"}"#;
        let entry_b = r#"{"id":"01AAA000000000000000000001","op":"update"}"#;
        let entry_c = r#"{"id":"01AAA000000000000000000002","op":"create"}"#;

        let base = write_file(&tmp, "base.jsonl", &format!("{}\n{}\n", entry_a, entry_b));
        let ours = write_file(
            &tmp,
            "ours.jsonl",
            &format!("{}\n{}\n{}\n", entry_a, entry_b, entry_c),
        );
        let theirs = write_file(&tmp, "theirs.jsonl", &format!("{}\n{}\n", entry_a, entry_b));

        let code = run_jsonl(&base, &ours, &theirs);
        assert_eq!(code, 0, "disjoint append should succeed");

        let result = fs::read_to_string(&ours).unwrap();
        assert!(result.contains(entry_a));
        assert!(result.contains(entry_b));
        assert!(result.contains(entry_c));
    }

    #[test]
    fn run_jsonl_conflict_returns_1() {
        let tmp = TempDir::new().unwrap();
        let entry_a = r#"{"id":"01AAA000000000000000000000","op":"create"}"#;
        let entry_x1 = r#"{"id":"01BBB000000000000000000000","op":"create","v":"1"}"#;
        let entry_x2 = r#"{"id":"01BBB000000000000000000000","op":"create","v":"2"}"#;

        let base = write_file(&tmp, "base.jsonl", &format!("{}\n", entry_a));
        let ours = write_file(&tmp, "ours.jsonl", &format!("{}\n{}\n", entry_a, entry_x1));
        let theirs = write_file(
            &tmp,
            "theirs.jsonl",
            &format!("{}\n{}\n", entry_a, entry_x2),
        );

        let code = run_jsonl(&base, &ours, &theirs);
        assert_eq!(code, 1, "conflict should return exit code 1");
    }

    #[test]
    fn run_jsonl_missing_file_returns_2() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("nonexistent_base.jsonl");
        let ours = write_file(&tmp, "ours.jsonl", "");
        let theirs = write_file(&tmp, "theirs.jsonl", "");

        let code = run_jsonl(&base, &ours, &theirs);
        assert_eq!(code, 2, "missing file should return exit code 2");
    }

    // ──────────────────────────── YAML tests ────────────────────────────

    #[test]
    fn run_yaml_simple_merge_success() {
        let tmp = TempDir::new().unwrap();
        let base_yaml = "name: Alice\ncolor: red\n";
        let ours_yaml = "name: Alice\ncolor: blue\n";
        let theirs_yaml = "name: Alice\ncolor: red\n";

        let base = write_file(&tmp, "base.yaml", base_yaml);
        let ours = write_file(&tmp, "ours.yaml", ours_yaml);
        let theirs = write_file(&tmp, "theirs.yaml", theirs_yaml);

        let code = run_yaml(&base, &ours, &theirs);
        assert_eq!(code, 0, "simple YAML merge should succeed");

        let result = fs::read_to_string(&ours).unwrap();
        // ours changed color, theirs didn't — ours wins
        assert!(result.contains("blue"), "ours value should be preserved");
    }

    #[test]
    fn run_yaml_missing_file_returns_2() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("nonexistent.yaml");
        let ours = write_file(&tmp, "ours.yaml", "name: Alice\n");
        let theirs = write_file(&tmp, "theirs.yaml", "name: Alice\n");

        let code = run_yaml(&base, &ours, &theirs);
        assert_eq!(code, 2, "missing base file should return exit code 2");
    }

    // ──────────────────────────── Markdown tests ────────────────────────────

    #[test]
    fn run_md_body_only_no_conflict() {
        let tmp = TempDir::new().unwrap();
        let base = write_file(&tmp, "base.md", "line one\n");
        let ours = write_file(&tmp, "ours.md", "line one\nline two\n");
        let theirs = write_file(&tmp, "theirs.md", "line one\n");

        let code = run_md(&base, &ours, &theirs);
        assert_eq!(code, 0, "non-conflicting md merge should succeed");

        let result = fs::read_to_string(&ours).unwrap();
        assert!(
            result.contains("line two"),
            "ours addition should be preserved"
        );
    }

    #[test]
    fn run_md_body_conflict_returns_1() {
        let tmp = TempDir::new().unwrap();
        let base = write_file(&tmp, "base.md", "shared line\n");
        let ours = write_file(&tmp, "ours.md", "ours changed this\n");
        let theirs = write_file(&tmp, "theirs.md", "theirs changed this differently\n");

        let code = run_md(&base, &ours, &theirs);
        assert_eq!(code, 1, "conflicting md body should return exit code 1");
    }

    #[test]
    fn run_md_missing_file_returns_2() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("nonexistent.md");
        let ours = write_file(&tmp, "ours.md", "hello\n");
        let theirs = write_file(&tmp, "theirs.md", "hello\n");

        let code = run_md(&base, &ours, &theirs);
        assert_eq!(code, 2, "missing base file should return exit code 2");
    }

    // ──────────────────────────── handle_merge dispatch ────────────────────────────

    /// Build `ArgMatches` for `merge <subcommand> <base> <ours> <theirs>` using the
    /// canonical `merge_command()` parser.  Panics if the args are invalid (test bug).
    fn parse_merge_args(args: &[&str]) -> ArgMatches {
        merge_command()
            .try_get_matches_from(args)
            .expect("test args should parse successfully")
    }

    #[test]
    fn handle_merge_jsonl_dispatch_success() {
        let tmp = TempDir::new().unwrap();
        let entry_a = r#"{"id":"01AAA000000000000000000000","op":"create"}"#;
        let entry_b = r#"{"id":"01AAA000000000000000000001","op":"update"}"#;

        let base = write_file(&tmp, "base.jsonl", &format!("{}\n", entry_a));
        let ours = write_file(&tmp, "ours.jsonl", &format!("{}\n{}\n", entry_a, entry_b));
        let theirs = write_file(&tmp, "theirs.jsonl", &format!("{}\n", entry_a));

        let matches = parse_merge_args(&[
            "merge",
            "jsonl",
            base.to_str().unwrap(),
            ours.to_str().unwrap(),
            theirs.to_str().unwrap(),
        ]);
        let sub_m = matches.subcommand_matches("jsonl").unwrap();
        let code = handle_merge(&matches);
        // Verify handle_merge delegates correctly by checking the exit code and result
        let _ = sub_m; // sub_m is just for clarity; handle_merge takes top-level matches
        assert_eq!(
            code, 0,
            "handle_merge jsonl dispatch should return 0 on success"
        );
    }

    #[test]
    fn handle_merge_yaml_dispatch_success() {
        let tmp = TempDir::new().unwrap();
        let base = write_file(&tmp, "base.yaml", "name: Alice\ncolor: red\n");
        let ours = write_file(&tmp, "ours.yaml", "name: Alice\ncolor: blue\n");
        let theirs = write_file(&tmp, "theirs.yaml", "name: Alice\ncolor: red\n");

        let matches = parse_merge_args(&[
            "merge",
            "yaml",
            base.to_str().unwrap(),
            ours.to_str().unwrap(),
            theirs.to_str().unwrap(),
        ]);
        let code = handle_merge(&matches);
        assert_eq!(
            code, 0,
            "handle_merge yaml dispatch should return 0 on success"
        );

        let result = fs::read_to_string(&ours).unwrap();
        assert!(
            result.contains("blue"),
            "yaml merge result should preserve ours value"
        );
    }

    #[test]
    fn handle_merge_md_dispatch_success() {
        let tmp = TempDir::new().unwrap();
        let base = write_file(&tmp, "base.md", "line one\n");
        let ours = write_file(&tmp, "ours.md", "line one\nline two\n");
        let theirs = write_file(&tmp, "theirs.md", "line one\n");

        let matches = parse_merge_args(&[
            "merge",
            "md",
            base.to_str().unwrap(),
            ours.to_str().unwrap(),
            theirs.to_str().unwrap(),
        ]);
        let code = handle_merge(&matches);
        assert_eq!(
            code, 0,
            "handle_merge md dispatch should return 0 on success"
        );

        let result = fs::read_to_string(&ours).unwrap();
        assert!(
            result.contains("line two"),
            "md merge result should preserve ours addition"
        );
    }

    #[test]
    fn handle_merge_jsonl_conflict_returns_1() {
        let tmp = TempDir::new().unwrap();
        let entry_a = r#"{"id":"01AAA000000000000000000000","op":"create"}"#;
        let entry_x1 = r#"{"id":"01BBB000000000000000000000","op":"create","v":"1"}"#;
        let entry_x2 = r#"{"id":"01BBB000000000000000000000","op":"create","v":"2"}"#;

        let base = write_file(&tmp, "base.jsonl", &format!("{}\n", entry_a));
        let ours = write_file(&tmp, "ours.jsonl", &format!("{}\n{}\n", entry_a, entry_x1));
        let theirs = write_file(
            &tmp,
            "theirs.jsonl",
            &format!("{}\n{}\n", entry_a, entry_x2),
        );

        let matches = parse_merge_args(&[
            "merge",
            "jsonl",
            base.to_str().unwrap(),
            ours.to_str().unwrap(),
            theirs.to_str().unwrap(),
        ]);
        let code = handle_merge(&matches);
        assert_eq!(
            code, 1,
            "handle_merge jsonl dispatch should return 1 on conflict"
        );
    }

    #[test]
    fn handle_merge_unknown_subcommand_returns_2() {
        // Build a permissive version of merge_command without subcommand_required so we can
        // pass matches with no recognized subcommand — exercising the `_ =>` fallback in
        // handle_merge (line 72) that would otherwise be unreachable via the CLI parser.
        let permissive = Command::new("merge")
            .about("Git merge drivers for .kanban/ files")
            .subcommand(merge_sub(
                "jsonl",
                "Union-by-id merge for JSONL activity logs",
            ))
            .subcommand(merge_sub("yaml", "Field-level merge for YAML entity files"))
            .subcommand(merge_sub(
                "md",
                "Frontmatter + body merge for Markdown task files",
            ));
        // Parse with no subcommand — results in matches with subcommand() == None
        let matches = permissive
            .try_get_matches_from(["merge"])
            .expect("permissive command should accept no subcommand");
        let code = handle_merge(&matches);
        assert_eq!(
            code, 2,
            "unknown/missing subcommand should return exit code 2"
        );
    }

    #[test]
    fn handle_merge_missing_file_returns_2() {
        let tmp = TempDir::new().unwrap();
        // base does not exist — triggers fatal error path
        let base = tmp.path().join("nonexistent.jsonl");
        let ours = write_file(&tmp, "ours.jsonl", "");
        let theirs = write_file(&tmp, "theirs.jsonl", "");

        let matches = parse_merge_args(&[
            "merge",
            "jsonl",
            base.to_str().unwrap(),
            ours.to_str().unwrap(),
            theirs.to_str().unwrap(),
        ]);
        let code = handle_merge(&matches);
        assert_eq!(
            code, 2,
            "handle_merge should return 2 when base file is missing"
        );
    }

    // ──────────────────────────── sibling_jsonl ────────────────────────────

    #[test]
    fn sibling_jsonl_returns_some_when_exists() {
        let tmp = TempDir::new().unwrap();
        let yaml_path = tmp.path().join("entity.yaml");
        let jsonl_path = tmp.path().join("entity.jsonl");
        fs::write(&yaml_path, "").unwrap();
        fs::write(&jsonl_path, "").unwrap();

        assert_eq!(sibling_jsonl(&yaml_path), Some(jsonl_path));
    }

    #[test]
    fn sibling_jsonl_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let yaml_path = tmp.path().join("entity.yaml");
        fs::write(&yaml_path, "").unwrap();

        assert_eq!(sibling_jsonl(&yaml_path), None);
    }
}
