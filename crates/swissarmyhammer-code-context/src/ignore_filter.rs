//! One ignore filter for the workspace walk and the file watchers.
//!
//! The startup walk, the periodic reconcile walk, the code-context watcher,
//! and the diagnostics watcher must agree on which paths they skip. If the
//! watchers skip less than the walk, build output (for example
//! `.build/checkouts/`) goes into the index and into the language server.
//!
//! This module holds the shared rules:
//!
//! - Hidden files and directories are skipped.
//! - `.gitignore` and `.ignore` files in the workspace, and
//!   `.git/info/exclude`, are obeyed.
//! - The directories in [`HARD_SKIP_DIRS`] are always skipped, also when no
//!   ignore file lists them.

use std::cmp::Reverse;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{Match, WalkBuilder};
use swissarmyhammer_lsp::PathFilter;

/// Directory names that are always skipped, also with no ignore file.
pub const HARD_SKIP_DIRS: &[&str] = &[
    ".build",
    "target",
    "node_modules",
    ".git",
    ".code-context",
    ".sah",
];

/// Ignore file names that each directory can hold, lowest precedence first.
///
/// These are the per-directory files that [`WalkBuilder`] reads with
/// `git_ignore(true)` and `ignore(true)`.
const IGNORE_FILE_NAMES: &[&str] = &[".gitignore", ".ignore"];

/// The git exclude file, relative to the workspace root.
const GIT_EXCLUDE_PATH: &str = ".git/info/exclude";

/// Tell if one path component is a name in [`HARD_SKIP_DIRS`].
pub fn is_hard_skip_name(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| HARD_SKIP_DIRS.contains(&name))
}

/// Tell if one path component is hidden (its name starts with a dot).
fn is_hidden_name(name: &OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

/// Make the [`WalkBuilder`] that every workspace walk uses.
///
/// The builder obeys the same rules as [`IgnoreFilter`]: it skips hidden
/// entries, obeys `.gitignore`, `.ignore` and `.git/info/exclude`, and prunes
/// each directory in [`HARD_SKIP_DIRS`].
///
/// `require_git(false)` makes the walk obey `.gitignore` also when the root
/// has no `.git` directory. `parents(false)` makes the walk read no ignore file
/// above the root. [`IgnoreFilter`] reads the same set of files, so the walk
/// and the watchers always agree.
pub fn workspace_walk_builder(workspace_root: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(workspace_root);
    builder
        .hidden(true)
        .ignore(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(false)
        .require_git(false)
        .parents(false)
        .filter_entry(|entry| !is_hard_skip_name(entry.file_name()));
    builder
}

/// A path filter that obeys the same rules as [`workspace_walk_builder`].
///
/// The file watchers see each changed path one at a time, with no walk. This
/// filter loads the ignore files one time, then answers for each path. It
/// tests each ancestor directory of the path with `is_dir = true` and then the
/// path itself, in the same order as the walk descends. Thus a directory rule
/// such as `build-out/` skips each file under that directory.
#[derive(Debug, Clone)]
pub struct IgnoreFilter {
    /// The workspace root as the caller gave it.
    root: PathBuf,
    /// The canonical workspace root, when it is different from `root`.
    ///
    /// Some watcher backends (FSEvents on macOS) send canonical paths, for
    /// example `/private/var/...` for a root of `/var/...`.
    canonical_root: Option<PathBuf>,
    /// One matcher for each directory that holds an ignore file, deepest
    /// directory first.
    matchers: Vec<Gitignore>,
}

impl IgnoreFilter {
    /// Load the ignore rules for `workspace_root`.
    ///
    /// This walks the workspace one time with [`workspace_walk_builder`] to
    /// find the ignore files. The walk does not go into ignored directories, so
    /// an ignore file in an ignored directory has no effect, the same as in
    /// the walk. An ignore file that cannot be read or parsed is logged and
    /// skipped.
    pub fn new(workspace_root: &Path) -> Self {
        let mut matchers = Vec::new();
        for entry in workspace_walk_builder(workspace_root).build() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    tracing::warn!("ignore filter: walk error: {}", e);
                    continue;
                }
            };
            if !entry.file_type().is_some_and(|t| t.is_dir()) {
                continue;
            }
            if let Some(matcher) = load_dir_matcher(workspace_root, entry.path()) {
                matchers.push(matcher);
            }
        }
        matchers.sort_by_key(|m| Reverse(m.path().components().count()));

        let canonical_root = workspace_root
            .canonicalize()
            .ok()
            .filter(|canonical| canonical != workspace_root);
        Self {
            root: workspace_root.to_path_buf(),
            canonical_root,
            matchers,
        }
    }

    /// Load the ignore rules for `workspace_root` on the blocking pool.
    ///
    /// [`IgnoreFilter::new`] walks the workspace, so an async caller must not
    /// run it on a runtime worker thread. This constructor runs it with
    /// [`tokio::task::spawn_blocking`] and gives the result.
    ///
    /// # Errors
    ///
    /// Gives the [`tokio::task::JoinError`] when the blocking task panics or
    /// the runtime cancels it.
    pub async fn load(workspace_root: &Path) -> Result<Self, tokio::task::JoinError> {
        let root = workspace_root.to_path_buf();
        tokio::task::spawn_blocking(move || Self::new(&root)).await
    }

    /// Tell if the watchers must skip `path`.
    ///
    /// `path` is absolute, or relative to the workspace root. A path that is
    /// not under the workspace root is always skipped.
    pub fn is_ignored(&self, path: &Path) -> bool {
        let Some(rel) = self.relative(path) else {
            return true;
        };
        let mut current = self.root.clone();
        let mut components = rel.components().peekable();
        while let Some(component) = components.next() {
            let name = match component {
                Component::Normal(name) => name,
                Component::CurDir => continue,
                _ => return true,
            };
            current.push(name);
            let is_dir = components.peek().is_some() || current.is_dir();
            if is_hidden_name(name)
                || is_hard_skip_name(name)
                || self.gitignore_ignores(&current, is_dir)
            {
                return true;
            }
        }
        false
    }

    /// Make this filter the [`PathFilter`] of the LSP sessions.
    ///
    /// The LSP crate cannot depend on this crate, so the owner gives it this
    /// predicate. The predicate skips each path that [`is_ignored`](Self::is_ignored)
    /// skips: an ignored path, and a path outside the workspace root. Thus the
    /// walk, the watchers and the language server obey one rule set.
    pub fn into_path_filter(self) -> PathFilter {
        Arc::new(move |path: &Path| self.is_ignored(path))
    }

    /// Give `path` relative to the workspace root, or `None` when it is not
    /// under the root.
    ///
    /// An absolute path is stripped of the root as the caller gave it, and
    /// then of the canonical root. A relative path is already relative to the
    /// root, so it is given back with no change. This is the one function that
    /// makes a workspace-relative path from a watcher path. Thus the watchers,
    /// the walk and the language server all get the same key for one file,
    /// also when a watcher backend sends the canonical form.
    pub fn relative<'a>(&self, path: &'a Path) -> Option<&'a Path> {
        if path.is_relative() {
            return Some(path);
        }
        path.strip_prefix(&self.root).ok().or_else(|| {
            self.canonical_root
                .as_deref()
                .and_then(|root| path.strip_prefix(root).ok())
        })
    }

    /// Give `path` as an absolute path under the root as the caller gave it,
    /// or `None` when it is not under the root.
    ///
    /// This joins the root to [`relative`](Self::relative). A watcher path
    /// under the canonical root thus gets the form of the root. A caller that
    /// keys documents by absolute path (for example the URI of a language
    /// server document) then gets one key for each file.
    pub fn to_root_form(&self, path: &Path) -> Option<PathBuf> {
        self.relative(path).map(|rel| self.root.join(rel))
    }

    /// Tell if the ignore files ignore one entry.
    ///
    /// Only the matchers of the ancestor directories of `path` apply. The
    /// deepest matcher with a match decides, the same as in the walk.
    fn gitignore_ignores(&self, path: &Path, is_dir: bool) -> bool {
        let Some(parent) = path.parent() else {
            return false;
        };
        for matcher in &self.matchers {
            if !parent.starts_with(matcher.path()) {
                continue;
            }
            match matcher.matched(path, is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }
}

/// Load the ignore files of one directory into one matcher.
///
/// The root directory also gets `.git/info/exclude`, with the lowest
/// precedence. Returns `None` when the directory has no ignore file, or when
/// the matcher cannot be built.
fn load_dir_matcher(workspace_root: &Path, dir: &Path) -> Option<Gitignore> {
    let mut builder = GitignoreBuilder::new(dir);
    let mut sources = Vec::new();
    if dir == workspace_root {
        sources.push(dir.join(GIT_EXCLUDE_PATH));
    }
    sources.extend(IGNORE_FILE_NAMES.iter().map(|name| dir.join(name)));

    let mut found = false;
    for source in sources.iter().filter(|source| source.is_file()) {
        found = true;
        if let Some(e) = builder.add(source) {
            tracing::warn!("ignore filter: cannot read {}: {}", source.display(), e);
        }
    }
    if !found {
        return None;
    }
    match builder.build() {
        Ok(matcher) => Some(matcher),
        Err(e) => {
            tracing::warn!(
                "ignore filter: cannot build matcher for {}: {}",
                dir.display(),
                e
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::symlinked_root;
    use std::fs;

    /// Make a file and its parent directories under `root`.
    fn touch(root: &Path, rel: &str) -> PathBuf {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "x").unwrap();
        path
    }

    #[test]
    fn normal_source_path_is_not_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = touch(dir.path(), "Sources/a.swift");
        let filter = IgnoreFilter::new(dir.path());
        assert!(!filter.is_ignored(&path));
    }

    #[test]
    fn hard_skip_dir_is_ignored_with_no_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let filter = IgnoreFilter::new(dir.path());
        for rel in [
            ".build/checkouts/X/Sources/a.swift",
            "target/debug/build/a.rs",
            "node_modules/pkg/index.js",
            ".git/hooks/a.sh",
            ".code-context/a.json",
            ".sah/a.yaml",
            "sub/node_modules/pkg/index.js",
        ] {
            assert!(filter.is_ignored(&dir.path().join(rel)), "{rel}");
        }
    }

    #[test]
    fn hidden_path_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = touch(dir.path(), ".hidden/a.rs");
        let filter = IgnoreFilter::new(dir.path());
        assert!(filter.is_ignored(&path));
    }

    #[test]
    fn gitignored_file_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "generated.rs\n").unwrap();
        let ignored = touch(dir.path(), "src/generated.rs");
        let kept = touch(dir.path(), "src/main.rs");
        let filter = IgnoreFilter::new(dir.path());
        assert!(filter.is_ignored(&ignored));
        assert!(!filter.is_ignored(&kept));
    }

    #[test]
    fn gitignored_directory_rule_skips_files_under_it() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "build-out/\n/dist\n").unwrap();
        let filter = IgnoreFilter::new(dir.path());
        assert!(filter.is_ignored(&dir.path().join("build-out/a/b.swift")));
        assert!(filter.is_ignored(&dir.path().join("dist/x/y.js")));
        assert!(!filter.is_ignored(&dir.path().join("src/dist/y.js")));
    }

    #[test]
    fn git_info_exclude_is_obeyed() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".git/info")).unwrap();
        fs::write(dir.path().join(".git/info/exclude"), "scratch/\n").unwrap();
        let filter = IgnoreFilter::new(dir.path());
        assert!(filter.is_ignored(&dir.path().join("scratch/a.rs")));
        assert!(!filter.is_ignored(&dir.path().join("src/a.rs")));
    }

    #[test]
    fn nested_gitignore_applies_under_its_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("pkg")).unwrap();
        fs::write(dir.path().join("pkg/.gitignore"), "out/\n").unwrap();
        let filter = IgnoreFilter::new(dir.path());
        assert!(filter.is_ignored(&dir.path().join("pkg/out/a.rs")));
        assert!(!filter.is_ignored(&dir.path().join("out/a.rs")));
    }

    #[test]
    fn path_outside_the_root_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let filter = IgnoreFilter::new(dir.path());
        assert!(filter.is_ignored(&other.path().join("a.rs")));
    }

    #[test]
    fn path_under_the_canonical_root_is_not_ignored() {
        let outer = tempfile::tempdir().unwrap();
        let (root, canonical) = symlinked_root(outer.path());
        let path = touch(&canonical, "src/a.rs");
        let filter = IgnoreFilter::new(&root);
        assert!(!filter.is_ignored(&path));
    }

    #[test]
    fn hard_skip_path_under_the_canonical_root_is_ignored() {
        let outer = tempfile::tempdir().unwrap();
        let (root, canonical) = symlinked_root(outer.path());
        let path = touch(&canonical, ".build/checkouts/X/Sources/a.swift");
        let filter = IgnoreFilter::new(&root);
        assert!(filter.is_ignored(&path));
    }

    #[test]
    fn relative_strips_the_root_and_the_canonical_root() {
        let outer = tempfile::tempdir().unwrap();
        let (root, canonical) = symlinked_root(outer.path());
        let filter = IgnoreFilter::new(&root);
        let expected = Some(Path::new("src/a.rs"));
        assert_eq!(filter.relative(&root.join("src/a.rs")), expected);
        assert_eq!(filter.relative(&canonical.join("src/a.rs")), expected);
        assert_eq!(filter.relative(Path::new("src/a.rs")), expected);
    }

    #[test]
    fn relative_gives_none_for_a_path_outside_both_roots() {
        let outer = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let (root, _canonical) = symlinked_root(outer.path());
        let filter = IgnoreFilter::new(&root);
        assert_eq!(filter.relative(&other.path().join("a.rs")), None);
    }

    #[test]
    fn to_root_form_gives_a_canonical_path_the_form_of_the_root() {
        let outer = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let (root, canonical) = symlinked_root(outer.path());
        let filter = IgnoreFilter::new(&root);
        assert_eq!(
            filter.to_root_form(&canonical.join("src/a.rs")),
            Some(root.join("src/a.rs"))
        );
        assert_eq!(filter.to_root_form(&other.path().join("a.rs")), None);
    }

    #[test]
    fn relative_path_is_read_from_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let filter = IgnoreFilter::new(dir.path());
        assert!(filter.is_ignored(Path::new(".build/a.swift")));
        assert!(!filter.is_ignored(Path::new("Sources/a.swift")));
    }

    #[test]
    fn path_filter_skips_ignored_and_outside_paths_and_keeps_normal_paths() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let normal = touch(dir.path(), "Sources/main.swift");
        let filter = IgnoreFilter::new(dir.path()).into_path_filter();

        assert!(filter(
            &dir.path().join(".build/checkouts/pkg/Package.swift")
        ));
        assert!(filter(&other.path().join("a.rs")));
        assert!(!filter(&normal));
    }

    #[tokio::test]
    async fn load_obeys_the_ignore_files_of_the_root() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "gen/\n").unwrap();
        let filter = IgnoreFilter::load(dir.path()).await.unwrap();
        assert!(filter.is_ignored(&dir.path().join("gen/a.rs")));
        assert!(!filter.is_ignored(&dir.path().join("src/a.rs")));
    }

    #[test]
    fn walk_skips_hard_skip_dir_with_no_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "target/a.rs");
        touch(dir.path(), "src/b.rs");
        fs::write(dir.path().join(".gitignore"), "gen/\n").unwrap();
        touch(dir.path(), "gen/c.rs");
        let files: Vec<PathBuf> = workspace_walk_builder(dir.path())
            .build()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
            .map(|e| e.path().strip_prefix(dir.path()).unwrap().to_path_buf())
            .collect();
        assert_eq!(files, vec![PathBuf::from("src/b.rs")]);
    }
}
