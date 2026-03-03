//! Project detection operation for discovering project types at runtime
//!
//! This operation scans the filesystem to detect project types (Rust, Node.js, Python, etc.)
//! and returns project metadata with language-specific guidelines. It does NOT require
//! a tree-sitter workspace/index — it's a pure filesystem scan.

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use crate::mcp::tools::treesitter::shared::resolve_workspace_path;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use swissarmyhammer_project_detection::{detect_projects, DetectedProject, ProjectType};

/// Operation metadata for project detection
#[derive(Debug, Default)]
pub struct DetectProjects;

static DETECT_PROJECTS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("path")
        .description("Root path to search for projects (default: current directory)")
        .param_type(ParamType::String),
    ParamMeta::new("max_depth")
        .description("Maximum directory depth to search (default: 3)")
        .param_type(ParamType::Integer),
    ParamMeta::new("include_guidelines")
        .description("Include language-specific guidelines in output (default: true)")
        .param_type(ParamType::Boolean),
];

impl Operation for DetectProjects {
    fn verb(&self) -> &'static str {
        "detect"
    }
    fn noun(&self) -> &'static str {
        "projects"
    }
    fn description(&self) -> &'static str {
        "Detect project types in the workspace and return language-specific guidelines"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        DETECT_PROJECTS_PARAMS
    }
}

#[derive(Deserialize, Default)]
struct DetectRequest {
    path: Option<String>,
    max_depth: Option<usize>,
    include_guidelines: Option<bool>,
}

// Include guideline files at compile time
static GUIDELINES_RUST: &str = include_str!("../../../../../../builtin/_partials/project-types/rust.md");
static GUIDELINES_NODEJS: &str =
    include_str!("../../../../../../builtin/_partials/project-types/nodejs.md");
static GUIDELINES_PYTHON: &str =
    include_str!("../../../../../../builtin/_partials/project-types/python.md");
static GUIDELINES_GO: &str = include_str!("../../../../../../builtin/_partials/project-types/go.md");
static GUIDELINES_JAVA_MAVEN: &str =
    include_str!("../../../../../../builtin/_partials/project-types/java-maven.md");
static GUIDELINES_JAVA_GRADLE: &str =
    include_str!("../../../../../../builtin/_partials/project-types/java-gradle.md");
static GUIDELINES_CSHARP: &str =
    include_str!("../../../../../../builtin/_partials/project-types/csharp.md");
static GUIDELINES_CMAKE: &str =
    include_str!("../../../../../../builtin/_partials/project-types/cmake.md");
static GUIDELINES_MAKEFILE: &str =
    include_str!("../../../../../../builtin/_partials/project-types/makefile.md");
static GUIDELINES_FLUTTER: &str =
    include_str!("../../../../../../builtin/_partials/project-types/flutter.md");

/// Get the guideline content for a project type
fn guidelines_for_type(project_type: ProjectType) -> &'static str {
    match project_type {
        ProjectType::Rust => GUIDELINES_RUST,
        ProjectType::NodeJs => GUIDELINES_NODEJS,
        ProjectType::Python => GUIDELINES_PYTHON,
        ProjectType::Go => GUIDELINES_GO,
        ProjectType::JavaMaven => GUIDELINES_JAVA_MAVEN,
        ProjectType::JavaGradle => GUIDELINES_JAVA_GRADLE,
        ProjectType::CSharp => GUIDELINES_CSHARP,
        ProjectType::CMake => GUIDELINES_CMAKE,
        ProjectType::Makefile => GUIDELINES_MAKEFILE,
        ProjectType::Flutter => GUIDELINES_FLUTTER,
    }
}

/// Strip YAML frontmatter from markdown content.
///
/// Frontmatter is delimited by `---` at the start of the content.
pub fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content;
    }
    // Find the closing ---
    if let Some(end_pos) = trimmed[3..].find("\n---") {
        // Skip past closing --- and the newline after it
        let after_frontmatter = &trimmed[3 + end_pos + 4..];
        after_frontmatter.trim_start_matches('\n')
    } else {
        content
    }
}

/// Get a display name for a project type
fn project_type_name(pt: ProjectType) -> &'static str {
    match pt {
        ProjectType::Rust => "Rust",
        ProjectType::NodeJs => "Node.js",
        ProjectType::Python => "Python",
        ProjectType::Go => "Go",
        ProjectType::JavaMaven => "Java (Maven)",
        ProjectType::JavaGradle => "Java (Gradle)",
        ProjectType::CSharp => "C# / .NET",
        ProjectType::CMake => "CMake",
        ProjectType::Makefile => "Makefile",
        ProjectType::Flutter => "Flutter",
    }
}

/// A stable string key for deduplication (matches serde rename)
fn project_type_key(pt: ProjectType) -> &'static str {
    match pt {
        ProjectType::Rust => "rust",
        ProjectType::NodeJs => "nodejs",
        ProjectType::Python => "python",
        ProjectType::Go => "go",
        ProjectType::JavaMaven => "java-maven",
        ProjectType::JavaGradle => "java-gradle",
        ProjectType::CSharp => "csharp",
        ProjectType::CMake => "cmake",
        ProjectType::Makefile => "makefile",
        ProjectType::Flutter => "flutter",
    }
}

/// Make a path relative to a root, returning the relative portion.
/// If the path is not under root, returns it as-is.
fn make_relative(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => {
            let s = rel.to_string_lossy().to_string();
            if s.is_empty() {
                ".".to_string()
            } else {
                s
            }
        }
        Err(_) => path.to_string_lossy().to_string(),
    }
}

/// Format detected projects as markdown output
fn format_detected_projects(
    projects: &[DetectedProject],
    root: &Path,
    include_guidelines: bool,
) -> String {
    if projects.is_empty() {
        return "## No Projects Detected\n\n\
                No standard project marker files (Cargo.toml, package.json, go.mod, etc.) \
                were found in the directory tree."
            .to_string();
    }

    let mut output = format!("## Detected Project Types\n\nFound {} project(s):\n\n", projects.len());

    for (i, project) in projects.iter().enumerate() {
        let name = project_type_name(project.project_type);
        let rel_path = make_relative(&project.path, root);
        output.push_str(&format!(
            "### {}. {} Project\n\n**Location:** `{}`\n**Markers:** {}\n",
            i + 1,
            name,
            rel_path,
            project.marker_files.join(", ")
        ));

        if let Some(ref ws) = project.workspace_info {
            if ws.is_root {
                output.push_str(&format!(
                    "**Workspace:** Yes ({} members)\n",
                    ws.members.len()
                ));
                if !ws.members.is_empty() {
                    output.push_str(&format!("**Members:** {}\n", ws.members.join(", ")));
                }
            }
        }

        output.push('\n');
    }

    if include_guidelines {
        // Collect unique project types (deduplicate)
        let mut seen = BTreeSet::new();
        let unique_types: Vec<ProjectType> = projects
            .iter()
            .filter(|p| seen.insert(project_type_key(p.project_type)))
            .map(|p| p.project_type)
            .collect();

        if !unique_types.is_empty() {
            output.push_str("## Project Guidelines\n\n");
            for pt in unique_types {
                let raw = guidelines_for_type(pt);
                let stripped = strip_frontmatter(raw);
                output.push_str(stripped);
                output.push_str("\n\n");
            }
        }
    }

    output
}

/// Execute project detection
pub async fn execute_detect(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: DetectRequest = BaseToolImpl::parse_arguments(arguments)?;
    let root_path = resolve_workspace_path(request.path.as_ref(), context);
    let max_depth = request.max_depth.unwrap_or(3);
    let include_guidelines = request.include_guidelines.unwrap_or(true);

    tracing::debug!(
        "Detecting projects at {:?} with max_depth={}",
        root_path,
        max_depth
    );

    let projects = detect_projects(&root_path, Some(max_depth)).map_err(|e| {
        McpError::internal_error(format!("Failed to detect projects: {}", e), None)
    })?;

    // Use the canonicalized root for relative path computation
    let canonical_root = root_path.canonicalize().unwrap_or(root_path);
    let output = format_detected_projects(&projects, &canonical_root, include_guidelines);

    Ok(BaseToolImpl::create_success_response(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_operation_metadata() {
        let op = DetectProjects;
        assert_eq!(op.verb(), "detect");
        assert_eq!(op.noun(), "projects");
        assert_eq!(op.op_string(), "detect projects");
        assert!(!op.description().is_empty());
    }

    #[test]
    fn test_operation_parameters() {
        let op = DetectProjects;
        let params = op.parameters();
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].name, "path");
        assert_eq!(params[1].name, "max_depth");
        assert_eq!(params[2].name, "include_guidelines");
    }

    #[test]
    fn test_strip_frontmatter_with_frontmatter() {
        let content = "---\ntitle: Test\ndescription: A test\npartial: true\n---\n\n### Hello\n\nWorld";
        let result = strip_frontmatter(content);
        assert_eq!(result, "### Hello\n\nWorld");
    }

    #[test]
    fn test_strip_frontmatter_without_frontmatter() {
        let content = "### Hello\n\nWorld";
        let result = strip_frontmatter(content);
        assert_eq!(result, "### Hello\n\nWorld");
    }

    #[test]
    fn test_strip_frontmatter_empty() {
        let result = strip_frontmatter("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_strip_frontmatter_only_opening() {
        let content = "---\ntitle: Test\nno closing delimiter";
        let result = strip_frontmatter(content);
        // No closing ---, returns original
        assert_eq!(result, content);
    }

    #[test]
    fn test_make_relative_under_root() {
        let root = Path::new("/workspace/project");
        let path = Path::new("/workspace/project/src/main.rs");
        assert_eq!(make_relative(path, root), "src/main.rs");
    }

    #[test]
    fn test_make_relative_same_as_root() {
        let root = Path::new("/workspace/project");
        let path = Path::new("/workspace/project");
        assert_eq!(make_relative(path, root), ".");
    }

    #[test]
    fn test_make_relative_outside_root() {
        let root = Path::new("/workspace/project");
        let path = Path::new("/other/location");
        assert_eq!(make_relative(path, root), "/other/location");
    }

    #[test]
    fn test_project_type_name_all_types() {
        // Verify all project types have display names
        let types = [
            ProjectType::Rust,
            ProjectType::NodeJs,
            ProjectType::Python,
            ProjectType::Go,
            ProjectType::JavaMaven,
            ProjectType::JavaGradle,
            ProjectType::CSharp,
            ProjectType::CMake,
            ProjectType::Makefile,
            ProjectType::Flutter,
        ];
        for pt in types {
            let name = project_type_name(pt);
            assert!(!name.is_empty(), "Project type {:?} should have a name", pt);
        }
    }

    #[test]
    fn test_all_project_types_have_guidelines() {
        let types = [
            ProjectType::Rust,
            ProjectType::NodeJs,
            ProjectType::Python,
            ProjectType::Go,
            ProjectType::JavaMaven,
            ProjectType::JavaGradle,
            ProjectType::CSharp,
            ProjectType::CMake,
            ProjectType::Makefile,
            ProjectType::Flutter,
        ];
        for pt in types {
            let guidelines = guidelines_for_type(pt);
            assert!(
                !guidelines.is_empty(),
                "Project type {:?} should have guidelines",
                pt
            );
            // Verify frontmatter stripping works
            let stripped = strip_frontmatter(guidelines);
            assert!(
                !stripped.starts_with("---"),
                "Guidelines for {:?} should not start with frontmatter after stripping",
                pt
            );
        }
    }

    #[test]
    fn test_format_no_projects() {
        let output = format_detected_projects(&[], Path::new("/root"), true);
        assert!(output.contains("No Projects Detected"));
    }

    #[test]
    fn test_format_with_relative_paths() {
        let root = Path::new("/workspace");
        let projects = vec![DetectedProject {
            path: "/workspace/backend".into(),
            project_type: ProjectType::Rust,
            marker_files: vec!["Cargo.toml".to_string()],
            workspace_info: None,
        }];

        let output = format_detected_projects(&projects, root, false);
        assert!(output.contains("`backend`"), "Path should be relative");
        assert!(
            !output.contains("/workspace/backend"),
            "Should not contain absolute path"
        );
    }

    #[test]
    fn test_format_with_guidelines() {
        let root = Path::new("/workspace");
        let projects = vec![DetectedProject {
            path: "/workspace".into(),
            project_type: ProjectType::Rust,
            marker_files: vec!["Cargo.toml".to_string()],
            workspace_info: None,
        }];

        let output = format_detected_projects(&projects, root, true);
        assert!(output.contains("Project Guidelines"));
        assert!(output.contains("Rust Project Guidelines"));
        // Frontmatter should be stripped
        assert!(!output.contains("partial: true"));
    }

    #[test]
    fn test_format_without_guidelines() {
        let root = Path::new("/workspace");
        let projects = vec![DetectedProject {
            path: "/workspace".into(),
            project_type: ProjectType::Rust,
            marker_files: vec!["Cargo.toml".to_string()],
            workspace_info: None,
        }];

        let output = format_detected_projects(&projects, root, false);
        assert!(!output.contains("Project Guidelines"));
    }

    #[test]
    fn test_guidelines_deduplication() {
        let root = Path::new("/workspace");
        let projects = vec![
            DetectedProject {
                path: "/workspace/app1".into(),
                project_type: ProjectType::Rust,
                marker_files: vec!["Cargo.toml".to_string()],
                workspace_info: None,
            },
            DetectedProject {
                path: "/workspace/app2".into(),
                project_type: ProjectType::Rust,
                marker_files: vec!["Cargo.toml".to_string()],
                workspace_info: None,
            },
        ];

        let output = format_detected_projects(&projects, root, true);
        // Rust guidelines should appear only once despite two Rust projects
        let count = output.matches("Rust Project Guidelines").count();
        assert_eq!(count, 1, "Rust guidelines should appear exactly once");
    }

    #[test]
    fn test_format_workspace_info() {
        let root = Path::new("/workspace");
        let projects = vec![DetectedProject {
            path: "/workspace".into(),
            project_type: ProjectType::Rust,
            marker_files: vec!["Cargo.toml".to_string()],
            workspace_info: Some(swissarmyhammer_project_detection::WorkspaceInfo {
                is_root: true,
                members: vec!["crate-a".to_string(), "crate-b".to_string()],
                metadata: None,
            }),
        }];

        let output = format_detected_projects(&projects, root, false);
        assert!(output.contains("**Workspace:** Yes (2 members)"));
        assert!(output.contains("crate-a, crate-b"));
    }

    #[tokio::test]
    async fn test_execute_detect_with_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test-project\"",
        )
        .unwrap();

        let context = crate::test_utils::create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );

        let result = execute_detect(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        let content = &call_result.content[0];
        let text = match content.raw {
            rmcp::model::RawContent::Text(ref t) => &t.text,
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("Rust Project"));
        assert!(text.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_execute_detect_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let context = crate::test_utils::create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );

        let result = execute_detect(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        let content = &call_result.content[0];
        let text = match content.raw {
            rmcp::model::RawContent::Text(ref t) => &t.text,
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("No Projects Detected"));
    }

    #[test]
    fn test_detect_request_defaults() {
        let json = serde_json::json!({});
        let request: DetectRequest = serde_json::from_value(json).unwrap();
        assert!(request.path.is_none());
        assert!(request.max_depth.is_none());
        assert!(request.include_guidelines.is_none());
    }

    #[test]
    fn test_detect_request_with_all_params() {
        let json = serde_json::json!({
            "path": "/some/dir",
            "max_depth": 5,
            "include_guidelines": false
        });
        let request: DetectRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.path, Some("/some/dir".to_string()));
        assert_eq!(request.max_depth, Some(5));
        assert_eq!(request.include_guidelines, Some(false));
    }
}
