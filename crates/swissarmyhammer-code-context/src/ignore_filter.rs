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
use std::sync::{Arc, PoisonError, RwLock};

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
/// filter loads the ignore files one time, then answers for each path
/// ([`SharedIgnoreFilter`] loads a new filter when an ignore file changes). It
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

    /// Give `path` as an absolute path under the canonical root, or `None`
    /// when it is not under the root.
    ///
    /// This joins the canonical root to [`relative`](Self::relative). When the
    /// root is already canonical, or the root cannot be made canonical, the
    /// root as the caller gave it is used. A watcher path in the form of the
    /// root and a watcher path in the canonical form thus get one key. A
    /// caller that keys documents by the canonical path (for example the URI
    /// of a language server document) then gets one key for each file.
    pub fn to_canonical_form(&self, path: &Path) -> Option<PathBuf> {
        let root = self.canonical_root.as_deref().unwrap_or(&self.root);
        self.relative(path).map(|rel| root.join(rel))
    }

    /// Tell if `path` is a file that holds ignore rules for this workspace.
    ///
    /// The ignore files are each `.gitignore` and each `.ignore` file under
    /// the root, and `.git/info/exclude`. A path outside the root is not an
    /// ignore file of this workspace. Each watcher calls this on the raw
    /// event paths, before the filter drops them: `.git/` is a hard skip
    /// directory, and the filter skips each hidden file.
    pub fn is_ignore_file(&self, path: &Path) -> bool {
        let Some(rel) = self.relative(path) else {
            return false;
        };
        rel == Path::new(GIT_EXCLUDE_PATH)
            || rel
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| IGNORE_FILE_NAMES.contains(&name))
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

/// The one ignore rule set of a workspace, shared by each consumer.
///
/// The code-context watcher, the diagnostics watcher and the [`PathFilter`] of
/// the LSP sessions hold clones of one `SharedIgnoreFilter`. When a watcher
/// sees a change to an ignore file, [`reload_if_changed`](Self::reload_if_changed)
/// loads the rules again, and each clone then uses the new rules. Thus the
/// rules change in one place, and no consumer keeps the old rules until sah
/// starts again.
#[derive(Debug, Clone)]
pub struct SharedIgnoreFilter {
    /// The current rules. A reload puts a new filter here, so a caller that
    /// holds the old [`Arc`] keeps a consistent rule set until it is done.
    current: Arc<RwLock<Arc<IgnoreFilter>>>,
}

/// The watchers and the LSP sessions use one `SharedIgnoreFilter` from many
/// threads.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SharedIgnoreFilter>();
};

impl SharedIgnoreFilter {
    /// Share `filter` as the current rules.
    pub fn new(filter: IgnoreFilter) -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::new(filter))),
        }
    }

    /// Load the rules for `workspace_root` on the blocking pool, and share
    /// them.
    ///
    /// # Errors
    ///
    /// Gives the [`tokio::task::JoinError`] of [`IgnoreFilter::load`].
    pub async fn load(workspace_root: &Path) -> Result<Self, tokio::task::JoinError> {
        IgnoreFilter::load(workspace_root).await.map(Self::new)
    }

    /// Give the current rules.
    pub fn current(&self) -> Arc<IgnoreFilter> {
        Arc::clone(&self.current.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// Load the rules again when one of `paths` is an ignore file.
    ///
    /// `paths` are the raw paths of one watcher event batch. The check
    /// ([`IgnoreFilter::is_ignore_file`]) runs before the watcher filters the
    /// batch, because the filter drops each ignore file. A batch with no
    /// ignore file reads no file and keeps the current rules. Returns `true`
    /// when the rules were loaded again. A load that fails is logged, and the
    /// current rules stay.
    pub async fn reload_if_changed<'a, I>(&self, paths: I) -> bool
    where
        I: IntoIterator<Item = &'a Path>,
    {
        let current = self.current();
        if !paths.into_iter().any(|path| current.is_ignore_file(path)) {
            return false;
        }
        match IgnoreFilter::load(&current.root).await {
            Ok(filter) => {
                *self.current.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(filter);
                tracing::info!(
                    "ignore filter: an ignore file changed, loaded the rules again for {}",
                    current.root.display()
                );
                true
            }
            Err(e) => {
                tracing::warn!("ignore filter: cannot load the rules again: {}", e);
                false
            }
        }
    }

    /// Make these rules the [`PathFilter`] of the LSP sessions.
    ///
    /// The LSP crate cannot depend on this crate, so the owner gives it this
    /// predicate. The predicate skips each path that
    /// [`IgnoreFilter::is_ignored`] skips: an ignored path, and a path outside
    /// the workspace root. It reads the current rules on each call, so it
    /// obeys a reload. Thus the walk, the watchers and the language server
    /// obey one rule set.
    pub fn path_filter(&self) -> PathFilter {
        let shared = self.clone();
        Arc::new(move |path: &Path| shared.current().is_ignored(path))
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
    fn to_canonical_form_gives_each_path_the_form_of_the_canonical_root() {
        let outer = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let (root, canonical) = symlinked_root(outer.path());
        let filter = IgnoreFilter::new(&root);
        let expected = Some(canonical.join("src/a.rs"));
        assert_eq!(filter.to_canonical_form(&root.join("src/a.rs")), expected);
        assert_eq!(
            filter.to_canonical_form(&canonical.join("src/a.rs")),
            expected
        );
        assert_eq!(filter.to_canonical_form(Path::new("src/a.rs")), expected);
        assert_eq!(filter.to_canonical_form(&other.path().join("a.rs")), None);
    }

    #[test]
    fn relative_strips_a_root_that_is_already_canonical() {
        let outer = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let (_root, canonical) = symlinked_root(outer.path());
        let filter = IgnoreFilter::new(&canonical);
        assert_eq!(
            filter.relative(&canonical.join("src/a.rs")),
            Some(Path::new("src/a.rs"))
        );
        assert_eq!(filter.relative(&other.path().join("a.rs")), None);
    }

    #[test]
    fn to_canonical_form_keeps_a_root_that_is_already_canonical() {
        let outer = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let (_root, canonical) = symlinked_root(outer.path());
        let filter = IgnoreFilter::new(&canonical);
        assert_eq!(
            filter.to_canonical_form(&canonical.join("src/a.rs")),
            Some(canonical.join("src/a.rs"))
        );
        assert_eq!(filter.to_canonical_form(&other.path().join("a.rs")), None);
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
        let filter = SharedIgnoreFilter::new(IgnoreFilter::new(dir.path())).path_filter();

        assert!(filter(
            &dir.path().join(".build/checkouts/pkg/Package.swift")
        ));
        assert!(filter(&other.path().join("a.rs")));
        assert!(!filter(&normal));
    }

    #[test]
    fn is_ignore_file_names_each_ignore_file_under_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let filter = IgnoreFilter::new(dir.path());
        for rel in [
            ".gitignore",
            "pkg/.gitignore",
            "pkg/.ignore",
            ".git/info/exclude",
        ] {
            assert!(filter.is_ignore_file(&dir.path().join(rel)), "{rel}");
        }
        assert!(filter.is_ignore_file(Path::new("pkg/.ignore")));
        for rel in ["src/a.rs", "pkg/info/exclude", ".git/config"] {
            assert!(!filter.is_ignore_file(&dir.path().join(rel)), "{rel}");
        }
        assert!(!filter.is_ignore_file(&other.path().join(".gitignore")));
    }

    #[test]
    fn is_ignore_file_names_an_ignore_file_under_the_canonical_root() {
        let outer = tempfile::tempdir().unwrap();
        let (root, canonical) = symlinked_root(outer.path());
        let filter = IgnoreFilter::new(&root);
        assert!(filter.is_ignore_file(&canonical.join(".git/info/exclude")));
    }

    #[tokio::test]
    async fn reload_if_changed_obeys_a_new_gitignore_rule() {
        let dir = tempfile::tempdir().unwrap();
        let shared = SharedIgnoreFilter::load(dir.path()).await.unwrap();
        let generated = dir.path().join("gen/a.rs");
        assert!(!shared.current().is_ignored(&generated));

        let gitignore = dir.path().join(".gitignore");
        fs::write(&gitignore, "gen/\n").unwrap();

        assert!(shared.reload_if_changed([gitignore.as_path()]).await);
        assert!(shared.current().is_ignored(&generated));
    }

    #[tokio::test]
    async fn reload_if_changed_obeys_a_new_git_exclude_rule() {
        let dir = tempfile::tempdir().unwrap();
        let shared = SharedIgnoreFilter::load(dir.path()).await.unwrap();
        let exclude = dir.path().join(".git/info/exclude");
        fs::create_dir_all(exclude.parent().unwrap()).unwrap();
        fs::write(&exclude, "scratch/\n").unwrap();

        assert!(shared.reload_if_changed([exclude.as_path()]).await);
        assert!(shared
            .current()
            .is_ignored(&dir.path().join("scratch/a.rs")));
    }

    #[tokio::test]
    async fn reload_if_changed_keeps_the_rules_for_a_batch_with_no_ignore_file() {
        let dir = tempfile::tempdir().unwrap();
        let shared = SharedIgnoreFilter::load(dir.path()).await.unwrap();
        let before = shared.current();
        let source = dir.path().join("src/a.rs");

        assert!(!shared.reload_if_changed([source.as_path()]).await);
        assert!(Arc::ptr_eq(&before, &shared.current()));
    }

    #[tokio::test]
    async fn path_filter_obeys_the_reloaded_rules() {
        let dir = tempfile::tempdir().unwrap();
        let shared = SharedIgnoreFilter::load(dir.path()).await.unwrap();
        let path_filter = shared.path_filter();
        let generated = dir.path().join("gen/a.rs");
        assert!(!path_filter(&generated));

        let gitignore = dir.path().join(".gitignore");
        fs::write(&gitignore, "gen/\n").unwrap();
        shared.reload_if_changed([gitignore.as_path()]).await;

        assert!(path_filter(&generated));
    }

    #[test]
    fn clones_share_one_rule_set() {
        let dir = tempfile::tempdir().unwrap();
        let shared = SharedIgnoreFilter::new(IgnoreFilter::new(dir.path()));
        let clone = shared.clone();
        assert!(Arc::ptr_eq(&shared.current(), &clone.current()));
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
