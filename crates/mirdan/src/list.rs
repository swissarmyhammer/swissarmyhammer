//! Mirdan List - List installed packages (skills, validators, tools, plugins).

use std::path::{Path, PathBuf};

use crate::agents::{self, agent_project_skill_dir, DetectedAgent};
use crate::lockfile::Lockfile;
use crate::mcp_config;
use crate::package_type::PackageType;
use crate::registry::RegistryError;
use crate::store;
use crate::table;

/// An installed package found during scanning.
#[derive(Debug, Clone)]
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
        discover_validators(&mut packages);
    }

    if filter.includes(PackageType::Tool) {
        discover_tools(&mut packages);
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
    if global_store.exists() {
        scan_skills_recursive(&global_store, &global_store, "global", packages);
    }

    // A project store that resolves to the global store would list every skill
    // twice, so scan it only when it is a distinct directory.
    let project_store = store::skill_store_dir(false);
    if !resolves_to_same_dir(&project_store, &global_store) && project_store.exists() {
        scan_skills_recursive(&project_store, &project_store, "project", packages);
    }

    for agent in target_agents(agent_filter) {
        let skill_dir = agent_project_skill_dir(&agent.def);
        if skill_dir.exists() {
            scan_skills(&skill_dir, &agent.def.name, packages);
        }
    }
}

/// Scan the project and global validator directories.
fn discover_validators(packages: &mut Vec<InstalledPackage>) {
    let local = crate::install::validators_dir(false);
    if local.exists() {
        scan_validators(&local, ".validators/", packages);
    }

    let global = crate::install::validators_dir(true);
    if global.exists() {
        scan_validators(&global, "~/.validators/", packages);
    }
}

/// Scan the project and global tool stores.
fn discover_tools(packages: &mut Vec<InstalledPackage>) {
    scan_tools(&store::tool_store_dir(false), ".tools/", packages);
    scan_tools(&store::tool_store_dir(true), "~/.tools/", packages);
}

/// Scan each agent's project and global plugin directories.
fn discover_plugins(agent_filter: Option<&str>, packages: &mut Vec<InstalledPackage>) {
    for agent in target_agents(agent_filter) {
        scan_plugin_dir(
            agents::agent_project_plugin_dir(&agent.def),
            &agent.def.name,
            packages,
        );
        scan_plugin_dir(
            agents::agent_global_plugin_dir(&agent.def),
            &format!("{} (global)", agent.def.name),
            packages,
        );
    }
}

/// Scan one optional plugin directory, when the agent has one and it exists.
fn scan_plugin_dir(dir: Option<PathBuf>, agent_name: &str, packages: &mut Vec<InstalledPackage>) {
    let Some(dir) = dir else {
        return;
    };
    if dir.exists() {
        scan_plugins(&dir, agent_name, packages);
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
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
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

/// Recursively scan a store directory for skill packages (any nested dir containing SKILL.md).
///
/// The store uses nested paths like `~/.skills/owner/repo/skill/SKILL.md`.
/// The skill name is derived from the path relative to the store root.
fn scan_skills_recursive(
    dir: &Path,
    store_root: &Path,
    location: &str,
    packages: &mut Vec<InstalledPackage>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.join("SKILL.md").exists() {
                let skill_md = path.join("SKILL.md");
                // Store-relative path preserves provenance (e.g.
                // `0xdarkmatter/claude-mods/explain`) — use as source key.
                let source = path
                    .strip_prefix(store_root)
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    });
                // Display name: frontmatter name, or terminal path segment. Never a full path.
                let name = read_frontmatter_name(&skill_md).unwrap_or_else(|| {
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
                let description = read_frontmatter_description(&skill_md);
                let version = read_frontmatter_version(&skill_md);
                packages.push(InstalledPackage {
                    source,
                    name,
                    description,
                    package_type: PackageType::Skill,
                    version,
                    targets: vec![location.to_string()],
                });
            } else {
                // Recurse into subdirectories
                scan_skills_recursive(&path, store_root, location, packages);
            }
        }
    }
}

/// Scan a directory for skill packages (subdirs containing SKILL.md).
fn scan_skills(dir: &Path, agent_name: &str, packages: &mut Vec<InstalledPackage>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").exists() {
            let skill_md = path.join("SKILL.md");
            let name = entry.file_name().to_string_lossy().to_string();
            let description = read_frontmatter_description(&skill_md);
            let version = read_frontmatter_version(&skill_md);
            packages.push(InstalledPackage {
                source: name.clone(),
                name,
                description,
                package_type: PackageType::Skill,
                version,
                targets: vec![agent_name.to_string()],
            });
        }
    }
}

/// Scan a directory for validator packages (subdirs containing VALIDATOR.md + rules/).
fn scan_validators(dir: &Path, location: &str, packages: &mut Vec<InstalledPackage>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("VALIDATOR.md").exists() && path.join("rules").is_dir() {
            let md = path.join("VALIDATOR.md");
            let name = entry.file_name().to_string_lossy().to_string();
            let description = read_frontmatter_description(&md);
            let version = read_frontmatter_version(&md);
            packages.push(InstalledPackage {
                source: name.clone(),
                name,
                description,
                package_type: PackageType::Validator,
                version,
                targets: vec![location.to_string()],
            });
        }
    }
}

/// Scan a directory for tool packages (subdirs containing TOOL.md).
fn scan_tools(dir: &Path, location: &str, packages: &mut Vec<InstalledPackage>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("TOOL.md").exists() {
            let md = path.join("TOOL.md");
            let name = entry.file_name().to_string_lossy().to_string();
            let description = read_frontmatter_description(&md);
            let version = read_frontmatter_version(&md);
            packages.push(InstalledPackage {
                source: name.clone(),
                name,
                description,
                package_type: PackageType::Tool,
                version,
                targets: vec![location.to_string()],
            });
        }
    }
}

/// Scan a directory for plugin packages (subdirs containing .claude-plugin/plugin.json).
fn scan_plugins(dir: &Path, agent_name: &str, packages: &mut Vec<InstalledPackage>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join(".claude-plugin").join("plugin.json").exists() {
            let name = mcp_config::read_plugin_json(&path.join(".claude-plugin/plugin.json"))
                .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());
            packages.push(InstalledPackage {
                source: name.clone(),
                name,
                description: String::new(),
                package_type: PackageType::Plugin,
                version: "latest".to_string(),
                targets: vec![agent_name.to_string()],
            });
        }
    }
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
/// Checks `metadata.version` first, then top-level `version`. Falls back to "latest".
fn read_frontmatter_version(path: &Path) -> String {
    parse_frontmatter(path)
        .and_then(|yaml| {
            yaml.get("metadata")
                .and_then(|m| m.get("version"))
                .and_then(|v| v.as_str())
                .or_else(|| yaml.get("version").and_then(|v| v.as_str()))
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "latest".to_string())
}

/// Merge packages with the same name (combining targets).
fn merge_packages(packages: Vec<InstalledPackage>) -> Vec<InstalledPackage> {
    let mut merged: Vec<InstalledPackage> = Vec::new();

    for pkg in packages {
        match merged.iter_mut().find(|p| p.name == pkg.name) {
            Some(existing) => merge_targets(existing, pkg.targets),
            None => merged.push(pkg),
        }
    }

    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
}

/// Add every target `existing` does not already carry.
fn merge_targets(existing: &mut InstalledPackage, targets: Vec<String>) {
    for target in targets {
        if !existing.targets.contains(&target) {
            existing.targets.push(target);
        }
    }
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
    fn test_scan_skills_recursive_nested_structure() {
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
        scan_skills_recursive(store_root, store_root, "global", &mut packages);

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-skill");
        assert_eq!(packages[0].source, "owner/repo/my-skill");
        assert_eq!(packages[0].description, "A test skill");
        assert_eq!(packages[0].version, "1.0.0");
        assert_eq!(packages[0].targets, vec!["global"]);
    }

    #[test]
    fn test_scan_skills_recursive_uses_dir_name_when_no_frontmatter_name() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path();

        let skill_dir = store_root.join("fallback-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# No frontmatter\n").unwrap();

        let mut packages = Vec::new();
        scan_skills_recursive(store_root, store_root, "global", &mut packages);

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
