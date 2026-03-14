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
    /// PHP project (composer.json)
    #[serde(rename = "php")]
    Php,
}

/// Builtin config yaml, embedded at compile time.
/// Edit `builtin/project-detection/config.yaml` to change defaults.
pub const BUILTIN_CONFIG_YAML: &str = include_str!("../../builtin/project-detection/config.yaml");

/// Top-level config wrapper for the yaml file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetectionConfig {
    /// Configurable symbol strings for detected project types (Nerd Font glyphs)
    pub symbols: ProjectSymbols,
}

/// Configurable symbols for all project types.
///
/// Like Starship, each language has a default Nerd Font symbol that can be overridden.
/// Defaults are loaded from `builtin/project-detection/config.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSymbols {
    pub rust: String,
    pub nodejs: String,
    pub python: String,
    pub go: String,
    pub java: String,
    pub csharp: String,
    pub c_cpp: String,
    pub dart: String,
    pub php: String,
}

impl Default for ProjectSymbols {
    /// Load defaults from the builtin config yaml
    fn default() -> Self {
        let config: ProjectDetectionConfig =
            serde_yaml_ng::from_str(BUILTIN_CONFIG_YAML).expect("builtin config.yaml must parse");
        config.symbols
    }
}

impl ProjectSymbols {
    /// Get the symbol for a project type
    pub fn get(&self, project_type: ProjectType) -> &str {
        match project_type {
            ProjectType::Rust => &self.rust,
            ProjectType::NodeJs => &self.nodejs,
            ProjectType::Python => &self.python,
            ProjectType::Go => &self.go,
            ProjectType::JavaMaven | ProjectType::JavaGradle => &self.java,
            ProjectType::CSharp => &self.csharp,
            ProjectType::CMake | ProjectType::Makefile => &self.c_cpp,
            ProjectType::Flutter => &self.dart,
            ProjectType::Php => &self.php,
        }
    }
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
            ProjectType::Php => &["composer.json"],
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
