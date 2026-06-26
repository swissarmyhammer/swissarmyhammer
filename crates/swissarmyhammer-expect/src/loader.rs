//! Discovering `*.expect.md` specs and resolving a `<scope>` argument to a set of
//! them.
//!
//! This is the loader that backs every op taking a `<scope>` (`doctor`,
//! `observe`, `evaluate`, `check`, `approve`) from `ideas/expect.md` §"Scope
//! resolution". An [`ExpectationLoader`] is bound to a repo root and offers two
//! entry points:
//!
//! - [`load_all`](ExpectationLoader::load_all) discovers **every** `*.expect.md`
//!   under the repo — colocated specs anywhere in the tree plus the repo-global
//!   specs under `.expect/expectations/` — skipping the usual build/dependency
//!   directories. This is the default CI invocation (a bare `check`).
//! - [`resolve_scope`](ExpectationLoader::resolve_scope) narrows that set with the
//!   three scope forms (a specific spec, a folder, or a glob) and an optional
//!   `--tag` filter.
//!
//! Identity is the repo-relative path with `.expect.md` stripped (the same
//! identity [`Expectation::parse`](crate::Expectation) derives), and results are
//! deduplicated by it so a spec selected two ways appears once.

use crate::error::ExpectError;
use crate::spec::{Expectation, EXPECT_EXTENSION};
use std::path::{Path, PathBuf};
use swissarmyhammer_project_detection::should_skip_directory;
use walkdir::{DirEntry, WalkDir};

/// Discovers and resolves `*.expect.md` expectation specs within one repo.
///
/// Bound to a repo root at construction; the root is both the discovery boundary
/// and the base that relative scope arguments resolve against.
///
/// # Examples
///
/// Discover every spec in a repo — colocated and repo-global alike:
///
/// ```
/// use swissarmyhammer_expect::ExpectationLoader;
/// use std::fs;
///
/// let repo = tempfile::tempdir()?;
/// let checkout = repo.path().join("src/checkout");
/// fs::create_dir_all(&checkout)?;
/// fs::write(
///     checkout.join("coupon.expect.md"),
///     "---\ndescription: a coupon reduces the total\nsurface: cli\n---\n\n- [ ] holds\n",
/// )?;
///
/// let loader = ExpectationLoader::new(repo.path());
/// let specs = loader.load_all()?;
/// assert_eq!(specs.len(), 1);
/// assert_eq!(specs[0].path, "src/checkout/coupon");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct ExpectationLoader {
    /// The repo root: the discovery boundary and the base for relative scopes.
    repo_root: PathBuf,
}

impl ExpectationLoader {
    /// Create a loader bound to `repo_root`.
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    /// Discover and parse every `*.expect.md` spec under the repo root.
    ///
    /// Walks the whole tree, skipping the usual build/dependency directories
    /// ([`should_skip_directory`]), so colocated specs anywhere plus the
    /// repo-global specs under `.expect/expectations/` are all found. Results are
    /// sorted and deduplicated by repo-relative identity.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] if a discovered spec cannot be read or parsed.
    pub fn load_all(&self) -> Result<Vec<Expectation>, ExpectError> {
        self.parse_files(discover_expect_files(&self.repo_root))
    }

    /// Resolve a `<scope>` (and optional `--tag`) to the set of matching specs.
    ///
    /// The scope string is matched against the three forms from `ideas/expect.md`
    /// §"Scope resolution", in order: a specific expectation (a `*.expect.md`
    /// file, or its repo-relative path with the extension dropped), a folder
    /// (every spec under it recursively), then a shell-style glob. With no scope
    /// (`None` or an empty string) the base set is every spec in the repo. An
    /// optional `tag` then narrows the base set to specs carrying that tag, so a
    /// bare `tag` selects every tagged spec.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] if a selected spec cannot be read or parsed, or if
    /// `scope` is a malformed glob pattern.
    pub fn resolve_scope(
        &self,
        scope: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<Expectation>, ExpectError> {
        let mut specs = match scope.filter(|s| !s.is_empty()) {
            Some(scope) => self.resolve_scope_str(scope)?,
            None => self.load_all()?,
        };
        if let Some(tag) = tag {
            specs.retain(|spec| spec.frontmatter.tags.iter().any(|t| t == tag));
        }
        Ok(specs)
    }

    /// Resolve a non-empty scope string through the three ordered forms.
    fn resolve_scope_str(&self, scope: &str) -> Result<Vec<Expectation>, ExpectError> {
        // Form 1: a specific expectation, by file path or extension-dropped path.
        if let Some(file) = self.specific_spec_file(scope) {
            return Ok(vec![self.parse_file(&file)?]);
        }
        // Form 2: a folder — every spec under it, recursively.
        let candidate = self.resolve_relative(scope);
        if candidate.is_dir() {
            return self.parse_files(discover_expect_files(&candidate));
        }
        // Form 3: a shell-style glob.
        self.parse_files(self.glob_files(scope)?)
    }

    /// Locate the single spec file a "specific expectation" scope addresses, by
    /// `*.expect.md` file path or by its extension-dropped repo-relative path.
    fn specific_spec_file(&self, scope: &str) -> Option<PathBuf> {
        let direct = self.resolve_relative(scope);
        if direct.is_file() && is_expect_file(&direct) {
            return Some(direct);
        }
        let dropped = self.resolve_relative(&format!("{scope}{EXPECT_EXTENSION}"));
        dropped.is_file().then_some(dropped)
    }

    /// Files matching a shell-style glob, resolved relative to the repo root.
    ///
    /// Only existing `*.expect.md` files are kept; directories and unreadable
    /// paths the glob walk surfaces are skipped.
    fn glob_files(&self, scope: &str) -> Result<Vec<PathBuf>, ExpectError> {
        let pattern = self.resolve_relative(scope);
        let pattern = pattern.to_string_lossy();
        let paths = glob::glob(&pattern).map_err(|e| ExpectError::Expectation {
            path: scope.to_string(),
            message: format!("invalid glob pattern: {e}"),
        })?;
        Ok(paths
            .filter_map(Result::ok)
            .filter(|path| path.is_file() && is_expect_file(path))
            .collect())
    }

    /// Resolve `scope` against the repo root unless it is already absolute.
    fn resolve_relative(&self, scope: &str) -> PathBuf {
        let path = Path::new(scope);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.repo_root.join(path)
        }
    }

    /// Read and parse each file, then sort and deduplicate by repo-relative
    /// identity so a spec reachable two ways appears once.
    fn parse_files(&self, files: Vec<PathBuf>) -> Result<Vec<Expectation>, ExpectError> {
        let mut specs = Vec::with_capacity(files.len());
        for file in files {
            specs.push(self.parse_file(&file)?);
        }
        specs.sort_by(|a, b| a.path.cmp(&b.path));
        specs.dedup_by(|a, b| a.path == b.path);
        Ok(specs)
    }

    /// Read and parse one spec file, deriving its identity from the repo root.
    fn parse_file(&self, file: &Path) -> Result<Expectation, ExpectError> {
        let content = std::fs::read_to_string(file)?;
        Expectation::parse(&content, file, &self.repo_root)
    }
}

/// Discover every `*.expect.md` file under `root`, skipping the usual
/// build/dependency directories ([`should_skip_directory`]).
fn discover_expect_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !is_skipped_dir(entry))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(DirEntry::into_path)
        .filter(|path| is_expect_file(path))
        .collect()
}

/// Whether `path` names an expectation spec (its file name ends in `.expect.md`).
fn is_expect_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.ends_with(EXPECT_EXTENSION))
        .unwrap_or(false)
}

/// Whether `entry` is a directory that traversal should not descend into.
fn is_skipped_dir(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(should_skip_directory)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use swissarmyhammer_project_detection::SKIP_DIRECTORIES;
    use tempfile::TempDir;

    /// Minimal spec body with the given description and tags, valid per
    /// [`Expectation::parse`](crate::Expectation).
    fn spec_contents(description: &str, tags: &[&str]) -> String {
        let tags_line = if tags.is_empty() {
            String::new()
        } else {
            format!("tags: [{}]\n", tags.join(", "))
        };
        format!("---\ndescription: {description}\nsurface: cli\n{tags_line}---\n\n- [ ] holds\n")
    }

    /// Write a `*.expect.md` spec at `repo_root/<rel>.expect.md`, creating parent
    /// dirs. `rel` is the repo-relative identity (no extension).
    fn write_spec(repo_root: &Path, rel: &str, tags: &[&str]) {
        let file = repo_root.join(format!("{rel}{EXPECT_EXTENSION}"));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, spec_contents(rel, tags)).unwrap();
    }

    /// A repo fixture with the three specs from the acceptance criteria.
    fn fixture() -> TempDir {
        let repo = TempDir::new().unwrap();
        write_spec(repo.path(), "src/a", &[]);
        write_spec(repo.path(), "src/checkout/coupon", &["checkout", "pricing"]);
        write_spec(repo.path(), ".expect/expectations/global", &["pricing"]);
        repo
    }

    /// The repo-relative identities of a result set, sorted.
    fn identities(specs: &[Expectation]) -> Vec<String> {
        let mut ids: Vec<String> = specs.iter().map(|s| s.path.clone()).collect();
        ids.sort();
        ids
    }

    #[test]
    fn load_all_discovers_colocated_and_repo_global_specs() {
        let repo = fixture();
        let loader = ExpectationLoader::new(repo.path());
        let specs = loader.load_all().unwrap();
        assert_eq!(
            identities(&specs),
            vec![
                ".expect/expectations/global",
                "src/a",
                "src/checkout/coupon",
            ]
        );
    }

    #[test]
    fn load_all_skips_build_and_dependency_directories() {
        let repo = fixture();
        // A spec buried in any directory of the shared skip set must not be
        // discovered. Driving the names from SKIP_DIRECTORIES keeps the test
        // tracking the real source of truth rather than re-typed literals.
        for skip in SKIP_DIRECTORIES {
            write_spec(repo.path(), &format!("{skip}/nested/ghost"), &[]);
        }

        let loader = ExpectationLoader::new(repo.path());
        let specs = loader.load_all().unwrap();
        assert_eq!(
            identities(&specs),
            vec![
                ".expect/expectations/global",
                "src/a",
                "src/checkout/coupon",
            ]
        );
    }

    #[test]
    fn resolve_scope_specific_by_extension_dropped_path() {
        let repo = fixture();
        let loader = ExpectationLoader::new(repo.path());
        let specs = loader
            .resolve_scope(Some("src/checkout/coupon"), None)
            .unwrap();
        assert_eq!(identities(&specs), vec!["src/checkout/coupon"]);
    }

    #[test]
    fn resolve_scope_specific_by_full_file_path() {
        let repo = fixture();
        let loader = ExpectationLoader::new(repo.path());
        let specs = loader
            .resolve_scope(Some("src/checkout/coupon.expect.md"), None)
            .unwrap();
        assert_eq!(identities(&specs), vec!["src/checkout/coupon"]);
    }

    #[test]
    fn resolve_scope_folder_selects_everything_under_it_recursively() {
        let repo = fixture();
        // A deeper colocated spec under the folder must be included.
        write_spec(repo.path(), "src/checkout/refund/partial", &[]);

        let loader = ExpectationLoader::new(repo.path());
        let specs = loader.resolve_scope(Some("src/checkout/"), None).unwrap();
        assert_eq!(
            identities(&specs),
            vec!["src/checkout/coupon", "src/checkout/refund/partial"]
        );
    }

    #[test]
    fn resolve_scope_glob_matches_shell_style_pattern() {
        let repo = fixture();
        let loader = ExpectationLoader::new(repo.path());
        let specs = loader
            .resolve_scope(Some("src/**/*coupon*.expect.md"), None)
            .unwrap();
        assert_eq!(identities(&specs), vec!["src/checkout/coupon"]);
    }

    #[test]
    fn resolve_scope_by_tag_selects_specs_carrying_the_tag() {
        let repo = fixture();
        let loader = ExpectationLoader::new(repo.path());
        let specs = loader.resolve_scope(None, Some("pricing")).unwrap();
        assert_eq!(
            identities(&specs),
            vec![".expect/expectations/global", "src/checkout/coupon"]
        );
    }

    #[test]
    fn resolve_scope_with_no_scope_returns_all_specs() {
        let repo = fixture();
        let loader = ExpectationLoader::new(repo.path());
        let specs = loader.resolve_scope(None, None).unwrap();
        assert_eq!(
            identities(&specs),
            vec![
                ".expect/expectations/global",
                "src/a",
                "src/checkout/coupon",
            ]
        );
    }

    #[test]
    fn resolve_scope_empty_string_is_treated_as_no_scope() {
        let repo = fixture();
        let loader = ExpectationLoader::new(repo.path());
        let specs = loader.resolve_scope(Some(""), None).unwrap();
        assert_eq!(specs.len(), 3);
    }

    #[test]
    fn parse_files_dedupes_repeated_paths_by_identity() {
        let repo = fixture();
        let loader = ExpectationLoader::new(repo.path());
        let file = repo
            .path()
            .join(format!("src/checkout/coupon{EXPECT_EXTENSION}"));
        // The same file reached twice (e.g. selected by two overlapping scopes)
        // collapses to a single entry, keyed by its repo-relative identity.
        let specs = loader.parse_files(vec![file.clone(), file]).unwrap();
        assert_eq!(identities(&specs), vec!["src/checkout/coupon"]);
    }

    #[test]
    fn resolve_scope_folder_then_tag_narrows_to_the_tagged_specs() {
        let repo = fixture();
        // An untagged sibling under the same folder: the folder form selects both,
        // and the tag filter then narrows the result to just the tagged coupon.
        write_spec(repo.path(), "src/checkout/refund", &[]);
        let loader = ExpectationLoader::new(repo.path());
        let specs = loader
            .resolve_scope(Some("src/checkout/"), Some("checkout"))
            .unwrap();
        assert_eq!(identities(&specs), vec!["src/checkout/coupon"]);
    }
}
