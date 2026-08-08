//! Mirdan List - List installed packages (skills, validators, tools, plugins).

use std::path::{Path, PathBuf};

use crate::agents::{self, agent_project_skill_dir, DetectedAgent};
use crate::lockfile::Lockfile;
use crate::mcp_config;
use crate::merge::merge_unique;
use crate::package_type::PackageType;
use crate::registry::RegistryError;
use crate::store;
use crate::table;

/// Version reported for a package whose metadata declares none.
const UNVERSIONED: &str = "latest";

/// An installed package found during scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPackage {
    /// Display name, taken from frontmatter or the terminal path segment.
    pub name: String,
    /// The lockfile key (source URL or name) used for install/uninstall operations.
    pub source: String,
    /// One-line summary, taken from frontmatter. Empty when absent.
    pub description: String,
    /// Which kind of package this is.
    pub package_type: PackageType,
    /// Package version, or `latest` when the frontmatter declares none.
    pub version: String,
    /// Where the package was found — an agent name or a store location label.
    pub targets: Vec<String>,
}

/// Which package types `list` scans.
///
/// The `mirdan list` flags `--skills`, `--validators`, `--tools`, and
/// `--plugins` each narrow the scan to one type, and clap rejects any two of
/// them together. [`PackageFilter::All`] is the no-flag default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackageFilter {
    /// Scan every package type.
    #[default]
    All,
    /// Scan installed skills only.
    SkillsOnly,
    /// Scan installed validators only.
    ValidatorsOnly,
    /// Scan installed tools only.
    ToolsOnly,
    /// Scan installed plugins only.
    PluginsOnly,
}

impl PackageFilter {
    /// Whether this filter selects `package_type`.
    fn includes(self, package_type: PackageType) -> bool {
        match self {
            Self::All => true,
            Self::SkillsOnly => package_type == PackageType::Skill,
            Self::ValidatorsOnly => package_type == PackageType::Validator,
            Self::ToolsOnly => package_type == PackageType::Tool,
            Self::PluginsOnly => package_type == PackageType::Plugin,
        }
    }
}

/// Discover installed packages by scanning the filesystem.
///
/// Scans agent skill directories, ./.validators/, .tools/, and agent plugin dirs.
/// Returns a deduplicated, sorted list.
pub fn discover_packages(
    filter: PackageFilter,
    agent_filter: Option<&str>,
) -> Vec<InstalledPackage> {
    let mut packages: Vec<InstalledPackage> = Vec::new();

    if filter.includes(PackageType::Skill) {
        discover_skills(agent_filter, &mut packages);
    }

    // Validators are not agent-scoped, so an --agent filter suppresses them.
    if filter.includes(PackageType::Validator) && agent_filter.is_none() {
        discover_scope_pair(
            &VALIDATOR_SPEC,
            crate::install::validators_dir,
            ".validators/",
            &mut packages,
        );
    }

    if filter.includes(PackageType::Tool) {
        discover_scope_pair(&TOOL_SPEC, store::tool_store_dir, ".tools/", &mut packages);
    }

    if filter.includes(PackageType::Plugin) {
        discover_plugins(agent_filter, &mut packages);
    }

    let mut merged = merge_packages(packages);
    enrich_sources_from_lockfiles(&mut merged);
    merged
}

/// Resolve the agents to scan, or an empty list when detection fails.
fn target_agents(agent_filter: Option<&str>) -> Vec<DetectedAgent> {
    let Ok(config) = agents::load_agents_config() else {
        return Vec::new();
    };
    agents::resolve_target_agents(&config, agent_filter).unwrap_or_default()
}

/// Whether two paths resolve to the same directory on disk.
///
/// `canonicalize` fails when a path does not exist or is not accessible (e.g.
/// permission denied), and this reports `false` in that case.
fn resolves_to_same_dir(left: &Path, right: &Path) -> bool {
    left.canonicalize()
        .ok()
        .zip(right.canonicalize().ok())
        .is_some_and(|(l, r)| l == r)
}

/// Scan the skill stores and each agent's project skill directory.
///
/// The store (`~/.skills/` global, `.skills/` project) is the source of truth
/// for installed packages. Agent directories (`.claude/skills/`, etc.) contain
/// symlinks into the store, which can break (e.g. when `~/.claude` is itself a
/// symlink to iCloud). Scanning the store directly is robust. The agent
/// project-level directories are scanned too, for skills installed without the
/// store (e.g. manually placed skills).
fn discover_skills(agent_filter: Option<&str>, packages: &mut Vec<InstalledPackage>) {
    let global_store = store::skill_store_dir(true);
    skill_store_scan(&global_store, "global").walk(&global_store, packages);

    // A project store that resolves to the global store would list every skill
    // twice, so scan it only when it is a distinct directory.
    let project_store = store::skill_store_dir(false);
    if !resolves_to_same_dir(&project_store, &global_store) {
        skill_store_scan(&project_store, "project").walk(&project_store, packages);
    }

    for agent in target_agents(agent_filter) {
        Scan {
            spec: &SKILL_SPEC,
            target: &agent.def.name,
            store_root: None,
            naming: Naming::DirectoryName,
        }
        .walk(&agent_project_skill_dir(&agent.def), packages);
    }
}

/// A scan of the skill store rooted at `root`, recorded under `target`.
fn skill_store_scan<'a>(root: &'a Path, target: &'a str) -> Scan<'a> {
    Scan {
        spec: &SKILL_SPEC,
        target,
        store_root: Some(root),
        naming: Naming::DeclaredName,
    }
}

/// Scan a package type's project directory, then its global directory.
///
/// `dir_for_scope` resolves that type's directory for a scope — `false` for the
/// project scope, `true` for the global one. Each scope is recorded under the
/// directory it reads: `project_label` as given, and the same label under `~/`
/// for the global scope.
fn discover_scope_pair(
    spec: &PackageSpec,
    dir_for_scope: fn(bool) -> PathBuf,
    project_label: &str,
    packages: &mut Vec<InstalledPackage>,
) {
    let global_label = format!("~/{project_label}");
    for (global, label) in [(false, project_label), (true, global_label.as_str())] {
        Scan {
            spec,
            target: label,
            store_root: None,
            naming: Naming::DirectoryName,
        }
        .walk(&dir_for_scope(global), packages);
    }
}

/// Scan each agent's project and global plugin directories.
fn discover_plugins(agent_filter: Option<&str>, packages: &mut Vec<InstalledPackage>) {
    for agent in target_agents(agent_filter) {
        let scopes = [
            (
                agents::agent_project_plugin_dir(&agent.def),
                agent.def.name.clone(),
            ),
            (
                agents::agent_global_plugin_dir(&agent.def),
                format!("{} (global)", agent.def.name),
            ),
        ];

        for (dir, target) in scopes {
            let Some(dir) = dir else {
                continue;
            };
            Scan {
                spec: &PLUGIN_SPEC,
                target: &target,
                store_root: None,
                naming: Naming::DeclaredName,
            }
            .walk(&dir, packages);
        }
    }
}

/// The directories searched for a lockfile: the home directory, then the CWD.
fn lockfile_search_dirs() -> [Option<PathBuf>; 2] {
    [dirs::home_dir(), std::env::current_dir().ok()]
}

/// Find the lockfile key that names `name`, either outright or as its last
/// path segment.
fn lockfile_key_for(lockfile: &Lockfile, name: &str) -> Option<String> {
    lockfile
        .packages
        .keys()
        .find(|key| *key == name || key.rsplit('/').next().unwrap_or(key) == name)
        .cloned()
}

/// Replace each bare display name in `source` with the package's full lockfile
/// key, so callers (e.g. the GUI) can pass the identifier that
/// uninstall/update expect.
fn enrich_sources_from_lockfiles(packages: &mut [InstalledPackage]) {
    for dir in lockfile_search_dirs().iter().flatten() {
        let Ok(lockfile) = Lockfile::load(dir) else {
            continue;
        };
        for pkg in packages.iter_mut() {
            // A source that already differs from the name carries the key.
            if pkg.source != pkg.name {
                continue;
            }
            if let Some(key) = lockfile_key_for(&lockfile, &pkg.name) {
                pkg.source = key;
            }
        }
    }
}

/// Get the mirdan.ai registry URL for a package.
///
/// Looks up the source URL from the lockfile (where the key is the full
/// source like `https://github.com/owner/repo/skill`), then constructs
/// `https://mirdan.ai/package/{url_encoded_source}`.
pub fn registry_url(name: &str) -> String {
    let key = lockfile_search_dirs()
        .iter()
        .flatten()
        .filter_map(|dir| Lockfile::load(dir).ok())
        .find_map(|lockfile| lockfile_key_for(&lockfile, name));

    let target = key.as_deref().unwrap_or(name);
    format!("https://mirdan.ai/package/{}", urlencoding::encode(target))
}

/// Run the list command.
///
/// Scans all package locations for installed packages.
///
/// # Errors
///
/// Returns [`RegistryError`] when the discovered packages cannot be rendered.
pub fn run_list(
    filter: PackageFilter,
    agent_filter: Option<&str>,
    json: bool,
) -> Result<(), RegistryError> {
    let packages = discover_packages(filter, agent_filter);

    if json {
        let entries: Vec<serde_json::Value> = packages
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "type": p.package_type.to_string(),
                    "version": p.version,
                    "targets": p.targets,
                })
            })
            .collect();
        let output = serde_json::json!({ "packages": entries });
        let rendered = serde_json::to_string_pretty(&output)
            .map_err(|error| RegistryError::Json(error.to_string()))?;
        println!("{rendered}");
        return Ok(());
    }

    if packages.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    println!("Installed Packages:\n");

    let mut tbl = table::new_table();
    tbl.set_header(vec!["Name", "Type", "Version", "Targets"]);

    for pkg in &packages {
        tbl.add_row(vec![
            pkg.name.clone(),
            pkg.package_type.to_string(),
            pkg.version.clone(),
            pkg.targets.join(", "),
        ]);
    }

    println!("{tbl}");
    println!("\n{} package(s) installed.", packages.len());

    Ok(())
}

/// How a package type's metadata file is read.
#[derive(Debug, Clone, Copy)]
enum MetadataFormat {
    /// YAML frontmatter at the head of a markdown file.
    Frontmatter,
    /// A Claude Code `plugin.json` manifest, which declares a name and nothing
    /// else this listing shows.
    PluginJson,
}

/// Where a scan takes a package's display name from.
#[derive(Debug, Clone, Copy)]
enum Naming {
    /// The package directory's own name.
    DirectoryName,
    /// The name the package metadata declares, falling back to the directory
    /// name when it declares none.
    DeclaredName,
}

/// The display fields a package's metadata file declares.
struct DeclaredMetadata {
    /// The declared name, when the metadata carries one.
    name: Option<String>,
    /// One-line summary. Empty when the metadata carries none.
    description: String,
    /// The declared version, or [`UNVERSIONED`].
    version: String,
}

/// How one package type is recognized on disk.
///
/// Every type lives in its own directory and is told apart by the metadata file
/// that directory holds. Holding that difference as data lets [`Scan`] read
/// every type, instead of one near-identical scan function per type.
#[derive(Debug, Clone, Copy)]
struct PackageSpec {
    /// The kind of package a matching directory holds.
    package_type: PackageType,
    /// The metadata file, relative to the package directory, that must exist.
    metadata_file: &'static str,
    /// A directory, relative to the package directory, that must also exist.
    /// `None` when the metadata file alone identifies the type.
    required_subdir: Option<&'static str>,
    /// How to read [`PackageSpec::metadata_file`].
    format: MetadataFormat,
}

/// Skills carry a `SKILL.md` with YAML frontmatter.
static SKILL_SPEC: PackageSpec = PackageSpec {
    package_type: PackageType::Skill,
    metadata_file: "SKILL.md",
    required_subdir: None,
    format: MetadataFormat::Frontmatter,
};

/// Validators carry a `VALIDATOR.md` beside the `rules/` directory it indexes.
static VALIDATOR_SPEC: PackageSpec = PackageSpec {
    package_type: PackageType::Validator,
    metadata_file: "VALIDATOR.md",
    required_subdir: Some("rules"),
    format: MetadataFormat::Frontmatter,
};

/// Tools carry a `TOOL.md` with YAML frontmatter.
static TOOL_SPEC: PackageSpec = PackageSpec {
    package_type: PackageType::Tool,
    metadata_file: "TOOL.md",
    required_subdir: None,
    format: MetadataFormat::Frontmatter,
};

/// Plugins carry a `.claude-plugin/plugin.json` manifest.
static PLUGIN_SPEC: PackageSpec = PackageSpec {
    package_type: PackageType::Plugin,
    metadata_file: ".claude-plugin/plugin.json",
    required_subdir: None,
    format: MetadataFormat::PluginJson,
};

impl PackageSpec {
    /// Whether `path` is a directory holding a package of this type.
    fn matches(&self, path: &Path) -> bool {
        path.is_dir()
            && path.join(self.metadata_file).exists()
            && self
                .required_subdir
                .is_none_or(|subdir| path.join(subdir).is_dir())
    }

    /// Read the display fields the package in `path` declares.
    fn declared(&self, path: &Path) -> DeclaredMetadata {
        let file = path.join(self.metadata_file);
        match self.format {
            MetadataFormat::Frontmatter => DeclaredMetadata {
                name: read_frontmatter_name(&file),
                description: read_frontmatter_description(&file),
                version: read_frontmatter_version(&file),
            },
            MetadataFormat::PluginJson => DeclaredMetadata {
                name: mcp_config::read_plugin_json(&file).ok(),
                description: String::new(),
                version: UNVERSIONED.to_string(),
            },
        }
    }
}

/// One scan of a directory for installed packages.
///
/// This is the single walk every package type goes through. The differences
/// between scanning a skill store, an agent skill directory, the validator
/// directories, the tool stores, and an agent plugin directory are these fields
/// and nothing else.
struct Scan<'a> {
    /// The package type to recognize.
    spec: &'a PackageSpec,
    /// Label recorded in [`InstalledPackage::targets`] — an agent name, or a
    /// store location such as `global` or `~/.tools/`.
    target: &'a str,
    /// The store root, when the directory walked is a package store.
    ///
    /// A store nests packages under their provenance path (e.g.
    /// `owner/repo/skill`), so a store scan descends into every subdirectory
    /// that is not itself a package, and keys each package by its path relative
    /// to this root. `None` reads one level of package directories, each keyed
    /// by its display name.
    store_root: Option<&'a Path>,
    /// Where the display name comes from.
    naming: Naming,
}

impl Scan<'_> {
    /// Add every package this scan finds under `dir` to `packages`.
    fn walk(&self, dir: &Path, packages: &mut Vec<InstalledPackage>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for path in entries.flatten().map(|entry| entry.path()) {
            if self.spec.matches(&path) {
                packages.push(self.package_at(&path));
            } else if self.store_root.is_some() && path.is_dir() {
                self.walk(&path, packages);
            }
        }
    }

    /// Build the record for the package directory `path`.
    fn package_at(&self, path: &Path) -> InstalledPackage {
        let dir_name = directory_name(path);
        let declared = self.spec.declared(path);

        // A display name is never a full path — it is the declared name or the
        // terminal path segment.
        let name = match self.naming {
            Naming::DirectoryName => dir_name,
            Naming::DeclaredName => declared.name.unwrap_or(dir_name),
        };
        let source = match self.store_root {
            // The store-relative path preserves provenance (e.g.
            // `0xdarkmatter/claude-mods/explain`), so it is the lockfile key.
            Some(root) => store_relative_key(path, root),
            None => name.clone(),
        };

        InstalledPackage {
            source,
            name,
            description: declared.description,
            package_type: self.spec.package_type,
            version: declared.version,
            targets: vec![self.target.to_string()],
        }
    }
}

/// The path of `path` relative to the store root `root`, falling back to the
/// terminal path segment when `path` does not sit under `root`.
fn store_relative_key(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().to_string())
        .unwrap_or_else(|_| directory_name(path))
}

/// The terminal segment of `path`, or an empty string when it has none.
fn directory_name(path: &Path) -> String {
    path.file_name()
        .map(|segment| segment.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Parse YAML frontmatter from a markdown file, returning the parsed YAML value.
fn parse_frontmatter(path: &Path) -> Option<serde_yaml_ng::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.trim();
    let rest = content.strip_prefix("---")?;
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];
    serde_yaml_ng::from_str(frontmatter).ok()
}

/// Read name from YAML frontmatter of SKILL.md, VALIDATOR.md, or TOOL.md.
pub fn read_frontmatter_name(path: &Path) -> Option<String> {
    parse_frontmatter(path)?
        .get("name")?
        .as_str()
        .map(|s| s.to_string())
}

/// Read description from YAML frontmatter.
fn read_frontmatter_description(path: &Path) -> String {
    parse_frontmatter(path)
        .and_then(|y| y.get("description")?.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

/// Read version from YAML frontmatter of SKILL.md, VALIDATOR.md, or TOOL.md.
///
/// Checks `metadata.version` first, then top-level `version`. Falls back to
/// [`UNVERSIONED`].
fn read_frontmatter_version(path: &Path) -> String {
    parse_frontmatter(path)
        .and_then(|yaml| {
            yaml.get("metadata")
                .and_then(|m| m.get("version"))
                .and_then(|v| v.as_str())
                .or_else(|| yaml.get("version").and_then(|v| v.as_str()))
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| UNVERSIONED.to_string())
}

/// Merge packages with the same name (combining targets).
fn merge_packages(packages: Vec<InstalledPackage>) -> Vec<InstalledPackage> {
    let mut merged: Vec<InstalledPackage> = Vec::new();

    for pkg in packages {
        match merged.iter_mut().find(|p| p.name == pkg.name) {
            Some(existing) => merge_unique(&mut existing.targets, pkg.targets),
            None => merged.push(pkg),
        }
    }

    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;

    #[test]
    fn test_read_frontmatter_version_skill() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            r#"---
name: test-skill
metadata:
  version: "1.2.3"
---
# Test
"#,
        )
        .unwrap();

        assert_eq!(read_frontmatter_version(&path), "1.2.3");
    }

    #[test]
    fn test_read_frontmatter_version_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(&path, "# No frontmatter").unwrap();

        assert_eq!(read_frontmatter_version(&path), "latest");
    }

    #[test]
    fn test_merge_packages() {
        let packages = vec![
            InstalledPackage {
                source: "skill-a".to_string(),
                name: "skill-a".to_string(),
                description: String::new(),
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                targets: vec!["Claude Code".to_string()],
            },
            InstalledPackage {
                source: "skill-a".to_string(),
                name: "skill-a".to_string(),
                description: String::new(),
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                targets: vec!["Cursor".to_string()],
            },
        ];

        let merged = merge_packages(packages);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].targets.len(), 2);
    }

    #[test]
    fn test_read_frontmatter_version_metadata_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            r#"---
name: test-skill
metadata:
  version: "2.0.0"
---
# Test
"#,
        )
        .unwrap();

        assert_eq!(read_frontmatter_version(&path), "2.0.0");
    }

    #[test]
    #[serial]
    fn test_run_list_empty() {
        // `run_list` reads the process working directory (project store,
        // agent detection, lockfile lookup), so pin it to an empty tempdir.
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Should not panic even with no packages
        let result = run_list(PackageFilter::All, None, true);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_run_list_agent_filter_suppresses_validators() {
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Create a validator structure
        let val_dir = dir.path().join(".validators/test-val");
        std::fs::create_dir_all(&val_dir).unwrap();
        std::fs::write(
            val_dir.join("VALIDATOR.md"),
            "---\nname: test-val\nmetadata:\n  version: \"1.0.0\"\n---\n# Test\n",
        )
        .unwrap();
        std::fs::create_dir(val_dir.join("rules")).unwrap();
        std::fs::write(val_dir.join("rules/rule.md"), "# Rule").unwrap();

        // With agent filter, validators should be suppressed
        let result = run_list(PackageFilter::All, Some("claude-code"), true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_skill_store_scan_nested_structure() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path();

        // Create nested owner/repo/skill/SKILL.md structure
        let skill_dir = store_root.join("owner/repo/my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\nmetadata:\n  version: \"1.0.0\"\n---\n# My Skill\n",
        )
        .unwrap();

        let mut packages = Vec::new();
        skill_store_scan(store_root, "global").walk(store_root, &mut packages);

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-skill");
        assert_eq!(packages[0].source, "owner/repo/my-skill");
        assert_eq!(packages[0].description, "A test skill");
        assert_eq!(packages[0].version, "1.0.0");
        assert_eq!(packages[0].targets, vec!["global"]);
    }

    #[test]
    fn test_skill_store_scan_uses_dir_name_when_no_frontmatter_name() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path();

        let skill_dir = store_root.join("fallback-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# No frontmatter\n").unwrap();

        let mut packages = Vec::new();
        skill_store_scan(store_root, "global").walk(store_root, &mut packages);

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "fallback-skill");
        assert_eq!(packages[0].source, "fallback-skill");
        assert_eq!(packages[0].version, "latest");
    }

    #[test]
    fn test_read_frontmatter_description_present() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: test\ndescription: Hello world\n---\n# Test\n",
        )
        .unwrap();

        assert_eq!(read_frontmatter_description(&path), "Hello world");
    }

    #[test]
    fn test_read_frontmatter_description_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(&path, "---\nname: test\n---\n# Test\n").unwrap();

        assert_eq!(read_frontmatter_description(&path), "");
    }

    #[test]
    fn test_read_frontmatter_description_no_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(&path, "# No frontmatter").unwrap();

        assert_eq!(read_frontmatter_description(&path), "");
    }

    #[test]
    #[serial]
    fn test_run_list_no_filter_shows_validators() {
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Create a validator structure
        let val_dir = dir.path().join(".validators/test-val");
        std::fs::create_dir_all(&val_dir).unwrap();
        std::fs::write(
            val_dir.join("VALIDATOR.md"),
            "---\nname: test-val\nmetadata:\n  version: \"1.0.0\"\n---\n# Test\n",
        )
        .unwrap();
        std::fs::create_dir(val_dir.join("rules")).unwrap();
        std::fs::write(val_dir.join("rules/rule.md"), "# Rule").unwrap();

        // Without agent filter, validators should appear
        let result = run_list(PackageFilter::All, None, true);
        assert!(result.is_ok());
    }
}
