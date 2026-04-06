//! Health check registry for SwissArmyHammer tools
//!
//! This module provides a centralized collection of all tool health checks
//! that can be used by the `sah doctor` command.
//!
//! MCP tools implement the Doctorable trait via their registration, and
//! standalone components (like prompts) also implement Doctorable directly.

use swissarmyhammer_common::health::{Doctorable, HealthCheck};

use crate::mcp::tool_registry::ToolRegistry;
use crate::mcp::{
    register_file_tools, register_git_tools, register_kanban_tools, register_questions_tools,
    register_shell_tools, register_web_tools,
};

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

        // Check user prompts directory
        if let Some(home) = dirs::home_dir() {
            let user_prompts = home.join(".prompts");
            if user_prompts.exists() {
                let count = count_markdown_files(&user_prompts);
                checks.push(HealthCheck::ok(
                    "User prompts directory",
                    format!("Found {} prompts in {:?}", count, user_prompts),
                    cat,
                ));
            } else {
                checks.push(HealthCheck::ok(
                    "User prompts directory",
                    format!("Not found (optional): {:?}", user_prompts),
                    cat,
                ));
            }
        }

        // Check local prompts directory
        let local_prompts = std::path::PathBuf::from(".prompts");
        if local_prompts.exists() {
            let count = count_markdown_files(&local_prompts);
            checks.push(HealthCheck::ok(
                "Local prompts directory",
                format!("Found {} prompts in {:?}", count, local_prompts),
                cat,
            ));
        } else {
            checks.push(HealthCheck::ok(
                "Local prompts directory",
                format!("Not found (optional): {:?}", local_prompts),
                cat,
            ));
        }

        // Check YAML front matter parsing in all prompt directories
        let mut dirs_to_check = vec![local_prompts];
        if let Some(home) = dirs::home_dir() {
            dirs_to_check.push(home.join(".prompts"));
        }

        let mut yaml_errors = Vec::new();
        for dir in dirs_to_check {
            if !dir.exists() {
                continue;
            }

            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            {
                match std::fs::read_to_string(entry.path()) {
                    Ok(content) => {
                        if content.starts_with("---") {
                            let parts: Vec<&str> = content.splitn(3, "---").collect();
                            if parts.len() >= 3 {
                                if let Err(e) =
                                    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(parts[1])
                                {
                                    yaml_errors.push((entry.path().to_path_buf(), e.to_string()));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yaml_errors.push((
                            entry.path().to_path_buf(),
                            format!("Failed to read file: {}", e),
                        ));
                    }
                }
            }
        }

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

/// Count markdown files in a directory
fn count_markdown_files(path: &std::path::Path) -> usize {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .count()
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

    // Register all MCP tools (same as server does)
    register_file_tools(&mut tool_registry).await;
    register_git_tools(&mut tool_registry);
    register_shell_tools(&mut tool_registry);
    register_kanban_tools(&mut tool_registry);
    register_questions_tools(&mut tool_registry);
    register_web_tools(&mut tool_registry);

    // Register skill tools with a default library
    {
        use crate::mcp::tools::skill::register_skill_tools;
        use std::sync::Arc;
        use swissarmyhammer_prompts::PromptLibrary;
        use swissarmyhammer_skills::SkillLibrary;
        use tokio::sync::RwLock;

        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }
        let prompt_library = Arc::new(RwLock::new(PromptLibrary::default()));
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

        // Run with the temp dir as working directory so the local `.prompts` path resolves.
        let original_dir = std::env::current_dir().ok();
        std::env::set_current_dir(tmp.path()).unwrap();

        let checker = PromptHealthChecker;
        let checks = checker.run_health_checks();

        // Restore original directory.
        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }

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

        let original_dir = std::env::current_dir().ok();
        std::env::set_current_dir(tmp.path()).unwrap();

        let checker = PromptHealthChecker;
        let checks = checker.run_health_checks();

        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }

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

    #[tokio::test]
    async fn test_collect_all_health_checks() {
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
    async fn test_web_search_health_check_included() {
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
    async fn test_prompt_health_checks_included() {
        let checks = collect_all_health_checks().await;

        // Should have prompt-related checks
        let prompt_checks: Vec<_> = checks.iter().filter(|c| c.category == "prompts").collect();
        assert!(
            !prompt_checks.is_empty(),
            "Should have prompt health checks"
        );
    }

    #[tokio::test]
    async fn test_skill_health_checks_included() {
        let checks = collect_all_health_checks().await;

        // Should have skill-related checks from SkillTool
        let skill_checks: Vec<_> = checks
            .iter()
            .filter(|c| c.name.contains("Skill") || c.name.contains("skill"))
            .collect();
        assert!(
            !skill_checks.is_empty(),
            "Should have skill health checks from SkillTool"
        );
    }
}
