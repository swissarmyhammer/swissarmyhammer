//! Git-based package installation support.
//!
//! Handles source classification, git cloning, and package discovery
//! within cloned repositories. Supports GitHub shorthand (`owner/repo`),
//! HTTPS URLs, SSH URLs, `#ref` fragments, and `@skill-name` suffixes.

use std::path::{Path, PathBuf};

use url::Url;

use crate::package_type::{self, PackageType};
use crate::registry::RegistryError;

/// Classified install source.
#[derive(Debug, PartialEq)]
pub enum InstallSource {
    /// A local filesystem path.
    LocalPath(String),
    /// A git repository to clone.
    GitRepo(GitSource),
    /// A registry package name (possibly with @version).
    Registry(String),
}

/// Parsed git source with all the pieces needed to clone and discover packages.
#[derive(Debug, Clone, PartialEq)]
pub struct GitSource {
    /// The URL to clone (HTTPS or SSH).
    pub clone_url: String,
    /// Optional git ref (branch, tag, commit) from `#ref` fragment.
    pub git_ref: Option<String>,
    /// Optional subpath within the repo.
    pub subpath: Option<String>,
    /// Optional skill/validator name to select from a multi-package repo.
    pub select: Option<String>,
    /// Human-readable display name (e.g. "owner/repo").
    pub display_name: String,
}

/// A package discovered inside a cloned repository.
#[derive(Debug, Clone)]
pub struct DiscoveredPackage {
    /// Package name from frontmatter.
    pub name: String,
    /// Detected package type.
    pub package_type: PackageType,
    /// Path to the package directory.
    pub path: PathBuf,
}

/// Classify a user-provided install spec into a source type.
///
/// 1. Local path? (starts with `./`, `../`, `/`, or is an existing directory)
/// 2. `--git` flag set? → parse as git source directly
/// 3. Everything else → `Registry` (caller handles fallback to git on NotFound)
pub fn classify_source(spec: &str, git_flag: bool) -> InstallSource {
    // Local path check
    if spec.starts_with("./")
        || spec.starts_with("../")
        || spec.starts_with('/')
        || Path::new(spec).is_dir()
    {
        return InstallSource::LocalPath(spec.to_string());
    }

    // --git flag forces git interpretation
    if git_flag {
        match parse_git_source(spec, None) {
            Ok(source) => return InstallSource::GitRepo(source),
            Err(_) => {
                // If parse fails with --git, still return GitRepo with best effort
                return InstallSource::GitRepo(GitSource {
                    clone_url: spec.to_string(),
                    git_ref: None,
                    subpath: None,
                    select: None,
                    display_name: spec.to_string(),
                });
            }
        }
    }

    // Everything else: try registry first, caller falls back to git on NotFound
    InstallSource::Registry(spec.to_string())
}

/// Attempt to parse a spec as a git source.
///
/// Returns `Ok(GitSource)` if the spec looks like a git repo, `Err` otherwise.
///
/// Supported formats:
/// - `owner/repo` (GitHub shorthand)
/// - `owner/repo@skill-name` (shorthand + skill select)
/// - `owner/repo#ref` (shorthand + git ref)
/// - `https://github.com/owner/repo`
/// - `https://github.com/owner/repo.git`
/// - `git@github.com:owner/repo.git` (SSH)
/// - Any URL with `#ref` fragment for branch/tag
pub fn parse_git_source(
    spec: &str,
    skill_override: Option<&str>,
) -> Result<GitSource, RegistryError> {
    let select = skill_override.map(|s| s.to_string());

    // SSH URL: git@host:owner/repo.git
    if spec.starts_with("git@") {
        let display = spec
            .strip_prefix("git@")
            .and_then(|s| s.strip_suffix(".git"))
            .unwrap_or(spec)
            .replace(':', "/");
        return Ok(GitSource {
            clone_url: spec.to_string(),
            git_ref: None,
            subpath: None,
            select,
            display_name: display,
        });
    }

    // Try parsing as a URL
    if let Ok(mut url) = Url::parse(spec) {
        let git_ref = url.fragment().map(|f| f.to_string());
        url.set_fragment(None);

        let mut clone_url = url.to_string();
        // Ensure .git suffix for GitHub/GitLab
        if (clone_url.contains("github.com") || clone_url.contains("gitlab.com"))
            && !clone_url.ends_with(".git")
        {
            clone_url = format!("{}.git", clone_url.trim_end_matches('/'));
        }

        let display = url
            .path()
            .trim_start_matches('/')
            .trim_end_matches(".git")
            .to_string();

        return Ok(GitSource {
            clone_url,
            git_ref,
            subpath: None,
            select,
            display_name: display,
        });
    }

    // GitHub shorthand: owner/repo, owner/repo@skill, owner/repo#ref
    // Must contain exactly one `/` and no spaces
    if !spec.contains(' ') && !spec.contains("://") {
        // Split off #ref first
        let (base, git_ref) = if let Some((b, r)) = spec.split_once('#') {
            (b, Some(r.to_string()))
        } else {
            (spec, None)
        };

        // Split off @skill-name (for shorthand like owner/repo@skill)
        let (base, shorthand_select) = if let Some((b, s)) = base.split_once('@') {
            (b, Some(s.to_string()))
        } else {
            (base, None)
        };

        // --skill override takes precedence over inline @skill
        let final_select = select.or(shorthand_select);

        // Validate it looks like owner/repo
        let parts: Vec<&str> = base.split('/').collect();
        if parts.len() == 2
            && !parts[0].is_empty()
            && !parts[1].is_empty()
            && parts[0]
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
            && parts[1]
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Ok(GitSource {
                clone_url: format!("https://github.com/{}.git", base),
                git_ref,
                subpath: None,
                select: final_select,
                display_name: base.to_string(),
            });
        }
    }

    Err(RegistryError::Validation(format!(
        "Cannot parse '{}' as a git source",
        spec
    )))
}

/// Clone a git repository into a temporary directory.
///
/// Uses `git2::Repository::clone()` for a full clone. If `git_ref` is specified,
/// checks out that branch/tag after cloning.
pub fn git_clone(source: &GitSource) -> Result<tempfile::TempDir, RegistryError> {
    let temp_dir = tempfile::tempdir()?;

    let repo = git2::Repository::clone(&source.clone_url, temp_dir.path())
        .map_err(|e| classify_git_error(e, &source.clone_url))?;

    // Checkout specific ref if requested
    if let Some(ref git_ref) = source.git_ref {
        checkout_ref(&repo, git_ref)?;
    }

    Ok(temp_dir)
}

/// Checkout a specific ref (branch, tag, or commit) in a cloned repo.
fn checkout_ref(repo: &git2::Repository, refspec: &str) -> Result<(), RegistryError> {
    // Try as a branch first (refs/remotes/origin/<name>)
    let remote_ref = format!("refs/remotes/origin/{}", refspec);
    if let Ok(reference) = repo.find_reference(&remote_ref) {
        let commit = reference.peel_to_commit().map_err(|e| {
            RegistryError::Validation(format!("Cannot resolve ref '{}': {}", refspec, e))
        })?;
        repo.set_head_detached(commit.id()).map_err(|e| {
            RegistryError::Validation(format!("Cannot checkout '{}': {}", refspec, e))
        })?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| {
                RegistryError::Validation(format!("Checkout failed for '{}': {}", refspec, e))
            })?;
        return Ok(());
    }

    // Try as a tag (refs/tags/<name>)
    let tag_ref = format!("refs/tags/{}", refspec);
    if let Ok(reference) = repo.find_reference(&tag_ref) {
        let obj = reference.peel(git2::ObjectType::Commit).map_err(|e| {
            RegistryError::Validation(format!("Cannot resolve tag '{}': {}", refspec, e))
        })?;
        repo.set_head_detached(obj.id()).map_err(|e| {
            RegistryError::Validation(format!("Cannot checkout tag '{}': {}", refspec, e))
        })?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| {
                RegistryError::Validation(format!("Checkout failed for '{}': {}", refspec, e))
            })?;
        return Ok(());
    }

    // Try as a commit SHA
    if let Ok(oid) = git2::Oid::from_str(refspec) {
        if repo.find_commit(oid).is_ok() {
            repo.set_head_detached(oid).map_err(|e| {
                RegistryError::Validation(format!("Cannot checkout commit '{}': {}", refspec, e))
            })?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                .map_err(|e| {
                    RegistryError::Validation(format!("Checkout failed for '{}': {}", refspec, e))
                })?;
            return Ok(());
        }
    }

    Err(RegistryError::Validation(format!(
        "Ref '{}' not found in repository",
        refspec
    )))
}

/// Map git2 errors to RegistryError variants.
fn classify_git_error(err: git2::Error, url: &str) -> RegistryError {
    let msg = err.message().to_lowercase();

    if msg.contains("authentication")
        || msg.contains("credentials")
        || msg.contains("401")
        || msg.contains("403")
    {
        return RegistryError::Unauthorized(format!(
            "Authentication failed for '{}': {}",
            url, err
        ));
    }

    if msg.contains("not found")
        || msg.contains("404")
        || msg.contains("does not exist")
        || msg.contains("repository not found")
    {
        return RegistryError::NotFound(format!("Repository not found: '{}'", url));
    }

    if msg.contains("resolve host")
        || msg.contains("dns")
        || msg.contains("name or service not known")
        || msg.contains("could not resolve")
    {
        return RegistryError::Validation(format!("DNS resolution failed for '{}': {}", url, err));
    }

    RegistryError::Validation(format!("Git clone failed for '{}': {}", url, err))
}

/// Priority directories to search for packages within a cloned repo.
const PRIORITY_DIRS: &[&str] = &[
    "skills",
    ".claude/skills",
    "validators",
    ".avp/validators",
    "tools",
    ".tools",
    "plugins",
];

/// Maximum recursion depth when scanning for packages.
const MAX_SCAN_DEPTH: usize = 5;

/// Discover packages (skills and validators) within a cloned repository.
///
/// Search order:
/// 1. Subpath (if provided)
/// 2. Root directory
/// 3. Priority directories (`skills/`, `.claude/skills/`, `validators/`, `.avp/validators/`)
/// 4. Recursive scan (max depth 5)
///
/// Deduplicates by package name.
pub fn discover_packages(
    repo_dir: &Path,
    subpath: Option<&str>,
    select: Option<&str>,
) -> Result<Vec<DiscoveredPackage>, RegistryError> {
    let mut packages = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    // 1. If subpath given, look only there
    if let Some(sub) = subpath {
        let target = repo_dir.join(sub);
        if target.is_dir() {
            scan_dir_for_package(&target, &mut packages, &mut seen_names);
            if !packages.is_empty() {
                return filter_by_select(packages, select);
            }
        }
        return Err(RegistryError::Validation(format!(
            "Subpath '{}' not found or contains no packages",
            sub
        )));
    }

    // 2. Check root
    scan_dir_for_package(repo_dir, &mut packages, &mut seen_names);

    // 3. Check priority directories
    for dir_name in PRIORITY_DIRS {
        let dir = repo_dir.join(dir_name);
        if dir.is_dir() {
            // Each subdirectory in a priority dir might be a package
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        scan_dir_for_package(&path, &mut packages, &mut seen_names);
                    }
                }
            }
        }
    }

    // 4. If still nothing, recursive scan
    if packages.is_empty() {
        scan_recursive(repo_dir, &mut packages, &mut seen_names, 0);
    }

    if packages.is_empty() {
        return Err(RegistryError::Validation(
            "No packages found in repository (expected SKILL.md, VALIDATOR.md + rules/, TOOL.md, or .claude-plugin/plugin.json)"
                .to_string(),
        ));
    }

    filter_by_select(packages, select)
}

/// Check a single directory for a package and add it if found.
fn scan_dir_for_package(
    dir: &Path,
    packages: &mut Vec<DiscoveredPackage>,
    seen: &mut std::collections::HashSet<String>,
) {
    if let Some(pkg_type) = package_type::detect_package_type(dir) {
        let name = match pkg_type {
            PackageType::Skill => {
                let md_file = dir.join("SKILL.md");
                std::fs::read_to_string(&md_file)
                    .ok()
                    .and_then(|c| extract_name_from_frontmatter(&c))
            }
            PackageType::Validator => {
                let md_file = dir.join("VALIDATOR.md");
                std::fs::read_to_string(&md_file)
                    .ok()
                    .and_then(|c| extract_name_from_frontmatter(&c))
            }
            PackageType::Tool => {
                let md_file = dir.join("TOOL.md");
                std::fs::read_to_string(&md_file)
                    .ok()
                    .and_then(|c| extract_name_from_frontmatter(&c))
            }
            PackageType::Plugin => extract_name_from_plugin_json(dir),
        };

        if let Some(name) = name {
            if seen.insert(name.clone()) {
                packages.push(DiscoveredPackage {
                    name,
                    package_type: pkg_type,
                    path: dir.to_path_buf(),
                });
            }
        }
    }
}

/// Extract name from .claude-plugin/plugin.json.
fn extract_name_from_plugin_json(dir: &Path) -> Option<String> {
    let path = dir.join(".claude-plugin").join("plugin.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Recursively scan for packages up to MAX_SCAN_DEPTH.
fn scan_recursive(
    dir: &Path,
    packages: &mut Vec<DiscoveredPackage>,
    seen: &mut std::collections::HashSet<String>,
    depth: usize,
) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }

    // Skip hidden dirs (except .claude, .avp) and common noise
    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if depth > 0 && dir_name.starts_with('.') && dir_name != ".claude" && dir_name != ".avp" {
        return;
    }
    if matches!(dir_name, "node_modules" | "target" | ".git" | "vendor") {
        return;
    }

    scan_dir_for_package(dir, packages, seen);

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_recursive(&path, packages, seen, depth + 1);
            }
        }
    }
}

/// Extract the `name` field from YAML frontmatter.
fn extract_name_from_frontmatter(content: &str) -> Option<String> {
    let content = content.trim();
    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];
    let yaml: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    yaml.get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Filter packages by the `--skill` select option.
fn filter_by_select(
    packages: Vec<DiscoveredPackage>,
    select: Option<&str>,
) -> Result<Vec<DiscoveredPackage>, RegistryError> {
    let Some(name) = select else {
        return Ok(packages);
    };

    let filtered: Vec<_> = packages.into_iter().filter(|p| p.name == name).collect();

    if filtered.is_empty() {
        return Err(RegistryError::NotFound(format!(
            "Package '{}' not found in repository",
            name
        )));
    }

    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- classify_source tests ---

    #[test]
    fn test_classify_local_path_dot_slash() {
        assert_eq!(
            classify_source("./my-skill", false),
            InstallSource::LocalPath("./my-skill".to_string())
        );
    }

    #[test]
    fn test_classify_local_path_dot_dot() {
        assert_eq!(
            classify_source("../other/skill", false),
            InstallSource::LocalPath("../other/skill".to_string())
        );
    }

    #[test]
    fn test_classify_local_path_absolute() {
        assert_eq!(
            classify_source("/tmp/skill", false),
            InstallSource::LocalPath("/tmp/skill".to_string())
        );
    }

    #[test]
    fn test_classify_registry_simple() {
        assert_eq!(
            classify_source("no-secrets", false),
            InstallSource::Registry("no-secrets".to_string())
        );
    }

    #[test]
    fn test_classify_registry_with_version() {
        assert_eq!(
            classify_source("no-secrets@1.0.0", false),
            InstallSource::Registry("no-secrets@1.0.0".to_string())
        );
    }

    #[test]
    fn test_classify_git_flag_url() {
        let result = classify_source("https://github.com/owner/repo", true);
        match result {
            InstallSource::GitRepo(src) => {
                assert!(src.clone_url.contains("github.com/owner/repo"));
            }
            other => panic!("Expected GitRepo, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_git_flag_shorthand() {
        let result = classify_source("owner/repo", true);
        match result {
            InstallSource::GitRepo(src) => {
                assert_eq!(src.clone_url, "https://github.com/owner/repo.git");
                assert_eq!(src.display_name, "owner/repo");
            }
            other => panic!("Expected GitRepo, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_url_without_git_flag_is_registry() {
        // Without --git, URLs go to registry first (caller handles fallback)
        assert_eq!(
            classify_source("https://github.com/owner/repo", false),
            InstallSource::Registry("https://github.com/owner/repo".to_string())
        );
    }

    // --- parse_git_source tests ---

    #[test]
    fn test_parse_github_shorthand() {
        let src = parse_git_source("owner/repo", None).unwrap();
        assert_eq!(src.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(src.display_name, "owner/repo");
        assert_eq!(src.git_ref, None);
        assert_eq!(src.select, None);
    }

    #[test]
    fn test_parse_github_shorthand_with_skill() {
        let src = parse_git_source("owner/repo@my-skill", None).unwrap();
        assert_eq!(src.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(src.select, Some("my-skill".to_string()));
    }

    #[test]
    fn test_parse_github_shorthand_with_ref() {
        let src = parse_git_source("owner/repo#main", None).unwrap();
        assert_eq!(src.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(src.git_ref, Some("main".to_string()));
    }

    #[test]
    fn test_parse_github_shorthand_skill_override() {
        let src = parse_git_source("owner/repo@inline-skill", Some("override-skill")).unwrap();
        // --skill override takes precedence over inline @skill
        assert_eq!(src.select, Some("override-skill".to_string()));
        assert_eq!(src.clone_url, "https://github.com/owner/repo.git");
    }

    #[test]
    fn test_parse_https_url() {
        let src = parse_git_source("https://github.com/owner/repo", None).unwrap();
        assert_eq!(src.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(src.display_name, "owner/repo");
    }

    #[test]
    fn test_parse_https_url_with_git_suffix() {
        let src = parse_git_source("https://github.com/owner/repo.git", None).unwrap();
        assert_eq!(src.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(src.display_name, "owner/repo");
    }

    #[test]
    fn test_parse_https_url_with_fragment() {
        let src = parse_git_source("https://github.com/owner/repo#v1.0", None).unwrap();
        assert_eq!(src.git_ref, Some("v1.0".to_string()));
        // Fragment should be stripped from clone URL
        assert!(!src.clone_url.contains('#'));
    }

    #[test]
    fn test_parse_ssh_url() {
        let src = parse_git_source("git@github.com:owner/repo.git", None).unwrap();
        assert_eq!(src.clone_url, "git@github.com:owner/repo.git");
        assert!(src.display_name.contains("owner/repo"));
    }

    #[test]
    fn test_parse_gitlab_url() {
        let src = parse_git_source("https://gitlab.com/owner/repo", None).unwrap();
        assert_eq!(src.clone_url, "https://gitlab.com/owner/repo.git");
    }

    #[test]
    fn test_parse_non_github_url() {
        let src = parse_git_source("https://my-server.com/repo.git", None).unwrap();
        assert_eq!(src.clone_url, "https://my-server.com/repo.git");
    }

    #[test]
    fn test_parse_invalid_spec() {
        assert!(parse_git_source("just-a-name", None).is_err());
    }

    #[test]
    fn test_parse_skill_override_on_url() {
        let src = parse_git_source("https://github.com/owner/repo", Some("my-skill")).unwrap();
        assert_eq!(src.select, Some("my-skill".to_string()));
    }

    // --- classify_git_error tests ---

    #[test]
    fn test_classify_auth_error() {
        let err = git2::Error::new(
            git2::ErrorCode::Auth,
            git2::ErrorClass::Net,
            "authentication required",
        );
        let result = classify_git_error(err, "https://example.com/repo.git");
        assert!(matches!(result, RegistryError::Unauthorized(_)));
    }

    #[test]
    fn test_classify_not_found_error() {
        let err = git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Net,
            "repository not found",
        );
        let result = classify_git_error(err, "https://example.com/repo.git");
        assert!(matches!(result, RegistryError::NotFound(_)));
    }

    #[test]
    fn test_classify_dns_error() {
        let err = git2::Error::new(
            git2::ErrorCode::GenericError,
            git2::ErrorClass::Net,
            "failed to resolve host",
        );
        let result = classify_git_error(err, "https://example.com/repo.git");
        assert!(matches!(result, RegistryError::Validation(_)));
    }

    #[test]
    fn test_classify_generic_error() {
        let err = git2::Error::new(
            git2::ErrorCode::GenericError,
            git2::ErrorClass::None,
            "something else went wrong",
        );
        let result = classify_git_error(err, "https://example.com/repo.git");
        assert!(matches!(result, RegistryError::Validation(_)));
    }

    // --- discover_packages tests ---

    #[test]
    fn test_discover_skill_in_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: root-skill\nmetadata:\n  version: \"1.0.0\"\n---\n# Skill\n",
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "root-skill");
        assert_eq!(pkgs[0].package_type, PackageType::Skill);
    }

    #[test]
    fn test_discover_validator_in_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("VALIDATOR.md"),
            "---\nname: root-val\n---\n# Validator\n",
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("rules")).unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "root-val");
        assert_eq!(pkgs[0].package_type, PackageType::Validator);
    }

    #[test]
    fn test_discover_multiple_in_skills_dir() {
        let dir = tempfile::tempdir().unwrap();
        let skills = dir.path().join("skills");
        std::fs::create_dir(&skills).unwrap();

        let s1 = skills.join("skill-one");
        std::fs::create_dir(&s1).unwrap();
        std::fs::write(s1.join("SKILL.md"), "---\nname: skill-one\n---\n# One\n").unwrap();

        let s2 = skills.join("skill-two");
        std::fs::create_dir(&s2).unwrap();
        std::fs::write(s2.join("SKILL.md"), "---\nname: skill-two\n---\n# Two\n").unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 2);
        let names: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"skill-one"));
        assert!(names.contains(&"skill-two"));
    }

    #[test]
    fn test_discover_with_subpath() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub").join("pkg");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("SKILL.md"), "---\nname: sub-skill\n---\n# Sub\n").unwrap();

        let pkgs = discover_packages(dir.path(), Some("sub/pkg"), None).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "sub-skill");
    }

    #[test]
    fn test_discover_select_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let skills = dir.path().join("skills");
        std::fs::create_dir(&skills).unwrap();

        let s1 = skills.join("a");
        std::fs::create_dir(&s1).unwrap();
        std::fs::write(s1.join("SKILL.md"), "---\nname: alpha\n---\n# A\n").unwrap();

        let s2 = skills.join("b");
        std::fs::create_dir(&s2).unwrap();
        std::fs::write(s2.join("SKILL.md"), "---\nname: beta\n---\n# B\n").unwrap();

        let pkgs = discover_packages(dir.path(), None, Some("beta")).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "beta");
    }

    #[test]
    fn test_discover_select_not_found() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: my-skill\n---\n# Skill\n",
        )
        .unwrap();

        let result = discover_packages(dir.path(), None, Some("nonexistent"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));
    }

    #[test]
    fn test_discover_empty_repo() {
        let dir = tempfile::tempdir().unwrap();
        let result = discover_packages(dir.path(), None, None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RegistryError::Validation(_)));
    }

    #[test]
    fn test_discover_deduplicates_by_name() {
        let dir = tempfile::tempdir().unwrap();

        // Same skill in root
        std::fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: dupe-skill\n---\n# Root\n",
        )
        .unwrap();

        // Same name in skills/ subdirectory
        let skills = dir.path().join("skills").join("dupe-skill");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(
            skills.join("SKILL.md"),
            "---\nname: dupe-skill\n---\n# Sub\n",
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "dupe-skill");
    }

    #[test]
    fn test_discover_recursive_scan() {
        let dir = tempfile::tempdir().unwrap();
        // Package nested deep (not in priority dirs)
        let nested = dir.path().join("packages").join("inner");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("SKILL.md"),
            "---\nname: deep-skill\n---\n# Deep\n",
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "deep-skill");
    }

    #[test]
    fn test_discover_skips_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git").join("hooks");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(
            git_dir.join("SKILL.md"),
            "---\nname: hidden-skill\n---\n# Hidden\n",
        )
        .unwrap();

        // Only the .git skill (which should be skipped), so no packages found
        let result = discover_packages(dir.path(), None, None);
        assert!(result.is_err());
    }

    // --- extract_name_from_frontmatter tests ---

    #[test]
    fn test_extract_name_valid() {
        let content = "---\nname: my-skill\nmetadata:\n  version: \"1.0.0\"\n---\n# Skill\n";
        assert_eq!(
            extract_name_from_frontmatter(content),
            Some("my-skill".to_string())
        );
    }

    #[test]
    fn test_extract_name_no_frontmatter() {
        assert_eq!(extract_name_from_frontmatter("# Just markdown"), None);
    }

    #[test]
    fn test_extract_name_no_name_field() {
        let content = "---\nmetadata:\n  version: \"1.0.0\"\n---\n# Skill\n";
        assert_eq!(extract_name_from_frontmatter(content), None);
    }

    // --- integration tests (require network) ---
    //
    // These clone real public repos to verify the full git pipeline:
    // parse → clone → discover → select → cleanup.

    #[test]
    fn test_clone_anthropics_skills_https_url() {
        let source = parse_git_source("https://github.com/anthropics/skills", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();

        // Must be a non-trivial clone
        assert!(temp_dir.path().join(".git").is_dir());
        let packages = discover_packages(temp_dir.path(), None, None).unwrap();
        assert!(
            packages.len() >= 2,
            "anthropics/skills should contain multiple skills, found {}",
            packages.len()
        );
        // Every discovered package must be a Skill with a non-empty name
        for pkg in &packages {
            assert_eq!(pkg.package_type, PackageType::Skill);
            assert!(!pkg.name.is_empty());
            assert!(
                pkg.path.join("SKILL.md").exists(),
                "SKILL.md missing for {}",
                pkg.name
            );
        }
    }

    #[test]
    fn test_clone_anthropics_skills_shorthand() {
        let source = parse_git_source("anthropics/skills", None).unwrap();
        assert_eq!(source.clone_url, "https://github.com/anthropics/skills.git");
        let temp_dir = git_clone(&source).unwrap();
        let packages = discover_packages(temp_dir.path(), None, None).unwrap();
        assert!(
            packages.len() >= 2,
            "shorthand should produce same result as full URL"
        );
    }

    #[test]
    fn test_clone_anthropics_skills_select_one() {
        let source = parse_git_source("anthropics/skills", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();

        // Discover all, then select the first by name
        let all = discover_packages(temp_dir.path(), None, None).unwrap();
        assert!(!all.is_empty());
        let target_name = &all[0].name;

        let filtered = discover_packages(temp_dir.path(), None, Some(target_name)).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(&filtered[0].name, target_name);
    }

    #[test]
    fn test_clone_anthropics_skills_select_nonexistent() {
        let source = parse_git_source("anthropics/skills", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();
        let result = discover_packages(temp_dir.path(), None, Some("zzz-does-not-exist"));
        assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));
    }

    #[test]
    fn test_clone_anthropics_skills_frontmatter_is_valid() {
        // Every discovered skill must have parseable frontmatter with a name
        let source = parse_git_source("anthropics/skills", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();
        let packages = discover_packages(temp_dir.path(), None, None).unwrap();
        for pkg in &packages {
            let content = std::fs::read_to_string(pkg.path.join("SKILL.md")).unwrap();
            let name = extract_name_from_frontmatter(&content);
            assert_eq!(
                name.as_deref(),
                Some(pkg.name.as_str()),
                "frontmatter name mismatch for {:?}",
                pkg.path
            );
        }
    }

    #[test]
    fn test_clone_basecamp_skills_discovers_packages() {
        let source = parse_git_source("basecamp/skills", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();
        let packages = discover_packages(temp_dir.path(), None, None).unwrap();
        assert!(
            !packages.is_empty(),
            "basecamp/skills should contain at least one package"
        );
        for pkg in &packages {
            assert!(!pkg.name.is_empty());
        }
    }

    #[test]
    fn test_clone_nonexistent_repo_returns_error() {
        let source = parse_git_source(
            "https://github.com/this-owner-does-not-exist-xyz/no-repo-here",
            None,
        )
        .unwrap();
        let result = git_clone(&source);
        assert!(result.is_err(), "Cloning a nonexistent repo must fail");
    }

    #[test]
    fn test_clone_temp_dir_cleanup_on_drop() {
        let source = parse_git_source("anthropics/skills", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();
        let path = temp_dir.path().to_path_buf();
        assert!(path.exists());
        drop(temp_dir);
        assert!(
            !path.exists(),
            "Temp dir must be cleaned up when TempDir drops"
        );
    }

    // --- plugin discovery from real git repos ---

    #[test]
    fn test_clone_obra_superpowers_discovers_plugin() {
        let source = parse_git_source("obra/superpowers", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();
        let packages = discover_packages(temp_dir.path(), None, None).unwrap();

        // obra/superpowers has .claude-plugin/plugin.json at root → Plugin
        let plugins: Vec<_> = packages
            .iter()
            .filter(|p| p.package_type == PackageType::Plugin)
            .collect();
        assert!(
            !plugins.is_empty(),
            "obra/superpowers should contain at least one Plugin, found types: {:?}",
            packages.iter().map(|p| &p.package_type).collect::<Vec<_>>()
        );

        // The root plugin should have name "superpowers"
        let sp = plugins.iter().find(|p| p.name == "superpowers");
        assert!(
            sp.is_some(),
            "Expected plugin named 'superpowers', found: {:?}",
            plugins.iter().map(|p| &p.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_clone_obra_superpowers_discovers_mixed_types() {
        // obra/superpowers has both .claude-plugin/plugin.json AND skills/ with SKILL.md files
        let source = parse_git_source("obra/superpowers", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();
        let packages = discover_packages(temp_dir.path(), None, None).unwrap();

        let types: std::collections::HashSet<_> =
            packages.iter().map(|p| format!("{}", p.package_type)).collect();

        assert!(
            types.contains("plugin"),
            "Should discover Plugin type, found: {:?}",
            types
        );
        assert!(
            types.contains("skill"),
            "Should discover Skill type, found: {:?}",
            types
        );
        assert!(
            packages.len() >= 2,
            "Should have at least 2 packages (1 plugin + skills), found {}",
            packages.len()
        );

        // Names should all be unique (deduplication works)
        let names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
        let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "Package names should be unique: {:?}",
            names
        );
    }

    #[test]
    fn test_clone_anthropics_plugins_discovers_multiple_plugins() {
        // anthropics/claude-plugins-official is a marketplace repo.
        // The plugins/ directory is a PRIORITY_DIR, so the scanner finds
        // all plugins inside it. external_plugins/ is not a priority dir
        // and the recursive scan is skipped once plugins/ yields results.
        let source =
            parse_git_source("anthropics/claude-plugins-official", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();
        let packages = discover_packages(temp_dir.path(), None, None).unwrap();

        let plugins: Vec<_> = packages
            .iter()
            .filter(|p| p.package_type == PackageType::Plugin)
            .collect();

        // plugins/ has 29+ entries; we should find many of them
        assert!(
            plugins.len() >= 10,
            "Marketplace should contain many plugins, found {}",
            plugins.len()
        );

        // Spot-check known plugins from plugins/ (not external_plugins/)
        let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"example-plugin"),
            "Should find example-plugin, found: {:?}",
            names
        );
        assert!(
            names.contains(&"code-review"),
            "Should find code-review plugin, found: {:?}",
            names
        );

        // Every discovered plugin should have a valid plugin.json
        for pkg in &plugins {
            let pj = pkg.path.join(".claude-plugin/plugin.json");
            assert!(
                pj.exists(),
                "plugin.json missing for {} at {:?}",
                pkg.name,
                pkg.path
            );
            let content = std::fs::read_to_string(&pj).unwrap();
            let json: serde_json::Value = serde_json::from_str(&content).unwrap();
            assert!(
                json.get("name").is_some(),
                "plugin.json should have name field for {}",
                pkg.name
            );
        }
    }

    #[test]
    fn test_clone_anthropics_plugins_select_one() {
        let source =
            parse_git_source("anthropics/claude-plugins-official", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();

        // Select "example-plugin" (lives in plugins/, a PRIORITY_DIR)
        let filtered =
            discover_packages(temp_dir.path(), None, Some("example-plugin")).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "example-plugin");
        assert_eq!(filtered[0].package_type, PackageType::Plugin);
    }

    #[test]
    fn test_clone_anthropics_plugins_select_nonexistent() {
        let source =
            parse_git_source("anthropics/claude-plugins-official", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();

        let result =
            discover_packages(temp_dir.path(), None, Some("zzz-not-a-plugin"));
        assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));
    }

    // --- tool + plugin discovery from tempdir fixtures ---

    #[test]
    fn test_discover_tool_in_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("TOOL.md"),
            "---\nname: root-tool\nmetadata:\n  version: \"1.0.0\"\nmcp:\n  command: echo\n  args: [\"hello\"]\n---\n# Tool\n",
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "root-tool");
        assert_eq!(pkgs[0].package_type, PackageType::Tool);
    }

    #[test]
    fn test_discover_tool_in_tools_dir() {
        let dir = tempfile::tempdir().unwrap();
        let tools = dir.path().join("tools");
        std::fs::create_dir(&tools).unwrap();

        let t1 = tools.join("tool-one");
        std::fs::create_dir(&t1).unwrap();
        std::fs::write(
            t1.join("TOOL.md"),
            "---\nname: tool-one\nmcp:\n  command: echo\n---\n# One\n",
        )
        .unwrap();

        let t2 = tools.join("tool-two");
        std::fs::create_dir(&t2).unwrap();
        std::fs::write(
            t2.join("TOOL.md"),
            "---\nname: tool-two\nmcp:\n  command: echo\n---\n# Two\n",
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 2);
        let names: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"tool-one"));
        assert!(names.contains(&"tool-two"));
    }

    #[test]
    fn test_discover_plugin_in_root() {
        let dir = tempfile::tempdir().unwrap();
        let pm = dir.path().join(".claude-plugin");
        std::fs::create_dir(&pm).unwrap();
        std::fs::write(
            pm.join("plugin.json"),
            r#"{"name": "root-plugin", "description": "test"}"#,
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "root-plugin");
        assert_eq!(pkgs[0].package_type, PackageType::Plugin);
    }

    #[test]
    fn test_discover_plugin_in_plugins_dir() {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("plugins");
        std::fs::create_dir(&plugins).unwrap();

        let p1 = plugins.join("plugin-a");
        std::fs::create_dir_all(p1.join(".claude-plugin")).unwrap();
        std::fs::write(
            p1.join(".claude-plugin/plugin.json"),
            r#"{"name": "plugin-a", "description": "a"}"#,
        )
        .unwrap();

        let p2 = plugins.join("plugin-b");
        std::fs::create_dir_all(p2.join(".claude-plugin")).unwrap();
        std::fs::write(
            p2.join(".claude-plugin/plugin.json"),
            r#"{"name": "plugin-b", "description": "b"}"#,
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 2);
        let names: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"plugin-a"));
        assert!(names.contains(&"plugin-b"));
    }

    #[test]
    fn test_discover_all_four_types_in_repo() {
        let dir = tempfile::tempdir().unwrap();

        // Root has a skill
        std::fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: my-skill\n---\n# Skill\n",
        )
        .unwrap();

        // validators/ has a validator
        let val_dir = dir.path().join("validators").join("my-val");
        std::fs::create_dir_all(val_dir.join("rules")).unwrap();
        std::fs::write(
            val_dir.join("VALIDATOR.md"),
            "---\nname: my-val\n---\n# Val\n",
        )
        .unwrap();

        // tools/ has a tool
        let tool_dir = dir.path().join("tools").join("my-tool");
        std::fs::create_dir(&dir.path().join("tools")).unwrap();
        std::fs::create_dir(&tool_dir).unwrap();
        std::fs::write(
            tool_dir.join("TOOL.md"),
            "---\nname: my-tool\nmcp:\n  command: echo\n---\n# Tool\n",
        )
        .unwrap();

        // plugins/ has a plugin
        let plugin_dir = dir.path().join("plugins").join("my-plugin");
        std::fs::create_dir(&dir.path().join("plugins")).unwrap();
        std::fs::create_dir_all(plugin_dir.join(".claude-plugin")).unwrap();
        std::fs::write(
            plugin_dir.join(".claude-plugin/plugin.json"),
            r#"{"name": "my-plugin", "description": "test"}"#,
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, None).unwrap();
        assert_eq!(pkgs.len(), 4, "Should find all 4 types: {:?}", pkgs);

        let types: std::collections::HashSet<String> =
            pkgs.iter().map(|p| format!("{}", p.package_type)).collect();
        assert!(types.contains("skill"));
        assert!(types.contains("validator"));
        assert!(types.contains("tool"));
        assert!(types.contains("plugin"));
    }

    #[test]
    fn test_discover_select_plugin_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("plugins");
        std::fs::create_dir(&plugins).unwrap();

        let p1 = plugins.join("alpha");
        std::fs::create_dir_all(p1.join(".claude-plugin")).unwrap();
        std::fs::write(
            p1.join(".claude-plugin/plugin.json"),
            r#"{"name": "alpha", "description": "a"}"#,
        )
        .unwrap();

        let p2 = plugins.join("beta");
        std::fs::create_dir_all(p2.join(".claude-plugin")).unwrap();
        std::fs::write(
            p2.join(".claude-plugin/plugin.json"),
            r#"{"name": "beta", "description": "b"}"#,
        )
        .unwrap();

        let pkgs = discover_packages(dir.path(), None, Some("beta")).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "beta");
        assert_eq!(pkgs[0].package_type, PackageType::Plugin);
    }
}
