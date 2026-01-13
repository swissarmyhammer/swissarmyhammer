//! Project type definitions and detection logic

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A detected project with its type and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedProject {
    /// Absolute path to the project root
    pub path: PathBuf,

    /// The type of project detected
    pub project_type: ProjectType,

    /// Marker files that were found (e.g., ["Cargo.toml", "Cargo.lock"])
    pub marker_files: Vec<String>,

    /// Workspace/monorepo information if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_info: Option<WorkspaceInfo>,
}

/// Type of project detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    /// Rust project (Cargo.toml)
    Rust,
    /// Node.js/JavaScript/TypeScript (package.json)
    #[serde(rename = "nodejs")]
    NodeJs,
    /// Python project (pyproject.toml or setup.py)
    Python,
    /// Go project (go.mod)
    Go,
    /// Java Maven project (pom.xml)
    #[serde(rename = "java-maven")]
    JavaMaven,
    /// Java Gradle project (build.gradle or build.gradle.kts)
    #[serde(rename = "java-gradle")]
    JavaGradle,
    /// C# / .NET project (*.csproj or *.sln)
    #[serde(rename = "csharp")]
    CSharp,
    /// C/C++ CMake project (CMakeLists.txt)
    #[serde(rename = "cmake")]
    CMake,
    /// C/C++ Makefile project (Makefile)
    Makefile,
    /// Dart/Flutter project (pubspec.yaml)
    #[serde(rename = "flutter")]
    Flutter,
}

impl ProjectType {
    /// Get the marker files that identify this project type
    pub fn marker_files(&self) -> &[&str] {
        match self {
            ProjectType::Rust => &["Cargo.toml"],
            ProjectType::NodeJs => &["package.json"],
            ProjectType::Python => &["pyproject.toml", "setup.py"],
            ProjectType::Go => &["go.mod"],
            ProjectType::JavaMaven => &["pom.xml"],
            ProjectType::JavaGradle => &["build.gradle", "build.gradle.kts"],
            ProjectType::CSharp => &["*.csproj", "*.sln"],
            ProjectType::CMake => &["CMakeLists.txt"],
            ProjectType::Makefile => &["Makefile"],
            ProjectType::Flutter => &["pubspec.yaml"],
        }
    }
}

/// Workspace/monorepo information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// Is this the workspace root?
    pub is_root: bool,

    /// Workspace members (relative paths from workspace root)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,

    /// Workspace type-specific metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Directories to skip during traversal (build outputs, dependencies, etc.)
pub const SKIP_DIRECTORIES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "build",
    "dist",
    "out",
    ".next",
    ".nuxt",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".venv",
    "venv",
    "env",
    ".tox",
    "vendor",
    ".idea",
    ".vscode",
    ".cargo",
    ".dart_tool",
];

/// Check if a directory should be skipped during traversal
pub fn should_skip_directory(dir_name: &str) -> bool {
    SKIP_DIRECTORIES.contains(&dir_name)
}
