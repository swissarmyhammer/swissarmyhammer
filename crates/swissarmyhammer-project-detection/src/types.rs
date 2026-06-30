//! Project type definitions and detection logic

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A detected project with its type and metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Swift project (Package.swift, *.xcodeproj, or *.xcworkspace)
    #[serde(rename = "swift")]
    Swift,
}

/// Builtin config yaml, embedded at compile time.
/// Edit `builtin/project-detection/config.yaml` to change defaults.
pub const BUILTIN_CONFIG_YAML: &str =
    include_str!("../../../builtin/project-detection/config.yaml");

/// Top-level config wrapper for the yaml file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectDetectionConfig {
    /// Configurable symbol strings for detected project types (Nerd Font glyphs)
    pub symbols: ProjectSymbols,
}

/// Configurable symbols for all project types.
///
/// Like Starship, each language has a default Nerd Font symbol that can be overridden.
/// Defaults are loaded from `builtin/project-detection/config.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectSymbols {
    /// Nerd Font symbol for Rust projects
    pub rust: String,
    /// Nerd Font symbol for Node.js projects
    pub nodejs: String,
    /// Nerd Font symbol for Python projects
    pub python: String,
    /// Nerd Font symbol for Go projects
    pub go: String,
    /// Nerd Font symbol for Java projects (Maven and Gradle)
    pub java: String,
    /// Nerd Font symbol for C# / .NET projects
    pub csharp: String,
    /// Nerd Font symbol for C/C++ projects (CMake and Makefile)
    pub c_cpp: String,
    /// Nerd Font symbol for Dart/Flutter projects
    pub dart: String,
    /// Nerd Font symbol for PHP projects
    pub php: String,
    /// Nerd Font symbol for Swift projects
    pub swift: String,
}

impl Default for ProjectSymbols {
    /// Load defaults from the builtin config yaml.
    ///
    /// # Panics
    ///
    /// Panics if the builtin config YAML ([`BUILTIN_CONFIG_YAML`]) is malformed
    /// and fails to deserialize into a [`ProjectDetectionConfig`].
    fn default() -> Self {
        let config: ProjectDetectionConfig =
            serde_yaml_ng::from_str(BUILTIN_CONFIG_YAML).expect("builtin config.yaml must parse");
        config.symbols
    }
}

/// Per-variant specification for a [`ProjectType`].
///
/// One entry per variant in [`PROJECT_TYPE_SPECS`] is the single authoritative
/// roster of project types. Every per-variant behavior — marker-file detection,
/// symbol lookup, detection priority, and the tools-layer presentation metadata
/// (display name, stable key, guideline partial) — derives from this table, so
/// adding a project type touches exactly one entry here and nowhere else.
///
/// Intentionally does NOT derive `PartialEq`/`Eq`/`Hash`: the `symbol` field is
/// a function pointer, and comparing function pointers is unpredictable (the
/// `unpredictable_function_pointer_comparisons` lint, denied as a warning). The
/// authoritative identity of a spec is its [`ProjectType`], which is fully
/// comparable on its own.
#[derive(Debug, Clone, Copy)]
pub struct ProjectTypeSpec {
    /// The project type this entry describes.
    pub project_type: ProjectType,
    /// Marker files that identify this project type.
    pub marker_files: &'static [&'static str],
    /// Accessor for this type's configurable symbol within [`ProjectSymbols`].
    pub symbol: fn(&ProjectSymbols) -> &str,
    /// Human-readable display name (e.g. `"Java (Maven)"`).
    pub name: &'static str,
    /// Stable string key. MUST equal the serde representation of
    /// [`ProjectTypeSpec::project_type`] (guarded by tests) because it doubles
    /// as the guideline partial filename and the deduplication key.
    pub key: &'static str,
    /// Guideline partial path (`_partials/project-types/{key}`), or `None` for
    /// types without one (e.g. PHP).
    pub partial: Option<&'static str>,
}

/// Build a guideline partial path from a project-type key.
///
/// The partial filename always follows the `_partials/project-types/{key}`
/// convention, so deriving it from the key here keeps the two in sync by
/// construction rather than by a hand-maintained literal per entry. `$key`
/// must be a string literal so the path is assembled at compile time via
/// [`concat!`] (guarded by `spec_partial_matches_key`).
macro_rules! partial {
    ($key:literal) => {
        Some(concat!("_partials/project-types/", $key))
    };
}

/// Single source of truth mapping each [`ProjectType`] to its metadata.
///
/// Adding a project type means adding one entry here; the accessors below (and
/// in the tools layer) are thin table lookups so the variants never drift out
/// of lockstep. The table order is the **detection priority order** used by
/// `detect_project_at_path` when a single directory matches multiple types.
const PROJECT_TYPE_SPECS: &[ProjectTypeSpec] = &[
    ProjectTypeSpec {
        project_type: ProjectType::Rust,
        marker_files: &["Cargo.toml"],
        symbol: |s| &s.rust,
        name: "Rust",
        key: "rust",
        partial: partial!("rust"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::NodeJs,
        marker_files: &["package.json"],
        symbol: |s| &s.nodejs,
        name: "Node.js",
        key: "nodejs",
        partial: partial!("nodejs"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::Go,
        marker_files: &["go.mod"],
        symbol: |s| &s.go,
        name: "Go",
        key: "go",
        partial: partial!("go"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::Python,
        marker_files: &["pyproject.toml", "setup.py"],
        symbol: |s| &s.python,
        name: "Python",
        key: "python",
        partial: partial!("python"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::JavaMaven,
        marker_files: &["pom.xml"],
        symbol: |s| &s.java,
        name: "Java (Maven)",
        key: "java-maven",
        partial: partial!("java-maven"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::JavaGradle,
        marker_files: &["build.gradle", "build.gradle.kts"],
        symbol: |s| &s.java,
        name: "Java (Gradle)",
        key: "java-gradle",
        partial: partial!("java-gradle"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::CSharp,
        marker_files: &["*.csproj", "*.sln"],
        symbol: |s| &s.csharp,
        name: "C# / .NET",
        key: "csharp",
        partial: partial!("csharp"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::CMake,
        marker_files: &["CMakeLists.txt"],
        symbol: |s| &s.c_cpp,
        name: "CMake",
        key: "cmake",
        partial: partial!("cmake"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::Makefile,
        marker_files: &["Makefile"],
        symbol: |s| &s.c_cpp,
        name: "Makefile",
        key: "makefile",
        partial: partial!("makefile"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::Flutter,
        marker_files: &["pubspec.yaml"],
        symbol: |s| &s.dart,
        name: "Flutter",
        key: "flutter",
        partial: partial!("flutter"),
    },
    ProjectTypeSpec {
        project_type: ProjectType::Php,
        marker_files: &["composer.json"],
        symbol: |s| &s.php,
        name: "PHP",
        key: "php",
        partial: None,
    },
    ProjectTypeSpec {
        project_type: ProjectType::Swift,
        marker_files: &["Package.swift", "*.xcodeproj", "*.xcworkspace"],
        symbol: |s| &s.swift,
        name: "Swift",
        key: "swift",
        partial: partial!("swift"),
    },
];

/// The authoritative roster of project-type specifications.
///
/// Iterate this to enumerate every [`ProjectType`] in detection-priority order
/// without maintaining a separate variant list. This is the single source of
/// truth for the variant roster across the workspace.
pub fn project_type_specs() -> &'static [ProjectTypeSpec] {
    PROJECT_TYPE_SPECS
}

/// Look up the spec entry for a project type.
///
/// Every [`ProjectType`] variant has exactly one entry in [`PROJECT_TYPE_SPECS`],
/// so this never returns `None` in practice.
///
/// # Panics
///
/// Panics if the given [`ProjectType`] has no entry in [`PROJECT_TYPE_SPECS`].
/// Every variant is expected to have one (guarded by tests), so this cannot
/// happen in practice, but a future variant added without a matching table
/// entry would trip it.
pub fn spec_for(project_type: ProjectType) -> &'static ProjectTypeSpec {
    PROJECT_TYPE_SPECS
        .iter()
        .find(|spec| spec.project_type == project_type)
        .expect("every ProjectType variant has a spec entry")
}

impl ProjectSymbols {
    /// Get the symbol for a project type.
    ///
    /// # Panics
    ///
    /// Panics if `project_type` has no entry in [`PROJECT_TYPE_SPECS`] (see
    /// [`spec_for`]). Every variant has one, so this cannot happen in practice.
    pub fn get(&self, project_type: ProjectType) -> &str {
        (spec_for(project_type).symbol)(self)
    }
}

impl ProjectType {
    /// Get the marker files that identify this project type.
    ///
    /// # Panics
    ///
    /// Panics if this variant has no entry in [`PROJECT_TYPE_SPECS`] (see
    /// [`spec_for`]). Every variant has one, so this cannot happen in practice.
    pub fn marker_files(&self) -> &[&str] {
        spec_for(*self).marker_files
    }
}

/// Workspace/monorepo information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_symbols_default_loads_successfully() {
        let symbols = ProjectSymbols::default();
        // All fields should be non-empty after loading from builtin YAML
        assert!(!symbols.rust.is_empty(), "rust symbol should not be empty");
        assert!(
            !symbols.nodejs.is_empty(),
            "nodejs symbol should not be empty"
        );
        assert!(
            !symbols.python.is_empty(),
            "python symbol should not be empty"
        );
        assert!(!symbols.go.is_empty(), "go symbol should not be empty");
        assert!(!symbols.java.is_empty(), "java symbol should not be empty");
        assert!(
            !symbols.csharp.is_empty(),
            "csharp symbol should not be empty"
        );
        assert!(
            !symbols.c_cpp.is_empty(),
            "c_cpp symbol should not be empty"
        );
        assert!(!symbols.dart.is_empty(), "dart symbol should not be empty");
        assert!(!symbols.php.is_empty(), "php symbol should not be empty");
        assert!(
            !symbols.swift.is_empty(),
            "swift symbol should not be empty"
        );
    }

    #[test]
    fn project_symbols_get_returns_nonempty_for_all_variants() {
        let symbols = ProjectSymbols::default();

        let variants = [
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
            ProjectType::Php,
            ProjectType::Swift,
        ];

        for variant in &variants {
            let symbol = symbols.get(*variant);
            assert!(
                !symbol.is_empty(),
                "symbol for {:?} should not be empty",
                variant
            );
        }
    }

    #[test]
    fn project_symbols_get_maps_variants_to_correct_fields() {
        let symbols = ProjectSymbols::default();

        // Direct 1:1 mappings
        assert_eq!(symbols.get(ProjectType::Rust), &symbols.rust);
        assert_eq!(symbols.get(ProjectType::NodeJs), &symbols.nodejs);
        assert_eq!(symbols.get(ProjectType::Python), &symbols.python);
        assert_eq!(symbols.get(ProjectType::Go), &symbols.go);
        assert_eq!(symbols.get(ProjectType::CSharp), &symbols.csharp);
        assert_eq!(symbols.get(ProjectType::Flutter), &symbols.dart);
        assert_eq!(symbols.get(ProjectType::Php), &symbols.php);
        assert_eq!(symbols.get(ProjectType::Swift), &symbols.swift);

        // Shared mappings: Java variants both map to java
        assert_eq!(symbols.get(ProjectType::JavaMaven), &symbols.java);
        assert_eq!(symbols.get(ProjectType::JavaGradle), &symbols.java);

        // Shared mappings: C/C++ variants both map to c_cpp
        assert_eq!(symbols.get(ProjectType::CMake), &symbols.c_cpp);
        assert_eq!(symbols.get(ProjectType::Makefile), &symbols.c_cpp);
    }

    #[test]
    fn every_variant_has_a_spec_entry() {
        // The data-driven accessors look the variant up in PROJECT_TYPE_SPECS and
        // `expect()` an entry. Confirm the table covers every variant so the
        // accessors can never panic in production.
        let variants = [
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
            ProjectType::Php,
            ProjectType::Swift,
        ];
        for variant in variants {
            // Must not panic — every variant resolves to a spec.
            let spec = spec_for(variant);
            assert_eq!(spec.project_type, variant);
            assert!(
                !spec.marker_files.is_empty(),
                "spec for {variant:?} should have marker files"
            );
        }
    }

    #[test]
    fn spec_key_matches_serde_repr() {
        // The spec `key` MUST equal the serde representation of the variant,
        // since the key doubles as the guideline partial filename and the
        // deduplication key. Guard the hidden coupling: serialize each variant
        // and compare.
        for spec in project_type_specs() {
            let serialized = serde_json::to_value(spec.project_type)
                .expect("ProjectType serializes")
                .as_str()
                .expect("ProjectType serializes to a string")
                .to_string();
            assert_eq!(
                spec.key, serialized,
                "key for {:?} must match its serde rename",
                spec.project_type
            );
        }
    }

    #[test]
    fn value_types_support_equality() {
        use std::collections::HashSet;

        // Derive-contract guard: the public value types compare by value so
        // downstream crates (blocked by orphan rules from adding the impls
        // later) can use them in assertions and sets. Types deriving `Hash + Eq`
        // are additionally exercised in a `HashSet` to prove set membership and
        // deduplication, not just equality.
        let a = ProjectSymbols::default();
        let b = ProjectSymbols::default();
        assert_eq!(a, b);

        // `ProjectSymbols` is `Hash + Eq`: equal values collapse to one entry,
        // and membership is by value.
        let symbol_set: HashSet<ProjectSymbols> = [a.clone(), b.clone()].into_iter().collect();
        assert_eq!(
            symbol_set.len(),
            1,
            "equal ProjectSymbols must dedup in a set"
        );
        assert!(
            symbol_set.contains(&a),
            "set must contain the value by equality"
        );

        // `ProjectType` is `Hash + Eq`: distinct variants stay distinct, and a
        // re-inserted variant does not grow the set.
        let mut type_set: HashSet<ProjectType> = HashSet::new();
        type_set.insert(ProjectType::Rust);
        type_set.insert(ProjectType::Go);
        type_set.insert(ProjectType::Rust);
        assert_eq!(
            type_set.len(),
            2,
            "duplicate ProjectType must not grow the set"
        );
        assert!(type_set.contains(&ProjectType::Rust));
        assert!(!type_set.contains(&ProjectType::Swift));

        // `ProjectTypeSpec` deliberately does not derive `PartialEq` (its
        // `symbol` field is a function pointer); compare by `project_type`.
        let spec_a = spec_for(ProjectType::Rust);
        let spec_b = spec_for(ProjectType::Rust);
        assert_eq!(spec_a.project_type, spec_b.project_type);
        assert_ne!(spec_a.project_type, spec_for(ProjectType::Go).project_type);

        let config_a = ProjectDetectionConfig {
            symbols: ProjectSymbols::default(),
        };
        let config_b = ProjectDetectionConfig {
            symbols: ProjectSymbols::default(),
        };
        assert_eq!(config_a, config_b);

        // `ProjectDetectionConfig` is `Hash + Eq`: equal configs dedup in a set.
        let config_set: HashSet<ProjectDetectionConfig> =
            [config_a.clone(), config_b.clone()].into_iter().collect();
        assert_eq!(config_set.len(), 1, "equal configs must dedup in a set");
        assert!(config_set.contains(&config_a));

        let ws_a = WorkspaceInfo {
            is_root: true,
            members: vec!["a".to_string()],
            metadata: None,
        };
        let ws_b = WorkspaceInfo {
            is_root: true,
            members: vec!["a".to_string()],
            metadata: None,
        };
        assert_eq!(ws_a, ws_b);

        let proj_a = DetectedProject {
            path: PathBuf::from("/tmp/x"),
            project_type: ProjectType::Rust,
            marker_files: vec!["Cargo.toml".to_string()],
            workspace_info: None,
        };
        let proj_b = DetectedProject {
            path: PathBuf::from("/tmp/x"),
            project_type: ProjectType::Rust,
            marker_files: vec!["Cargo.toml".to_string()],
            workspace_info: None,
        };
        assert_eq!(proj_a, proj_b);
    }

    #[test]
    fn spec_partial_matches_key() {
        // Every type with a guideline partial uses the `_partials/project-types/{key}`
        // convention. PHP is the deliberate exception with no partial.
        for spec in project_type_specs() {
            match spec.partial {
                Some(partial) => assert_eq!(
                    partial,
                    format!("_partials/project-types/{}", spec.key),
                    "partial for {:?} must follow the key convention",
                    spec.project_type
                ),
                None => assert_eq!(
                    spec.project_type,
                    ProjectType::Php,
                    "only PHP is expected to have no partial"
                ),
            }
        }
    }
}
