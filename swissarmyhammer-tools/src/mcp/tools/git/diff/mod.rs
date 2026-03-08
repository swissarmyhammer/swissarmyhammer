//! Semantic diff operation for the git tool
//!
//! Provides entity-level semantic diffing using sem-core. Compares code at the
//! level of functions, classes, and other semantic entities rather than raw lines.
//!
//! ## Modes
//!
//! 1. **Inline text**: Compare two code snippets directly (`left_text` + `right_text` + `language`)
//! 2. **File mode**: Compare files, optionally at different git refs (`left` and/or `right` with `file@ref` syntax)
//! 3. **Auto-detect**: No parameters -- detects dirty/staged files and diffs them

use serde::{Deserialize, Serialize};
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

use sem_core::git::bridge::GitBridge;
use sem_core::git::types::{FileChange, FileStatus};
use sem_core::model::change::SemanticChange;
use sem_core::model::identity::match_entities;
use sem_core::parser::differ::{compute_semantic_diff, DiffResult};
use sem_core::parser::plugins::create_default_registry;


/// Maps a language name to a file extension for sem-core plugin lookup.
///
/// The registry dispatches by file extension, so we need to construct a
/// synthetic file path with the right extension when doing inline text diffs.
///
/// # Arguments
///
/// * `language` - Language identifier (e.g., "rust", "typescript", "python")
///
/// # Returns
///
/// The corresponding file extension including the dot (e.g., ".rs", ".ts", ".py")
fn language_to_extension(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "rust" | "rs" => ".rs",
        "typescript" | "ts" => ".ts",
        "tsx" => ".tsx",
        "javascript" | "js" => ".js",
        "jsx" => ".jsx",
        "python" | "py" => ".py",
        "go" => ".go",
        "java" => ".java",
        "c" => ".c",
        "cpp" | "c++" | "cxx" => ".cpp",
        "ruby" | "rb" => ".rb",
        "csharp" | "c#" | "cs" => ".cs",
        "php" => ".php",
        "fortran" | "f90" => ".f90",
        "swift" => ".swift",
        "elixir" | "ex" => ".ex",
        "bash" | "sh" => ".sh",
        "json" => ".json",
        "yaml" | "yml" => ".yaml",
        "toml" => ".toml",
        "csv" => ".csv",
        "markdown" | "md" => ".md",
        "vue" => ".vue",
        _ => ".txt", // fallback
    }
}

/// Parses a `file@ref` spec into (file_path, Option<git_ref>).
///
/// If no `@` is present, the entire string is the file path with no ref.
/// If `@` is present, the part before is the file path and after is the git ref.
///
/// # Arguments
///
/// * `spec` - A string like "src/main.rs" or "src/main.rs@HEAD~1"
///
/// # Returns
///
/// A tuple of (file_path, optional git ref)
fn parse_file_ref(spec: &str) -> (&str, Option<&str>) {
    if let Some(at_pos) = spec.rfind('@') {
        // Avoid splitting on @ at position 0 (e.g. "@ref" with no path)
        if at_pos > 0 {
            let (path, rest) = spec.split_at(at_pos);
            let git_ref = &rest[1..]; // skip the '@'
            if !git_ref.is_empty() {
                return (path, Some(git_ref));
            }
        }
    }
    (spec, None)
}

/// Response structure for semantic diff results
#[derive(Debug, Serialize, Deserialize)]
pub struct DiffResponse {
    /// Summary counts of changes
    pub summary: DiffSummary,
    /// Individual semantic changes
    pub changes: Vec<ChangeEntry>,
}

/// Summary counts for a semantic diff
#[derive(Debug, Serialize, Deserialize)]
pub struct DiffSummary {
    pub files: usize,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub moved: usize,
    pub renamed: usize,
}

/// A single semantic change entry in the diff output.
///
/// Fields are ordered so that identifying info (what changed, where, how)
/// appears before content blobs, making the output scannable.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeEntry {
    pub change_type: String,
    pub entity_type: String,
    pub entity_name: String,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structural_change: Option<bool>,
    pub entity_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_content: Option<String>,
}

/// Converts a `SemanticChange` from sem-core into our output `ChangeEntry`.
fn to_change_entry(sc: &SemanticChange) -> ChangeEntry {
    ChangeEntry {
        change_type: sc.change_type.to_string(),
        entity_type: sc.entity_type.clone(),
        entity_name: sc.entity_name.clone(),
        file_path: sc.file_path.clone(),
        old_file_path: sc.old_file_path.clone(),
        structural_change: sc.structural_change,
        entity_id: sc.entity_id.clone(),
        before_content: sc.before_content.clone(),
        after_content: sc.after_content.clone(),
    }
}

/// Converts a `DiffResult` from sem-core into our `DiffResponse`.
fn diff_result_to_response(result: &DiffResult) -> DiffResponse {
    DiffResponse {
        summary: DiffSummary {
            files: result.file_count,
            added: result.added_count,
            modified: result.modified_count,
            deleted: result.deleted_count,
            moved: result.moved_count,
            renamed: result.renamed_count,
        },
        changes: result.changes.iter().map(to_change_entry).collect(),
    }
}

/// Operation metadata for getting a semantic diff
#[derive(Debug, Default)]
pub struct GetDiff;

static GET_DIFF_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("left")
        .description("File path or file@ref (e.g. 'src/main.rs@HEAD~1'). Used with 'right' for file-mode diffing.")
        .param_type(ParamType::String),
    ParamMeta::new("right")
        .description("File path or file@ref (e.g. 'src/main.rs'). Used with 'left' for file-mode diffing.")
        .param_type(ParamType::String),
    ParamMeta::new("left_text")
        .description("Inline source code for the 'before' side. Requires 'right_text' and 'language'.")
        .param_type(ParamType::String),
    ParamMeta::new("right_text")
        .description("Inline source code for the 'after' side. Requires 'left_text' and 'language'.")
        .param_type(ParamType::String),
    ParamMeta::new("language")
        .description("Language identifier for inline text mode (e.g. 'rust', 'typescript', 'python'). Required when using left_text/right_text.")
        .param_type(ParamType::String),
];

impl Operation for GetDiff {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "diff"
    }
    fn description(&self) -> &'static str {
        "Semantic diff at the entity level (functions, classes, etc.) using tree-sitter parsing"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_DIFF_PARAMS
    }
}

/// Executes the inline text diff mode.
///
/// Extracts semantic entities from both text snippets using the appropriate
/// language plugin, then matches entities to find additions, modifications,
/// deletions, renames, and moves.
///
/// # Arguments
///
/// * `left_text` - The "before" source code
/// * `right_text` - The "after" source code
/// * `language` - Language identifier for parser selection
///
/// # Returns
///
/// A JSON string containing the diff response, or an error message
pub fn execute_inline_diff(
    left_text: &str,
    right_text: &str,
    language: &str,
) -> Result<String, String> {
    let registry = create_default_registry();
    let ext = language_to_extension(language);
    let synthetic_path = format!("inline{ext}");

    let plugin = registry
        .get_plugin(&synthetic_path)
        .ok_or_else(|| format!("No parser plugin found for language '{language}'"))?;

    let before_entities = plugin.extract_entities(left_text, &synthetic_path);
    let after_entities = plugin.extract_entities(right_text, &synthetic_path);

    let sim_fn =
        |a: &sem_core::model::entity::SemanticEntity,
         b: &sem_core::model::entity::SemanticEntity|
         -> f64 { plugin.compute_similarity(a, b) };

    let match_result = match_entities(
        &before_entities,
        &after_entities,
        &synthetic_path,
        Some(&sim_fn),
        None,
        None,
    );

    // Count changes by type for summary
    let mut added = 0;
    let mut modified = 0;
    let mut deleted = 0;
    let mut moved = 0;
    let mut renamed = 0;
    for c in &match_result.changes {
        match c.change_type {
            sem_core::model::change::ChangeType::Added => added += 1,
            sem_core::model::change::ChangeType::Modified => modified += 1,
            sem_core::model::change::ChangeType::Deleted => deleted += 1,
            sem_core::model::change::ChangeType::Moved => moved += 1,
            sem_core::model::change::ChangeType::Renamed => renamed += 1,
        }
    }

    let response = DiffResponse {
        summary: DiffSummary {
            files: 1,
            added,
            modified,
            deleted,
            moved,
            renamed,
        },
        changes: match_result.changes.iter().map(to_change_entry).collect(),
    };

    serde_json::to_string_pretty(&response)
        .map_err(|e| format!("Failed to serialize diff response: {e}"))
}

/// Executes the file-mode diff.
///
/// Reads file contents from disk or from git refs (using `file@ref` syntax),
/// creates `FileChange` entries, and runs `compute_semantic_diff`.
///
/// # Arguments
///
/// * `left_spec` - Left side file spec (path or path@ref)
/// * `right_spec` - Right side file spec (path or path@ref)
/// * `working_dir` - The working directory (repo root)
///
/// # Returns
///
/// A JSON string containing the diff response, or an error message
pub fn execute_file_diff(
    left_spec: &str,
    right_spec: &str,
    working_dir: &std::path::Path,
) -> Result<String, String> {
    let registry = create_default_registry();

    let (left_path, left_ref) = parse_file_ref(left_spec);
    let (right_path, right_ref) = parse_file_ref(right_spec);

    // Read content for left side
    let left_content = read_file_content(left_path, left_ref, working_dir)?;

    // Read content for right side
    let right_content = read_file_content(right_path, right_ref, working_dir)?;

    // Always use right_path as the canonical file path for entity extraction.
    // When two different paths are provided, the user wants a content comparison,
    // not move/rename detection. Setting old_file_path would cause entity IDs to
    // diverge (they include the file path), preventing semantic matching.
    let file_change = FileChange {
        file_path: right_path.to_string(),
        status: FileStatus::Modified,
        old_file_path: None,
        before_content: Some(left_content),
        after_content: Some(right_content),
    };

    let diff_result = compute_semantic_diff(&[file_change], &registry, None, None);
    let response = diff_result_to_response(&diff_result);

    serde_json::to_string_pretty(&response)
        .map_err(|e| format!("Failed to serialize diff response: {e}"))
}

/// Executes the auto-detect diff mode.
///
/// Uses `GitBridge` to detect dirty/staged files in the repository and runs
/// semantic diff on them.
///
/// # Arguments
///
/// * `working_dir` - The working directory (repo root)
///
/// # Returns
///
/// A JSON string containing the diff response, or an error message
pub fn execute_auto_diff(working_dir: &std::path::Path) -> Result<String, String> {
    let registry = create_default_registry();

    let bridge = GitBridge::open(working_dir)
        .map_err(|e| format!("Failed to open git repository: {e}"))?;

    let (_scope, file_changes) = bridge
        .detect_and_get_files()
        .map_err(|e| format!("Failed to detect changed files: {e}"))?;

    let diff_result = compute_semantic_diff(&file_changes, &registry, None, None);
    let response = diff_result_to_response(&diff_result);

    serde_json::to_string_pretty(&response)
        .map_err(|e| format!("Failed to serialize diff response: {e}"))
}

/// Reads file content either from disk or from a git ref.
///
/// When `git_ref` is `None`, reads the file from disk relative to `working_dir`.
/// When `git_ref` is `Some`, uses `git show ref:path` via the GitBridge.
///
/// # Arguments
///
/// * `file_path` - Relative file path within the repository
/// * `git_ref` - Optional git ref (e.g., "HEAD", "HEAD~1", a commit SHA)
/// * `working_dir` - Repository root directory
///
/// # Returns
///
/// The file content as a string, or an error message
fn read_file_content(
    file_path: &str,
    git_ref: Option<&str>,
    working_dir: &std::path::Path,
) -> Result<String, String> {
    match git_ref {
        Some(refspec) => {
            // Read file content at a specific git ref via `git show ref:path`.
            // GitBridge doesn't expose read_blob_from_tree publicly, so we
            // shell out to git for this operation.
            let output = std::process::Command::new("git")
                .args(["show", &format!("{refspec}:{file_path}")])
                .current_dir(working_dir)
                .output()
                .map_err(|e| format!("Failed to run git show: {e}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "git show {refspec}:{file_path} failed: {stderr}"
                ));
            }

            String::from_utf8(output.stdout)
                .map_err(|e| format!("File content is not valid UTF-8: {e}"))
        }
        None => {
            // Read from disk
            let full_path = working_dir.join(file_path);
            std::fs::read_to_string(&full_path).map_err(|e| {
                format!(
                    "Failed to read file '{}': {e}",
                    full_path.display()
                )
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_ref_no_ref() {
        let (path, git_ref) = parse_file_ref("src/main.rs");
        assert_eq!(path, "src/main.rs");
        assert_eq!(git_ref, None);
    }

    #[test]
    fn test_parse_file_ref_with_ref() {
        let (path, git_ref) = parse_file_ref("src/main.rs@HEAD~1");
        assert_eq!(path, "src/main.rs");
        assert_eq!(git_ref, Some("HEAD~1"));
    }

    #[test]
    fn test_parse_file_ref_with_sha() {
        let (path, git_ref) = parse_file_ref("src/lib.rs@abc123");
        assert_eq!(path, "src/lib.rs");
        assert_eq!(git_ref, Some("abc123"));
    }

    #[test]
    fn test_parse_file_ref_no_path() {
        // Edge case: @ref with no path should treat whole thing as path
        let (path, git_ref) = parse_file_ref("@HEAD");
        assert_eq!(path, "@HEAD");
        assert_eq!(git_ref, None);
    }

    #[test]
    fn test_parse_file_ref_trailing_at() {
        let (path, git_ref) = parse_file_ref("src/main.rs@");
        assert_eq!(path, "src/main.rs@");
        assert_eq!(git_ref, None);
    }

    #[test]
    fn test_language_to_extension_rust() {
        assert_eq!(language_to_extension("rust"), ".rs");
        assert_eq!(language_to_extension("rs"), ".rs");
        assert_eq!(language_to_extension("Rust"), ".rs");
    }

    #[test]
    fn test_language_to_extension_typescript() {
        assert_eq!(language_to_extension("typescript"), ".ts");
        assert_eq!(language_to_extension("ts"), ".ts");
    }

    #[test]
    fn test_language_to_extension_unknown() {
        assert_eq!(language_to_extension("brainfuck"), ".txt");
    }

    #[test]
    fn test_inline_diff_modified_function() {
        let left = r#"fn process_data() {
    println!("hello");
}

fn other() {
    println!("other");
}
"#;
        let right = r#"fn process_data(x: i32) {
    println!("hello {}", x);
}

fn other() {
    println!("other");
}
"#;
        let result = execute_inline_diff(left, right, "rust").unwrap();
        let response: DiffResponse = serde_json::from_str(&result).unwrap();

        // process_data changed, other stayed the same
        assert_eq!(response.summary.modified, 1, "Expected 1 modified entity");
        assert_eq!(response.summary.added, 0);
        assert_eq!(response.summary.deleted, 0);

        let modified = response
            .changes
            .iter()
            .find(|c| c.change_type == "modified")
            .expect("Should have a modified change");
        assert_eq!(modified.entity_name, "process_data");
    }

    #[test]
    fn test_inline_diff_added_function() {
        let left = r#"fn existing() {
    println!("existing");
}
"#;
        let right = r#"fn existing() {
    println!("existing");
}

fn new_function() {
    println!("new");
}
"#;
        let result = execute_inline_diff(left, right, "rust").unwrap();
        let response: DiffResponse = serde_json::from_str(&result).unwrap();

        assert_eq!(response.summary.added, 1);
        assert_eq!(response.summary.modified, 0);

        let added = response
            .changes
            .iter()
            .find(|c| c.change_type == "added")
            .expect("Should have an added change");
        assert_eq!(added.entity_name, "new_function");
    }

    #[test]
    fn test_inline_diff_deleted_function() {
        let left = r#"fn keep_me() {
    println!("keep");
}

fn delete_me() {
    println!("delete");
}
"#;
        let right = r#"fn keep_me() {
    println!("keep");
}
"#;
        let result = execute_inline_diff(left, right, "rust").unwrap();
        let response: DiffResponse = serde_json::from_str(&result).unwrap();

        assert_eq!(response.summary.deleted, 1);
        assert_eq!(response.summary.modified, 0);

        let deleted = response
            .changes
            .iter()
            .find(|c| c.change_type == "deleted")
            .expect("Should have a deleted change");
        assert_eq!(deleted.entity_name, "delete_me");
    }

    #[test]
    fn test_inline_diff_typescript() {
        let left = r#"export function hello(): string {
    return "hello";
}
"#;
        let right = r#"export function hello(name: string): string {
    return `Hello, ${name}!`;
}
"#;
        let result = execute_inline_diff(left, right, "typescript").unwrap();
        let response: DiffResponse = serde_json::from_str(&result).unwrap();

        assert_eq!(response.summary.modified, 1);
        let modified = &response.changes[0];
        assert_eq!(modified.entity_name, "hello");
    }

    #[test]
    fn test_inline_diff_no_changes() {
        let code = r#"fn same() {
    println!("same");
}
"#;
        let result = execute_inline_diff(code, code, "rust").unwrap();
        let response: DiffResponse = serde_json::from_str(&result).unwrap();

        assert_eq!(response.summary.added, 0);
        assert_eq!(response.summary.modified, 0);
        assert_eq!(response.summary.deleted, 0);
        assert!(response.changes.is_empty());
    }
}
