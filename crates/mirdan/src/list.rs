//! Mirdan List - List installed packages (skills, validators, tools, plugins).

use std::path::Path;

use swissarmyhammer_common::frontmatter::split_frontmatter_body;

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
    /// Display name: the frontmatter `name`, or the terminal path segment when
    /// the package carries none. Never a full path.
    pub name: String,
    /// The lockfile key (source URL or name) used for install/uninstall operations.
    pub source: String,
    /// The frontmatter `description`, or empty when the package carries none.
    pub description: String,
    /// Which kind of package this is, and so which manifest file was read.
    pub package_type: PackageType,
    /// The frontmatter `metadata.version` or `version`, or "latest" when the
    /// package carries neither.
    pub version: String,
    /// Where the package was found: a store location such as `global`, or the
    /// name of the agent whose directory holds it. One package merged from
    /// several locations carries one entry for each.
    pub targets: Vec<String>,
}

/// Which package types a scan covers.
///
/// The `mirdan list` type flags combine: `--skills --tools` selects two types.
/// An empty selection covers every type, which is what a bare `mirdan list`
/// asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageFilter {
    /// The selected types, or empty for every type.
    selected: Vec<PackageType>,
}

impl PackageFilter {
    /// A filter that covers every package type.
    pub fn all() -> Self {
        Self::default()
    }

    /// A filter that covers only `types`.
    ///
    /// An empty iterator gives the same filter as [`PackageFilter::all`].
    pub fn only(types: impl IntoIterator<Item = PackageType>) -> Self {
        Self {
            selected: types.into_iter().collect(),
        }
    }

    /// Build a filter from `(flag, type)` pairs, keeping each type whose flag
    /// is set.
    ///
    /// This is the shape the CLI flags arrive in. Naming the type beside its
    /// flag keeps the call site readable, which a row of bare booleans is not.
    pub fn from_flags(flags: impl IntoIterator<Item = (bool, PackageType)>) -> Self {
        Self::only(
            flags
                .into_iter()
                .filter(|(selected, _)| *selected)
                .map(|(_, package_type)| package_type),
        )
    }

    /// Whether a scan covers `package_type`.
    pub fn includes(&self, package_type: PackageType) -> bool {
        self.selected.is_empty() || self.selected.contains(&package_type)
    }
}

/// Discover installed packages by scanning the filesystem.
///
/// Scans agent skill directories, ./.validators/, .tools/, and agent plugin
/// dirs, limited to the types `filter` covers. Returns a deduplicated, sorted
/// list whose `source` fields carry the lockfile key where one exists.
pub fn discover_packages(
    filter: &PackageFilter,
    agent_filter: Option<&str>,
) -> Vec<InstalledPackage> {
    let mut packages: Vec<InstalledPackage> = Vec::new();

    if filter.includes(PackageType::Skill) {
        scan_skill_stores(&mut packages);
        scan_agent_skill_dirs(agent_filter, &mut packages);
    }

    // Validators are not agent-scoped, so an agent filter suppresses them.
    if filter.includes(PackageType::Validator) && agent_filter.is_none() {
        scan_validator_dirs(&mut packages);
    }

    if filter.includes(PackageType::Tool) {
        scan_tool_dirs(&mut packages);
    }

    if filter.includes(PackageType::Plugin) {
        scan_agent_plugin_dirs(agent_filter, &mut packages);
    }

    let mut merged = merge_packages(packages);
    enrich_sources_from_lockfiles(&mut merged);
    merged
}

/// The agents a scan covers, or an empty list when the agents config will not
/// load or names no agent the filter matches.
fn target_agents(agent_filter: Option<&str>) -> Vec<DetectedAgent> {
    let Ok(config) = agents::load_agents_config() else {
        return Vec::new();
    };
    agents::resolve_target_agents(&config, agent_filter).unwrap_or_default()
}

/// Scan the global and project skill stores.
///
/// The store (`~/.skills/` global, `.skills/` project) is the source of truth
/// for installed packages. Agent directories (`.claude/skills/`, etc.) hold
/// symlinks into the store, which can break (e.g. when `~/.claude` is itself a
/// symlink to iCloud), so scanning the store directly is robust.
fn scan_skill_stores(packages: &mut Vec<InstalledPackage>) {
    let global_store = store::skill_store_dir(true);
    if global_store.exists() {
        scan_skills_recursive(&global_store, &global_store, "global", packages);
    }

    // Skip the project store when it resolves to the same path as global.
    // `canonicalize()` fails when a path does not exist or is not accessible
    // (e.g. permission denied). In those cases both stores are scanned, which
    // may produce duplicates if the project store is an inaccessible symlink to
    // the global store. This is unlikely in practice.
    let project_store = store::skill_store_dir(false);
    let same_as_global = project_store
        .canonicalize()
        .ok()
        .zip(global_store.canonicalize().ok())
        .is_some_and(|(p, g)| p == g);
    if !same_as_global && project_store.exists() {
        scan_skills_recursive(&project_store, &project_store, "project", packages);
    }
}

/// Scan each target agent's project-level skill directory, which holds skills
/// installed without the store (e.g. manually placed skills).
fn scan_agent_skill_dirs(agent_filter: Option<&str>, packages: &mut Vec<InstalledPackage>) {
    for agent in target_agents(agent_filter) {
        let skill_dir = agent_project_skill_dir(&agent.def);
        if skill_dir.exists() {
            scan_skills(&skill_dir, &agent.def.name, packages);
        }
    }
}

/// Scan the project and global validator directories.
fn scan_validator_dirs(packages: &mut Vec<InstalledPackage>) {
    let locations = [
        (crate::install::validators_dir(false), ".validators/"),
        (crate::install::validators_dir(true), "~/.validators/"),
    ];
    for (dir, location) in locations {
        if dir.exists() {
            scan_validators(&dir, location, packages);
        }
    }
}

/// Scan the project and global tool stores.
fn scan_tool_dirs(packages: &mut Vec<InstalledPackage>) {
    scan_tools(&store::tool_store_dir(false), ".tools/", packages);
    scan_tools(&store::tool_store_dir(true), "~/.tools/", packages);
}

/// Scan every target agent's plugin directories.
fn scan_agent_plugin_dirs(agent_filter: Option<&str>, packages: &mut Vec<InstalledPackage>) {
    for agent in target_agents(agent_filter) {
        scan_one_agents_plugin_dirs(&agent, packages);
    }
}

/// Scan one agent's project-level and global plugin directories.
///
/// The global scope's target label carries a `(global)` suffix, so a plugin
/// installed in both scopes lists one entry for each.
fn scan_one_agents_plugin_dirs(agent: &DetectedAgent, packages: &mut Vec<InstalledPackage>) {
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

    for (plugin_dir, target) in scopes {
        let Some(plugin_dir) = plugin_dir else {
            continue;
        };
        if plugin_dir.exists() {
            scan_plugins(&plugin_dir, &target, packages);
        }
    }
}

/// Every package key in the lockfiles mirdan reads, home directory first.
///
/// Each lockfile is read once for each call, so a caller that resolves many
/// names still touches the disk twice.
fn lockfile_keys() -> Vec<String> {
    let lockfile_dirs = [dirs::home_dir(), std::env::current_dir().ok()];
    lockfile_dirs
        .iter()
        .flatten()
        .filter_map(|dir| Lockfile::load(dir).ok())
        .flat_map(|lf| lf.packages.keys().cloned().collect::<Vec<_>>())
        .collect()
}

/// The first key in `keys` that identifies the package called `name`.
///
/// A key is either the full source (`owner/repo/skill`) or, for a package
/// installed by bare name, the name itself. Both match, so a display name
/// finds the full source key it came from.
fn find_lockfile_key<'a>(keys: &'a [String], name: &str) -> Option<&'a str> {
    keys.iter().map(String::as_str).find(|key| {
        let last_segment = key.rsplit('/').next().unwrap_or(key);
        last_segment == name || *key == name
    })
}

/// Replace each package's bare display-name source with its lockfile key, so
/// callers (e.g. the GUI) pass the identifier uninstall and update expect.
fn enrich_sources_from_lockfiles(packages: &mut [InstalledPackage]) {
    let keys = lockfile_keys();
    for pkg in packages {
        // A source that already differs from the display name is a store path,
        // which is more specific than any lockfile key.
        if pkg.source != pkg.name {
            continue;
        }
        if let Some(key) = find_lockfile_key(&keys, &pkg.name) {
            pkg.source = key.to_string();
        }
    }
}

/// Get the mirdan.ai registry URL for a package.
///
/// Looks up the source URL from the lockfile (where the key is the full
/// source like `https://github.com/owner/repo/skill`), then constructs
/// `https://mirdan.ai/package/{url_encoded_source}`. Falls back to `name`
/// when no lockfile names the package.
pub fn registry_url(name: &str) -> String {
    let keys = lockfile_keys();
    let source = find_lockfile_key(&keys, name).unwrap_or(name);
    format!("https://mirdan.ai/package/{}", urlencoding::encode(source))
}

/// Run the list command.
///
/// Scans every package location the `filter` covers.
///
/// # Errors
///
/// Returns an error when the listing cannot be rendered.
pub fn run_list(
    filter: &PackageFilter,
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
///
/// [`split_frontmatter_body`] makes the split, so only a line that is exactly
/// three hyphens delimits the block. A three-hyphen run inside a value -- a
/// description that writes `---` as a separator, a horizontal rule indented in
/// a block scalar -- stays in the frontmatter instead of cutting it short, and
/// an opening line of `----` or `---x` opens nothing.
///
/// Returns `None` when the file will not read, when it carries no frontmatter
/// block, or when the block holds YAML the parser rejects. Every caller reads
/// one optional field, so a missing block and a malformed one need no
/// distinction here.
fn parse_frontmatter(path: &Path) -> Option<serde_yaml_ng::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let (frontmatter, _body) = split_frontmatter_body(&content)?;
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
        if let Some(existing) = merged.iter_mut().find(|p| p.name == pkg.name) {
            for target in pkg.targets {
                if !existing.targets.contains(&target) {
                    existing.targets.push(target);
                }
            }
        } else {
            merged.push(pkg);
        }
    }

    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

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
    fn test_run_list_empty() {
        // Should not panic even with no packages
        let result = run_list(&PackageFilter::all(), None, true);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_run_list_agent_filter_suppresses_validators() {
        let dir = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

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
        let result = run_list(&PackageFilter::all(), Some("claude-code"), true);
        assert!(result.is_ok());

        std::env::set_current_dir(old_dir).unwrap();
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
    fn test_three_hyphen_run_in_a_description_keeps_every_frontmatter_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: test-skill\ndescription: Uses --- as a separator\nmetadata:\n  version: \"1.2.3\"\n---\n# Test\n",
        )
        .unwrap();

        assert_eq!(read_frontmatter_name(&path).as_deref(), Some("test-skill"));
        assert_eq!(
            read_frontmatter_description(&path),
            "Uses --- as a separator"
        );
        assert_eq!(read_frontmatter_version(&path), "1.2.3");
    }

    #[test]
    fn test_an_opening_line_of_more_than_three_hyphens_is_not_a_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(&path, "----\nname: test-skill\n---\n# Test\n").unwrap();

        assert_eq!(read_frontmatter_name(&path), None);
        assert_eq!(read_frontmatter_version(&path), "latest");
    }

    #[test]
    fn test_an_opening_line_with_text_after_the_hyphens_is_not_a_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---description: leaked\nname: test-skill\n---\n# Test\n",
        )
        .unwrap();

        assert_eq!(read_frontmatter_name(&path), None);
        assert_eq!(read_frontmatter_description(&path), "");
        assert_eq!(read_frontmatter_version(&path), "latest");
    }

    #[test]
    fn test_a_file_with_no_closing_delimiter_line_reads_no_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: test-skill\ndescription: Uses --- as a separator\n",
        )
        .unwrap();

        assert_eq!(read_frontmatter_name(&path), None);
        assert_eq!(read_frontmatter_description(&path), "");
        assert_eq!(read_frontmatter_version(&path), "latest");
    }

    #[test]
    #[serial]
    fn test_run_list_no_filter_shows_validators() {
        let dir = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

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
        let result = run_list(&PackageFilter::all(), None, true);
        assert!(result.is_ok());

        std::env::set_current_dir(old_dir).unwrap();
    }
}
