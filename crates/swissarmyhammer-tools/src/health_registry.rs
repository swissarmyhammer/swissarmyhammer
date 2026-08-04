//! Health check registry for SwissArmyHammer tools
//!
//! This module provides a centralized collection of all tool health checks
//! that can be used by the `sah doctor` command.
//!
//! MCP tools implement the Doctorable trait via their registration, and
//! standalone components (like prompts) also implement Doctorable directly.

use swissarmyhammer_common::frontmatter::split_frontmatter_body;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};

use crate::mcp::tool_registry::ToolRegistry;
use crate::mcp::{
    register_code_context_tools, register_diagnostics_tools, register_file_tools,
    register_git_tools, register_kanban_tools, register_questions_tools, register_ralph_tools,
    register_review_tools, register_shell_tools, register_web_tools,
};

/// Directory name (relative to a user's home directory, or the current
/// working directory) that holds user-authored prompts.
const PROMPTS_DIR_NAME: &str = ".prompts";

/// Health check name for the user-level prompts directory.
const USER_PROMPTS_CHECK_NAME: &str = "User prompts directory";

/// Health check name for the local (project) prompts directory.
const LOCAL_PROMPTS_CHECK_NAME: &str = "Local prompts directory";

/// Health checker for prompt directories and YAML front matter
///
/// Prompts aren't an MCP tool — they're served via MCP's native Prompts
/// capability. This struct provides health checks for prompt configuration.
struct PromptHealthChecker;

impl Doctorable for PromptHealthChecker {
    fn name(&self) -> &str {
        "Prompts"
    }

    fn category(&self) -> &str {
        "prompts"
    }

    fn run_health_checks(&self) -> Vec<HealthCheck> {
        let mut checks = Vec::new();
        let cat = self.category();

        // Built-in prompts are always available
        checks.push(HealthCheck::ok(
            "Built-in prompts",
            "Built-in prompts are embedded in the binary",
            cat,
        ));

        // Resolve both prompt directory paths once; the per-directory checks
        // below and the YAML frontmatter scan both need the same paths.
        let home_prompts = dirs::home_dir().map(|home| home.join(PROMPTS_DIR_NAME));
        let local_prompts = std::path::PathBuf::from(PROMPTS_DIR_NAME);

        // Check user prompts directory
        if let Some(user_prompts) = &home_prompts {
            check_prompts_directory(USER_PROMPTS_CHECK_NAME, user_prompts, cat, &mut checks);
        }

        // Check local prompts directory
        check_prompts_directory(LOCAL_PROMPTS_CHECK_NAME, &local_prompts, cat, &mut checks);

        // Check YAML front matter parsing in all prompt directories
        let mut dirs_to_check = vec![local_prompts];
        dirs_to_check.extend(home_prompts);

        let yaml_errors: Vec<(std::path::PathBuf, String)> = dirs_to_check
            .iter()
            .flat_map(|dir| collect_yaml_errors_from_dir(dir))
            .collect();

        if yaml_errors.is_empty() {
            checks.push(HealthCheck::ok(
                "YAML parsing",
                "All prompt YAML front matter is valid",
                cat,
            ));
        } else {
            for (path, error) in yaml_errors {
                checks.push(HealthCheck::error(
                    format!("YAML parsing: {:?}", path.file_name().unwrap_or_default()),
                    error,
                    Some(format!("Fix the YAML syntax in {:?}", path)),
                    cat,
                ));
            }
        }

        checks
    }
}

/// Check whether a single prompts directory exists and push an OK
/// [`HealthCheck`] reporting either the markdown file count (found) or that
/// the directory is absent (optional).
fn check_prompts_directory(
    check_name: &str,
    dir: &std::path::Path,
    cat: &str,
    checks: &mut Vec<HealthCheck>,
) {
    if dir.exists() {
        let count = count_markdown_files(dir);
        checks.push(HealthCheck::ok(
            check_name,
            format!("Found {} prompts in {:?}", count, dir),
            cat,
        ));
    } else {
        checks.push(HealthCheck::ok(
            check_name,
            format!("Not found (optional): {:?}", dir),
            cat,
        ));
    }
}

/// Validate the YAML frontmatter of a single file.
///
/// Returns `None` when the file reads and its frontmatter (if any) parses.
/// Returns `Some(message)` when the file cannot be read, or its frontmatter
/// fails to parse.
fn validate_frontmatter_file(path: &std::path::Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => frontmatter_yaml_error(&content),
        Err(e) => Some(format!("Failed to read file: {}", e)),
    }
}

/// Iterate over every markdown file within a directory, recursively.
///
/// A file counts as markdown when its extension is `md`, matched
/// case-insensitively (so `.md`, `.MD`, and `.Md` all match). Shared by
/// [`count_markdown_files`] and [`collect_yaml_errors_from_dir`] so the two
/// can never drift on what counts as a markdown file.
fn iter_markdown_files(dir: &std::path::Path) -> impl Iterator<Item = walkdir::DirEntry> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
}

/// Collect YAML frontmatter errors from every markdown file in a directory.
///
/// Returns an empty list when the directory does not exist. Each entry pairs
/// the offending file's path with its error message.
fn collect_yaml_errors_from_dir(dir: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
    if !dir.exists() {
        return Vec::new();
    }

    iter_markdown_files(dir)
        .filter_map(|entry| {
            validate_frontmatter_file(entry.path())
                .map(|error| (entry.path().to_path_buf(), error))
        })
        .collect()
}

/// Report the YAML error in a markdown file's frontmatter block
///
/// [`split_frontmatter_body`] finds the block between two lines of exactly
/// three hyphens. Returns `None` when the text carries no such block -- a
/// first line of `----` or `---x` opens nothing -- and when the block's YAML
/// parses. Returns `Some(message)` with the parse error otherwise.
fn frontmatter_yaml_error(content: &str) -> Option<String> {
    let (frontmatter, _body) = split_frontmatter_body(content)?;
    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(frontmatter)
        .err()
        .map(|e| e.to_string())
}

/// Count markdown files in a directory
fn count_markdown_files(path: &std::path::Path) -> usize {
    iter_markdown_files(path).count()
}

/// Collect all health checks from MCP tools and standalone components
///
/// Iterates over all registered MCP tools and standalone Doctorable
/// components to collect their health checks. Called by `sah doctor`.
///
/// # Returns
///
/// * `Vec<HealthCheck>` - All health checks from all registered components
pub async fn collect_all_health_checks() -> Vec<HealthCheck> {
    // Create MCP tool registry and register all tools
    let mut tool_registry = ToolRegistry::new();

    // Register every MCP tool group (same set the server registers in
    // `register_all_tools`) so `sah doctor` enumerates all tools, each
    // surfacing at least one OK line via the Doctorable default.
    register_file_tools(&mut tool_registry);
    register_git_tools(&mut tool_registry);
    register_shell_tools(&mut tool_registry);
    register_kanban_tools(&mut tool_registry);
    register_questions_tools(&mut tool_registry);
    register_web_tools(&mut tool_registry);
    register_code_context_tools(&mut tool_registry);
    register_ralph_tools(&mut tool_registry);
    register_review_tools(&mut tool_registry);
    register_diagnostics_tools(&mut tool_registry);

    // Register tools that need libraries (skill, agent) with default
    // libraries built here.
    {
        use crate::mcp::tools::agent::register_agent_tools;
        use crate::mcp::tools::skill::register_skill_tools;
        use std::sync::Arc;
        use swissarmyhammer_agents::AgentLibrary;
        use swissarmyhammer_skills::SkillLibrary;
        use swissarmyhammer_templating::TemplateLibrary;
        use tokio::sync::RwLock;

        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }
        let prompt_library = Arc::new(RwLock::new(TemplateLibrary::default()));

        let agent_library = Arc::new(RwLock::new(AgentLibrary::new()));
        {
            let mut lib = agent_library.write().await;
            lib.load_defaults();
        }
        register_agent_tools(&mut tool_registry, agent_library, prompt_library.clone());
        register_skill_tools(&mut tool_registry, library, prompt_library);
    }

    // Collect health checks from all MCP tools
    let mut all_checks = Vec::new();
    for tool in tool_registry.iter_tools() {
        if Doctorable::is_applicable(tool) {
            all_checks.extend(tool.run_health_checks());
        }
    }

    // Collect health checks from standalone components (not MCP tools)
    let prompt_checker = PromptHealthChecker;
    if prompt_checker.is_applicable() {
        all_checks.extend(prompt_checker.run_health_checks());
    }

    all_checks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Test that PromptHealthChecker reports its name and category correctly.
    #[test]
    fn test_prompt_health_checker_name_and_category() {
        let checker = PromptHealthChecker;
        assert_eq!(checker.name(), "Prompts");
        assert_eq!(checker.category(), "prompts");
    }

    /// Test that PromptHealthChecker is always applicable.
    #[test]
    fn test_prompt_health_checker_is_applicable() {
        let checker = PromptHealthChecker;
        assert!(checker.is_applicable());
    }

    /// Test that run_health_checks covers the local prompts "found" branch
    /// by creating a temp directory with markdown files and running with it
    /// as the current directory.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_prompt_health_checker_with_local_prompts_directory() {
        let tmp = TempDir::new().unwrap();
        let prompts_dir = tmp.path().join(".prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        // Create two plain markdown files (no YAML frontmatter).
        fs::write(prompts_dir.join("a.md"), "# Title\n\nSome content.").unwrap();
        fs::write(prompts_dir.join("b.md"), "# Another\n\nMore content.").unwrap();

        // Run with the temp dir as working directory so the local `.prompts`
        // path resolves. The RAII guard restores the original working directory
        // on drop, even if an assertion below panics.
        let checks = {
            use swissarmyhammer_common::test_utils::CurrentDirGuard;
            let _cwd_guard = CurrentDirGuard::new(tmp.path())
                .expect("Failed to pin working directory to the isolated temp dir");
            let checker = PromptHealthChecker;
            checker.run_health_checks()
        };

        // All checks should succeed (plain markdown, no YAML errors).
        for check in &checks {
            assert_ne!(
                check.status,
                swissarmyhammer_common::health::HealthStatus::Error,
                "Unexpected error in check '{}': {}",
                check.name,
                check.message
            );
        }

        // Should have a "Local prompts directory" check with count information.
        let local_check = checks.iter().find(|c| c.name == "Local prompts directory");
        assert!(
            local_check.is_some(),
            "Should have Local prompts directory check"
        );
        let msg = &local_check.unwrap().message;
        assert!(
            msg.contains("Found 2 prompts"),
            "Should report 2 prompts, got: {}",
            msg
        );
    }

    /// Test that PromptHealthChecker detects YAML parsing errors when a prompt file
    /// has invalid YAML frontmatter.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_prompt_health_checker_detects_yaml_errors() {
        let tmp = TempDir::new().unwrap();
        let prompts_dir = tmp.path().join(".prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        // Write a prompt file with invalid YAML frontmatter.
        fs::write(
            prompts_dir.join("bad.md"),
            "---\ntitle: [unclosed bracket\n---\n# Content",
        )
        .unwrap();

        // The RAII guard restores the original working directory on drop,
        // even if an assertion below panics.
        let checks = {
            use swissarmyhammer_common::test_utils::CurrentDirGuard;
            let _cwd_guard = CurrentDirGuard::new(tmp.path())
                .expect("Failed to pin working directory to the isolated temp dir");
            let checker = PromptHealthChecker;
            checker.run_health_checks()
        };

        // Should have at least one error check for the bad YAML.
        let error_checks: Vec<_> = checks
            .iter()
            .filter(|c| {
                c.status == swissarmyhammer_common::health::HealthStatus::Error
                    && c.name.contains("YAML parsing")
            })
            .collect();
        assert!(
            !error_checks.is_empty(),
            "Should detect YAML parsing error in bad.md"
        );
    }

    /// Test that a first line which only begins with three hyphens is not
    /// read as a frontmatter delimiter, so the file reports no YAML error.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_prompt_health_checker_ignores_first_line_that_only_begins_with_hyphens() {
        let tmp = TempDir::new().unwrap();
        let prompts_dir = tmp.path().join(".prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        // The first line starts with three hyphens but carries more text, so
        // this file has no frontmatter block and its text is plain markdown.
        fs::write(
            prompts_dir.join("notfm.md"),
            "---x\ntitle: [unclosed bracket\n---\n# Content",
        )
        .unwrap();

        // The RAII guard restores the original working directory on drop,
        // even if an assertion below panics.
        let checks = {
            use swissarmyhammer_common::test_utils::CurrentDirGuard;
            let _cwd_guard = CurrentDirGuard::new(tmp.path())
                .expect("Failed to pin working directory to the isolated temp dir");
            let checker = PromptHealthChecker;
            checker.run_health_checks()
        };

        let reported: Vec<&str> = checks
            .iter()
            .filter(|c| c.name.contains("notfm.md"))
            .map(|c| c.message.as_str())
            .collect();
        assert!(
            reported.is_empty(),
            "notfm.md has no frontmatter, so it should report no YAML error, got: {:?}",
            reported
        );
    }

    /// `iter_markdown_files` (and, through it, `count_markdown_files` and
    /// `collect_yaml_errors_from_dir`) must match markdown extensions
    /// case-insensitively, so `.MD` and `.Md` files aren't silently excluded
    /// from prompt counts or YAML frontmatter validation.
    #[test]
    fn test_markdown_files_are_matched_case_insensitively() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.md"), "content").unwrap();
        fs::write(tmp.path().join("b.MD"), "content").unwrap();
        fs::write(tmp.path().join("c.Md"), "content").unwrap();
        fs::write(tmp.path().join("d.txt"), "content").unwrap();

        assert_eq!(
            count_markdown_files(tmp.path()),
            3,
            "count_markdown_files should match .md, .MD, and .Md case-insensitively"
        );

        // A .MD file with invalid frontmatter must still surface as a YAML
        // error — proving collect_yaml_errors_from_dir also matches
        // case-insensitively via the same shared iter_markdown_files helper.
        fs::write(
            tmp.path().join("bad.MD"),
            "---\ntitle: [unclosed bracket\n---\n# Content",
        )
        .unwrap();
        let errors = collect_yaml_errors_from_dir(tmp.path());
        assert!(
            errors
                .iter()
                .any(|(path, _)| path.file_name().unwrap() == "bad.MD"),
            "collect_yaml_errors_from_dir should validate .MD files too, got: {:?}",
            errors
        );
    }

    /// `collect_all_health_checks` calls `register_shell_tools`, which
    /// registers the real `ShellExecuteTool::new()` — the constructor that
    /// roots its state under the process CWD (the crate directory under
    /// `cargo nextest`). A `CurrentDirGuard` pins the CWD to a throwaway
    /// temp dir for the call so these tests don't leave a `.shell` directory
    /// behind in the crate source tree. `#[serial_test::serial(cwd)]` keeps
    /// this in step with the other CWD-mutating tests in this file.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_collect_all_health_checks() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let cwd_dir = TempDir::new().expect("temp dir for isolated shell state");
        let _cwd_guard = CurrentDirGuard::new(cwd_dir.path()).expect("chdir guard");
        let checks = collect_all_health_checks().await;

        // Should have at least some checks (web_search provides Chrome check)
        assert!(!checks.is_empty());

        // All checks should have proper fields
        for check in &checks {
            assert!(!check.category.is_empty());
            assert!(!check.name.is_empty());
            assert!(!check.message.is_empty());
        }
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_web_search_health_check_included() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let cwd_dir = TempDir::new().expect("temp dir for isolated shell state");
        let _cwd_guard = CurrentDirGuard::new(cwd_dir.path()).expect("chdir guard");
        let checks = collect_all_health_checks().await;

        // Should have a Brave Search check from web tool
        let brave_check = checks
            .iter()
            .find(|c| c.name.contains("Brave") && c.category == "tools");
        assert!(
            brave_check.is_some(),
            "Should have Brave Search check from web tool. Checks: {:?}",
            checks
                .iter()
                .map(|c| format!("{}/{}", c.category, c.name))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_all_tool_groups_enumerated() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let cwd_dir = TempDir::new().expect("temp dir for isolated shell state");
        let _cwd_guard = CurrentDirGuard::new(cwd_dir.path()).expect("chdir guard");
        let checks = collect_all_health_checks().await;

        // Every registered tool group should surface at least one check.
        // code_context, ralph, and agent were previously omitted from the
        // hand-picked subset. Ralph inherits the default OK check (name ==
        // its Doctorable name "Ralph"); code_context and agent contribute
        // their own checks under their category, so assert via category +
        // representative names.
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();

        // Ralph has no special checks, so the default OK check carries its name.
        assert!(
            names.contains(&"Ralph"),
            "Expected default OK check named 'Ralph', got names: {:?}",
            names
        );

        // code_context contributes an LSP-related check (e.g. "LSP servers"
        // when no project type is detected). Previously this group was not
        // registered at all.
        assert!(
            names.iter().any(|n| n.contains("LSP")),
            "Expected a code_context LSP health check, got names: {:?}",
            names
        );

        // agent contributes an "Agent library" check.
        assert!(
            names.iter().any(|n| n.contains("Agent library")),
            "Expected an agent library health check, got names: {:?}",
            names
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_review_validators_health_check_included() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let cwd_dir = TempDir::new().expect("temp dir for isolated shell state");
        let _cwd_guard = CurrentDirGuard::new(cwd_dir.path()).expect("chdir guard");
        let checks = collect_all_health_checks().await;

        // The review tool overrides the blanket OK default to lint validators;
        // its check surfaces under the "validators" category named "Validators"
        // (or "Validator <name/path>" for each problem). Confirm the review tool
        // is enumerated and its validators check appears.
        let validators_checks: Vec<_> = checks
            .iter()
            .filter(|c| c.category == "validators")
            .collect();
        assert!(
            !validators_checks.is_empty(),
            "the review tool's validators check must surface in `sah doctor`, got: {:?}",
            checks
                .iter()
                .map(|c| format!("{}/{}", c.category, c.name))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_prompt_health_checks_included() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let cwd_dir = TempDir::new().expect("temp dir for isolated shell state");
        let _cwd_guard = CurrentDirGuard::new(cwd_dir.path()).expect("chdir guard");
        let checks = collect_all_health_checks().await;

        // Should have prompt-related checks
        let prompt_checks: Vec<_> = checks.iter().filter(|c| c.category == "prompts").collect();
        assert!(
            !prompt_checks.is_empty(),
            "Should have prompt health checks"
        );
    }
}
