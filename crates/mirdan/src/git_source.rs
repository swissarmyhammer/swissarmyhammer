//! Git-based package installation support.
//!
//! Handles source classification, git cloning, and package discovery
//! within cloned repositories. Supports GitHub shorthand (`owner/repo`),
//! HTTPS URLs, SSH URLs, `#ref` fragments, and `@skill-name` suffixes.

use std::path::{Component, Path, PathBuf};

use url::Url;

use crate::frontmatter;
use crate::package_type::{self, PackageType};
use crate::registry::RegistryError;

/// Classified install source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstallSource {
    /// A local filesystem path.
    LocalPath(String),
    /// A git repository to clone.
    GitRepo(GitSource),
    /// A registry package name (possibly with @version).
    Registry(String),
}

/// Parsed git source with all the pieces needed to clone and discover packages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    parse_ssh_source(spec, skill_override)
        .or_else(|| parse_url_source(spec, skill_override))
        .or_else(|| parse_shorthand_source(spec, skill_override))
        .ok_or_else(|| {
            RegistryError::Validation(format!("cannot parse '{}' as a git source", spec))
        })
}

/// Parse an SSH spec, `git@host:owner/repo.git`.
///
/// Returns `None` when `spec` does not open with `git@`.
fn parse_ssh_source(spec: &str, select: Option<&str>) -> Option<GitSource> {
    let rest = spec.strip_prefix("git@")?;
    let display = rest.strip_suffix(".git").unwrap_or(rest).replace(':', "/");

    Some(GitSource {
        clone_url: spec.to_string(),
        git_ref: None,
        subpath: None,
        select: select.map(str::to_string),
        display_name: display,
    })
}

/// Parse a full URL spec, with an optional `#ref` fragment naming a branch,
/// tag, or commit.
///
/// Returns `None` when `spec` is not a URL.
fn parse_url_source(spec: &str, select: Option<&str>) -> Option<GitSource> {
    let mut url = Url::parse(spec).ok()?;
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

    Some(GitSource {
        clone_url,
        git_ref,
        subpath: None,
        select: select.map(str::to_string),
        display_name: display,
    })
}

/// Parse a GitHub shorthand spec: `owner/repo`, `owner/repo@skill`, or
/// `owner/repo#ref`.
///
/// `select` is the `--skill` flag, which takes precedence over an inline
/// `@skill` suffix. Returns `None` when `spec` carries a space or a URL
/// scheme, or when the base is not one `owner/repo` pair.
fn parse_shorthand_source(spec: &str, select: Option<&str>) -> Option<GitSource> {
    if spec.contains(' ') || spec.contains("://") {
        return None;
    }

    let (base, git_ref) = split_once_owned(spec, '#');
    let (base, shorthand_select) = split_once_owned(base, '@');

    let (owner, repo) = base.split_once('/')?;
    if !is_shorthand_segment(owner) || !is_shorthand_segment(repo) {
        return None;
    }

    Some(GitSource {
        clone_url: format!("https://github.com/{}.git", base),
        git_ref,
        subpath: None,
        select: select.map(str::to_string).or(shorthand_select),
        display_name: base.to_string(),
    })
}

/// Split `spec` at the first `separator`, owning the tail.
///
/// Returns the whole of `spec` and `None` when `separator` is absent.
fn split_once_owned(spec: &str, separator: char) -> (&str, Option<String>) {
    match spec.split_once(separator) {
        Some((head, tail)) => (head, Some(tail.to_string())),
        None => (spec, None),
    }
}

/// Whether `segment` is a usable owner or repository name in GitHub
/// shorthand.
fn is_shorthand_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Clone a git repository into a temporary directory.
///
/// Prefers a shallow (`depth = 1`) clone: the package scanner only reads the
/// checked-out working tree, never history, so a single commit is sufficient.
/// This cuts the transfer for large marketplace repos dramatically, speeding up
/// both real installs and the integration tests that clone public repos.
///
/// Shallow cloning is used only when it is safe:
///   * If a specific `git_ref` is pinned it may name a branch/tag/commit that a
///     depth-1 clone of the default branch would not contain, so that (rare)
///     case falls back to a full clone followed by a checkout of that ref.
///   * libgit2 does not support shallow clones of local (`file://`) remotes, so
///     those also take the full-clone path (they are already fast).
pub fn git_clone(source: &GitSource) -> Result<tempfile::TempDir, RegistryError> {
    let temp_dir = tempfile::tempdir()?;

    let shallow = source.git_ref.is_none() && !is_local_remote(&source.clone_url);

    let repo = if shallow {
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.depth(1);
        git2::build::RepoBuilder::new()
            .fetch_options(fetch_options)
            .clone(&source.clone_url, temp_dir.path())
            .map_err(|e| classify_git_error(e, &source.clone_url))?
    } else {
        git2::Repository::clone(&source.clone_url, temp_dir.path())
            .map_err(|e| classify_git_error(e, &source.clone_url))?
    };

    // Checkout specific ref if requested
    if let Some(ref git_ref) = source.git_ref {
        checkout_ref(&repo, git_ref)?;
    }

    Ok(temp_dir)
}

/// Whether `url` points at a local repository rather than a network remote.
///
/// libgit2 cannot perform shallow clones of local remotes, so [`git_clone`]
/// uses these to decide whether `depth = 1` is safe to request.
fn is_local_remote(url: &str) -> bool {
    url.starts_with("file://") || url.starts_with('/') || url.starts_with('.')
}

/// Checkout a specific ref (branch, tag, or commit) in a cloned repo.
fn checkout_ref(repo: &git2::Repository, refspec: &str) -> Result<(), RegistryError> {
    // Try as a branch first (refs/remotes/origin/<name>)
    let remote_ref = format!("refs/remotes/origin/{}", refspec);
    if let Ok(reference) = repo.find_reference(&remote_ref) {
        let commit = reference.peel_to_commit().map_err(|e| {
            RegistryError::Validation(format!("cannot resolve ref '{}': {}", refspec, e))
        })?;
        repo.set_head_detached(commit.id()).map_err(|e| {
            RegistryError::Validation(format!("cannot checkout '{}': {}", refspec, e))
        })?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| {
                RegistryError::Validation(format!("checkout failed for '{}': {}", refspec, e))
            })?;
        return Ok(());
    }

    // Try as a tag (refs/tags/<name>)
    let tag_ref = format!("refs/tags/{}", refspec);
    if let Ok(reference) = repo.find_reference(&tag_ref) {
        let obj = reference.peel(git2::ObjectType::Commit).map_err(|e| {
            RegistryError::Validation(format!("cannot resolve tag '{}': {}", refspec, e))
        })?;
        repo.set_head_detached(obj.id()).map_err(|e| {
            RegistryError::Validation(format!("cannot checkout tag '{}': {}", refspec, e))
        })?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| {
                RegistryError::Validation(format!("checkout failed for '{}': {}", refspec, e))
            })?;
        return Ok(());
    }

    // Try as a commit SHA
    if let Ok(oid) = git2::Oid::from_str(refspec) {
        if repo.find_commit(oid).is_ok() {
            repo.set_head_detached(oid).map_err(|e| {
                RegistryError::Validation(format!("cannot checkout commit '{}': {}", refspec, e))
            })?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                .map_err(|e| {
                    RegistryError::Validation(format!("checkout failed for '{}': {}", refspec, e))
                })?;
            return Ok(());
        }
    }

    Err(RegistryError::Validation(format!(
        "ref '{}' not found in repository",
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
            "authentication failed for '{}': {}",
            url, err
        ));
    }

    if msg.contains("not found")
        || msg.contains("404")
        || msg.contains("does not exist")
        || msg.contains("repository not found")
    {
        return RegistryError::NotFound(format!("repository not found: '{}'", url));
    }

    if msg.contains("resolve host")
        || msg.contains("dns")
        || msg.contains("name or service not known")
        || msg.contains("could not resolve")
    {
        return RegistryError::Validation(format!("dns resolution failed for '{}': {}", url, err));
    }

    RegistryError::Validation(format!("git clone failed for '{}': {}", url, err))
}

/// Priority directories to search for packages within a cloned repo.
const PRIORITY_DIRS: &[&str] = &[
    "skills",
    ".claude/skills",
    "validators",
    ".validators",
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
/// 3. Priority directories (`skills/`, `.claude/skills/`, `validators/`, `.validators/`)
/// 4. Recursive scan (max depth 5)
///
/// Deduplicates by package name.
///
/// Every directory the search reads stays inside the clone.
///
/// # Errors
///
/// Returns an error when `repo_dir` does not resolve to a real directory,
/// when the repository holds no package, or when `subpath` or `select` names
/// none.
pub fn discover_packages(
    repo_dir: &Path,
    subpath: Option<&str>,
    select: Option<&str>,
) -> Result<Vec<DiscoveredPackage>, RegistryError> {
    if let Some(sub) = subpath {
        return discover_in_subpath(repo_dir, sub, select);
    }

    let mut scan = RepoScan::open(repo_dir)?;

    // 1. Check root
    scan.scan_dir(repo_dir);

    // 2. Check priority directories
    for dir_name in PRIORITY_DIRS {
        scan.scan_child_dirs(&repo_dir.join(dir_name));
    }

    // 3. If still nothing, recursive scan
    if scan.is_empty() {
        scan.scan_recursive(repo_dir, 0);
    }

    if scan.is_empty() {
        return Err(RegistryError::Validation(
            "no packages found in repository (expected SKILL.md, VALIDATOR.md + rules/, TOOL.md, or .claude-plugin/plugin.json)"
                .to_string(),
        ));
    }

    filter_by_select(scan.into_packages(), select)
}

/// Discover packages under one subpath of a cloned repository.
///
/// The subpath names one package directory, so the search stops there instead
/// of falling back to the priority directories or a recursive scan.
///
/// The subpath is untrusted text -- it comes from the install spec -- and it
/// indexes a third-party repository. Two checks hold the search inside the
/// clone: [`subpath_stays_inside`] refuses text that names a location outside
/// it, and [`RepoScan`] refuses a directory that resolves outside it once
/// every symbolic link is followed.
///
/// # Errors
///
/// Returns an error when the subpath leaves the repository, when it is not a
/// directory, when it holds no package, or when `select` names no package it
/// holds.
fn discover_in_subpath(
    repo_dir: &Path,
    subpath: &str,
    select: Option<&str>,
) -> Result<Vec<DiscoveredPackage>, RegistryError> {
    if !subpath_stays_inside(subpath) {
        return Err(RegistryError::Validation(format!(
            "subpath '{}' leaves the repository",
            subpath
        )));
    }

    let mut scan = RepoScan::open(repo_dir)?;
    scan.scan_dir(&repo_dir.join(subpath));

    if scan.is_empty() {
        return Err(RegistryError::Validation(format!(
            "subpath '{}' not found or contains no packages",
            subpath
        )));
    }

    filter_by_select(scan.into_packages(), select)
}

/// Whether the text of `subpath` names a location inside the repository.
///
/// Only a relative path of ordinary segments can stay inside the clone. A
/// leading separator, a drive prefix, and a `..` segment each name something
/// outside it.
fn subpath_stays_inside(subpath: &str) -> bool {
    Path::new(subpath)
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// One walk of a cloned repository, collecting the packages it holds.
///
/// A cloned repository is third-party content, so a symbolic link it carries
/// may point anywhere on the host. The scan owns the canonical repository
/// root and resolves every directory against it, so a walk that would leave
/// the clone reads nothing.
struct RepoScan {
    /// The canonical repository root. Every directory read stays inside it.
    root: PathBuf,
    /// The packages collected so far, in discovery order.
    packages: Vec<DiscoveredPackage>,
    /// The package names already collected, so each package is reported once.
    seen: std::collections::HashSet<String>,
}

impl RepoScan {
    /// Open a scan of the repository checked out at `repo_dir`.
    ///
    /// # Errors
    ///
    /// Returns an error when `repo_dir` does not resolve to a real directory.
    fn open(repo_dir: &Path) -> Result<Self, RegistryError> {
        let root = repo_dir.canonicalize().map_err(|e| {
            RegistryError::Validation(format!(
                "cannot read repository directory '{}': {}",
                repo_dir.display(),
                e
            ))
        })?;

        Ok(Self {
            root,
            packages: Vec::new(),
            seen: std::collections::HashSet::new(),
        })
    }

    /// Whether `dir` resolves to a location inside the repository.
    ///
    /// Resolution follows every symbolic link, so a link that points out of
    /// the clone answers `false`. A directory that resolves to nothing -- one
    /// that is missing, or that is not a directory -- also answers `false`,
    /// because the scan has nothing to read there.
    fn contains(&self, dir: &Path) -> bool {
        dir.canonicalize()
            .is_ok_and(|resolved| resolved.starts_with(&self.root))
    }

    /// Check a single directory for a package and collect the one it holds.
    fn scan_dir(&mut self, dir: &Path) {
        if !self.contains(dir) {
            return;
        }

        let Some(pkg_type) = package_type::detect_package_type(dir) else {
            return;
        };

        // Every type but Plugin names itself in the frontmatter of its
        // manifest; a plugin names itself in the JSON of its.
        let name = match pkg_type {
            PackageType::Plugin => extract_name_from_plugin_json(dir),
            manifest_type => {
                frontmatter::file_field(&dir.join(manifest_type.manifest_file()), "name")
            }
        };

        let Some(name) = name else {
            return;
        };

        if self.seen.insert(name.clone()) {
            self.packages.push(DiscoveredPackage {
                name,
                package_type: pkg_type,
                path: dir.to_path_buf(),
            });
        }
    }

    /// Check each immediate subdirectory of `dir` for a package.
    ///
    /// A `dir` that is missing, that is not a directory, or that resolves
    /// outside the repository holds no subdirectory to check, so it adds
    /// nothing.
    fn scan_child_dirs(&mut self, dir: &Path) {
        if !self.contains(dir) {
            return;
        }

        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.scan_dir(&path);
            }
        }
    }

    /// Walk `dir` and the directories under it, to [`MAX_SCAN_DEPTH`].
    fn scan_recursive(&mut self, dir: &Path, depth: usize) {
        if depth > MAX_SCAN_DEPTH || !self.contains(dir) {
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

        self.scan_dir(dir);

        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.scan_recursive(&path, depth + 1);
            }
        }
    }

    /// Whether the scan has collected no package.
    fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// The packages the scan collected, in discovery order.
    fn into_packages(self) -> Vec<DiscoveredPackage> {
        self.packages
    }
}

/// Extract name from .claude-plugin/plugin.json.
///
/// Accepts JSONC (comments and trailing commas) because plugin authors edit
/// this file by hand and may carry JSONC conventions over from agent settings.
fn extract_name_from_plugin_json(dir: &Path) -> Option<String> {
    let path = dir.join(".claude-plugin").join("plugin.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = crate::parse_jsonc(&content).ok()?;
    json.get("name")
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
            "package '{}' not found in repository",
            name
        )));
    }

    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::frontmatter::fixtures::{
        write_skill_md, NO_CLOSING_DELIMITER, OPENING_LINE_OF_FOUR_HYPHENS,
        OPENING_LINE_WITH_TRAILING_TEXT, THREE_HYPHEN_RUN_IN_DESCRIPTION,
    };

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

    #[test]
    fn test_classify_git_error_messages_open_lowercase() {
        let cases = [
            (
                git2::ErrorCode::Auth,
                git2::ErrorClass::Net,
                "authentication required",
            ),
            (
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Net,
                "repository not found",
            ),
            (
                git2::ErrorCode::GenericError,
                git2::ErrorClass::Net,
                "failed to resolve host",
            ),
            (
                git2::ErrorCode::GenericError,
                git2::ErrorClass::None,
                "something else went wrong",
            ),
        ];

        for (code, class, git_message) in cases {
            let classified = classify_git_error(
                git2::Error::new(code, class, git_message),
                "https://example.com/repo.git",
            );
            let message = match &classified {
                RegistryError::Unauthorized(message)
                | RegistryError::NotFound(message)
                | RegistryError::Validation(message) => message.clone(),
                other => panic!("unexpected error variant: {other}"),
            };
            let opening = message
                .chars()
                .next()
                .expect("a classified message is never empty");
            assert!(
                !opening.is_uppercase(),
                "a git error message must open lowercase, got {message:?}"
            );
        }
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

    // The frontmatter reader and its delimiter rule are stated once, in
    // `crate::frontmatter`. Discovery is pinned against the same four fixtures
    // anyway, because it is the one reader whose input this repository does
    // not control: a third-party repository writes the package files a scan
    // walks, so the delimiter rule must hold on the discovery path itself and
    // not only on the reader it delegates to.

    #[test]
    fn test_scan_dir_keeps_every_key_past_a_three_hyphen_run() {
        let dir = tempfile::tempdir().unwrap();
        write_skill_md(dir.path(), THREE_HYPHEN_RUN_IN_DESCRIPTION);

        let mut scan = RepoScan::open(dir.path()).unwrap();
        scan.scan_dir(dir.path());
        let packages = scan.into_packages();

        assert_eq!(packages.len(), 1);
        assert_eq!(
            packages[0].name, "test-skill",
            "a three-hyphen run inside the description must not cut the frontmatter short"
        );
    }

    #[test]
    fn test_scan_dir_skips_a_skill_whose_opening_line_carries_trailing_text() {
        let dir = tempfile::tempdir().unwrap();
        write_skill_md(dir.path(), OPENING_LINE_WITH_TRAILING_TEXT);

        let mut scan = RepoScan::open(dir.path()).unwrap();
        scan.scan_dir(dir.path());

        assert!(scan.is_empty());
    }

    #[test]
    fn test_scan_dir_skips_a_skill_whose_opening_line_is_four_hyphens() {
        let dir = tempfile::tempdir().unwrap();
        write_skill_md(dir.path(), OPENING_LINE_OF_FOUR_HYPHENS);

        let mut scan = RepoScan::open(dir.path()).unwrap();
        scan.scan_dir(dir.path());

        assert!(scan.is_empty());
    }

    #[test]
    fn test_scan_dir_skips_a_skill_with_no_closing_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        write_skill_md(dir.path(), NO_CLOSING_DELIMITER);

        let mut scan = RepoScan::open(dir.path()).unwrap();
        scan.scan_dir(dir.path());

        assert!(scan.is_empty());
    }

    #[test]
    fn test_scan_dir_for_package_names_a_skill_from_its_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: my-skill\nmetadata:\n  version: \"1.0.0\"\n---\n# Skill\n",
        )
        .unwrap();

        let mut scan = RepoScan::open(dir.path()).unwrap();
        scan.scan_dir(dir.path());
        let packages = scan.into_packages();

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-skill");
        assert_eq!(packages[0].package_type, PackageType::Skill);
    }

    #[test]
    fn test_scan_dir_for_package_skips_a_skill_whose_frontmatter_names_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("SKILL.md"),
            "---\nmetadata:\n  version: \"1.0.0\"\n---\n# Skill\n",
        )
        .unwrap();

        let mut scan = RepoScan::open(dir.path()).unwrap();
        scan.scan_dir(dir.path());

        assert!(scan.is_empty());
    }

    #[test]
    fn test_scan_dir_for_package_skips_a_skill_carrying_no_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("SKILL.md"), "# Just markdown").unwrap();

        let mut scan = RepoScan::open(dir.path()).unwrap();
        scan.scan_dir(dir.path());

        assert!(scan.is_empty());
    }

    // --- containment tests ---

    /// A repository directory beside a package that sits outside it.
    struct RepoAndOutsidePackage {
        /// The repository a scan is rooted at.
        repo: PathBuf,
        /// A package directory that sits outside the repository.
        outside: PathBuf,
    }

    /// Create a repository directory and, beside it, a package outside it.
    ///
    /// Both live under `root`, so a subpath of `../outside/pkg` reaches the
    /// package from the repository.
    fn repo_and_outside_package(root: &Path) -> RepoAndOutsidePackage {
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();

        let outside = root.join("outside").join("pkg");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(
            outside.join("SKILL.md"),
            "---\nname: outside-skill\n---\n# Outside\n",
        )
        .unwrap();

        RepoAndOutsidePackage { repo, outside }
    }

    #[test]
    fn test_discover_refuses_a_subpath_that_climbs_out_of_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let tree = repo_and_outside_package(dir.path());

        let result = discover_packages(&tree.repo, Some("../outside/pkg"), None);

        assert!(
            result.is_err(),
            "a subpath that climbs out of the repository must be refused, got {:?}",
            result
        );
    }

    #[test]
    fn test_discover_refuses_an_absolute_subpath() {
        let dir = tempfile::tempdir().unwrap();
        let tree = repo_and_outside_package(dir.path());

        let result = discover_packages(&tree.repo, tree.outside.to_str(), None);

        assert!(
            result.is_err(),
            "an absolute subpath must be refused, got {:?}",
            result
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_discover_refuses_a_subpath_that_links_out_of_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let tree = repo_and_outside_package(dir.path());
        std::os::unix::fs::symlink(&tree.outside, tree.repo.join("link")).unwrap();

        let result = discover_packages(&tree.repo, Some("link"), None);

        assert!(
            result.is_err(),
            "a subpath that resolves outside the repository must be refused, got {:?}",
            result
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_discover_skips_a_priority_directory_that_links_out_of_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let tree = repo_and_outside_package(dir.path());
        let outside_store = tree.outside.parent().unwrap().to_path_buf();
        std::os::unix::fs::symlink(&outside_store, tree.repo.join("skills")).unwrap();

        let result = discover_packages(&tree.repo, None, None);

        assert!(
            result.is_err(),
            "a priority directory that resolves outside the repository holds no \
             package of this repository, got {:?}",
            result
        );
    }

    #[test]
    fn test_install_source_clones() {
        let source = InstallSource::GitRepo(parse_git_source("owner/repo", None).unwrap());

        assert_eq!(source.clone(), source);
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
            let name =
                frontmatter::file_field(&pkg.path.join(PackageType::Skill.manifest_file()), "name");
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
        // Use a local bare repo to avoid network dependency
        let bare = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(bare.path()).unwrap();

        let source = GitSource {
            clone_url: format!("file://{}", bare.path().display()),
            git_ref: None,
            subpath: None,
            select: None,
            display_name: "local/test".to_string(),
        };
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
    fn test_clone_anthropics_plugins_discovers_multiple_plugins() {
        // anthropics/claude-plugins-official is a marketplace repo.
        // The plugins/ directory is a PRIORITY_DIR, so the scanner finds
        // all plugins inside it. external_plugins/ is not a priority dir
        // and the recursive scan is skipped once plugins/ yields results.
        let source = parse_git_source("anthropics/claude-plugins-official", None).unwrap();
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
        let source = parse_git_source("anthropics/claude-plugins-official", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();

        // Select "example-plugin" (lives in plugins/, a PRIORITY_DIR)
        let filtered = discover_packages(temp_dir.path(), None, Some("example-plugin")).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "example-plugin");
        assert_eq!(filtered[0].package_type, PackageType::Plugin);
    }

    #[test]
    fn test_clone_anthropics_plugins_select_nonexistent() {
        let source = parse_git_source("anthropics/claude-plugins-official", None).unwrap();
        let temp_dir = git_clone(&source).unwrap();

        let result = discover_packages(temp_dir.path(), None, Some("zzz-not-a-plugin"));
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
        std::fs::create_dir(dir.path().join("tools")).unwrap();
        std::fs::create_dir(&tool_dir).unwrap();
        std::fs::write(
            tool_dir.join("TOOL.md"),
            "---\nname: my-tool\nmcp:\n  command: echo\n---\n# Tool\n",
        )
        .unwrap();

        // plugins/ has a plugin
        let plugin_dir = dir.path().join("plugins").join("my-plugin");
        std::fs::create_dir(dir.path().join("plugins")).unwrap();
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
