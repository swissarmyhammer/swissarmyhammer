//! Scope resolution — the deterministic git side of engine stage 1.
//!
//! Resolves a [`Scope`](super::Scope) selector into the changed-file set and
//! every input the later steps need: the sem-diff before/after content, the
//! review-level change purpose, and blame's history anchor. Every path is
//! confined to the repository root ([`confine_to_repo`]) and every scope's
//! resolved file set passes the same `.reviewignore` + `.gitignore` filter, so
//! an escaping or ignored path can never reach the review agent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use swissarmyhammer_git::GitOperations;
use swissarmyhammer_sem::git_types::{FileChange as SemFileChange, FileStatus};
use swissarmyhammer_sem::parser::plugins::code::is_code_file;

use ::ignore::gitignore::Gitignore;

use crate::error::AvpError;
use crate::review::ignore::{
    ensure_reviewignore, load_review_ignore_matcher, review_ignore_reason,
};

use super::{Scope, SCOPE_VALIDATOR};

/// The resolved scope: the changed-file set, the sem-diff inputs, the per-file
/// after-content, the review-level change purpose, and blame's history anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedScope {
    pub(super) files: Vec<String>,
    pub(super) file_changes: Vec<SemFileChange>,
    pub(super) after_content: BTreeMap<String, String>,
    pub(super) change_purpose: String,
    /// The commit blame's history walk is bounded to, mirroring `git blame
    /// <blame_at> -- path`. [`Scope::Working`], [`Scope::File`], and
    /// [`Scope::Glob`] set this to [`working_tree_blame_anchor`]'s merge-base
    /// pin (a stable anchor for the life of a branch), falling back to `None`
    /// (blame against HEAD) only when no such anchor exists. [`Scope::Sha`]
    /// sets this to the range's "to" commit so a bounded historical review
    /// never attributes a line to a commit past that point.
    pub(super) blame_at: Option<git2::Oid>,
}

/// Resolve a [`Scope`] to its changed-file set and the inputs every later step
/// needs (sem-diff `FileChange`s, after-content, change purpose).
pub(super) fn resolve_scope_files(
    scope: &Scope,
    repo_path: &Path,
) -> Result<ResolvedScope, AvpError> {
    // Auto-generate `.reviewignore` (defaulting to `.kanban/`) on the first
    // review of any repo, never clobbering a user-edited one. It is untracked
    // and non-code, so it never enters the working scope resolved below.
    ensure_reviewignore(repo_path)?;

    let resolved = match scope {
        Scope::Working => resolve_working(repo_path)?,
        Scope::Sha(range) => resolve_sha(repo_path, range)?,
        Scope::File(path) => resolve_file(repo_path, path)?,
        Scope::Glob(pattern) => resolve_glob(repo_path, pattern)?,
    };

    // Uniform choke point: every scope's resolved file set is filtered through
    // the same `.reviewignore` + `.gitignore` matcher, so a `.kanban/` board or a
    // gitignored artifact is dropped identically whether it arrived via Working,
    // Sha, File, or Glob. The per-scope resolver above has already read each
    // candidate's disk/blob content; the matcher discards an ignored path's entry
    // here so that content never reaches the review agent. Escape paths are
    // rejected independently and earlier by `confine_to_repo`, so this filter is
    // about relevance, not containment.
    let matcher = load_review_ignore_matcher(repo_path)?;
    Ok(filter_resolved_scope(resolved, &matcher))
}

/// Drop every resolved file the review-scope ignore `matcher` excludes, keeping
/// the three views of the scope (paths, sem-diff inputs, after-content) mutually
/// consistent.
///
/// A `Scope::File` naming an ignored path therefore resolves to an empty scope —
/// consistent with the other scopes, never an error. Each excluded path is logged
/// at DEBUG with its FULL path and the excluding pattern's source, never truncated.
pub(super) fn filter_resolved_scope(resolved: ResolvedScope, matcher: &Gitignore) -> ResolvedScope {
    let mut kept: Vec<String> = Vec::with_capacity(resolved.files.len());
    for path in &resolved.files {
        match review_ignore_reason(matcher, path) {
            Some(pattern) => tracing::debug!(
                path = %path,
                pattern = %pattern,
                "review scope: excluded ignored path"
            ),
            None => kept.push(path.clone()),
        }
    }

    retain_scope_files(resolved, kept)
}

/// Narrow `resolved` to the `kept` paths, keeping its three views of the scope
/// (paths, sem-diff inputs, after-content) mutually consistent.
///
/// The single place a scope-stage filter narrows a [`ResolvedScope`], shared by
/// the `.reviewignore` filter above and the validator-fixture split in
/// [`super::fixtures`], so the two cannot drop a path from one view and leave it
/// in another.
pub(super) fn retain_scope_files(resolved: ResolvedScope, kept: Vec<String>) -> ResolvedScope {
    let ResolvedScope {
        files: _,
        file_changes,
        after_content,
        change_purpose,
        blame_at,
    } = resolved;

    let keep: BTreeSet<&str> = kept.iter().map(String::as_str).collect();
    let file_changes = file_changes
        .into_iter()
        .filter(|change| keep.contains(change.file_path.as_str()))
        .collect();
    let after_content = after_content
        .into_iter()
        .filter(|(path, _)| keep.contains(path.as_str()))
        .collect();

    ResolvedScope {
        files: kept,
        file_changes,
        after_content,
        change_purpose,
        blame_at,
    }
}

/// Open the repo, mapping git failures to [`AvpError::Context`].
pub(super) fn open_repo(repo_path: &Path) -> Result<GitOperations, AvpError> {
    GitOperations::with_work_dir(repo_path)
        .map_err(|e| AvpError::Context(format!("failed to open git repo: {e}")))
}

/// The [`AvpError::Validator`] raised for a scope path that resolves outside the
/// repository root. Carries the FULL, untruncated offending path so the caller
/// can see exactly what was rejected; the message is lowercase and unpunctuated.
pub(super) fn path_escapes_repo_root(path: &str) -> AvpError {
    AvpError::Validator {
        validator: SCOPE_VALIDATOR.to_string(),
        message: format!("path '{path}' escapes the repository root"),
    }
}

/// Lexically normalize an absolute path, resolving `.` and `..` components
/// WITHOUT touching the filesystem.
///
/// Used to contain a not-yet-existing candidate, which [`Path::canonicalize`]
/// cannot resolve (it requires every component to exist). A `..` that would
/// climb above the root pops past it, so the resulting path no longer starts
/// with the root and the containment check rejects it.
pub(super) fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve a repo-relative scope `path` to an on-disk path guaranteed to lie
/// under `repo_path`, enforcing the review-scope containment contract.
///
/// Review paths are repo-relative by contract, so an absolute input is rejected
/// outright: [`Path::join`] with an absolute argument REPLACES the base
/// entirely, which would otherwise read an arbitrary file (e.g. `/etc/passwd`).
/// For a relative input the candidate is joined onto the canonicalized root,
/// then contained: an existing candidate is canonicalized (following symlinks,
/// so a link whose target escapes the root is caught), and a not-yet-existing
/// one is normalized lexically (preserving the absent-path `Ok(None)` behavior
/// its caller relies on). Any resolved path not under the root is rejected.
///
/// # Errors
///
/// [`AvpError::Validator`] via [`path_escapes_repo_root`] when `path` is
/// absolute or resolves outside the repository root; [`AvpError::Context`] when
/// the root or an existing candidate cannot be canonicalized.
pub(super) fn confine_to_repo(repo_path: &Path, path: &str) -> Result<PathBuf, AvpError> {
    if Path::new(path).is_absolute() {
        return Err(path_escapes_repo_root(path));
    }
    let root = repo_path.canonicalize().map_err(|e| {
        AvpError::Context(format!(
            "failed to canonicalize repo root {}: {e}",
            repo_path.display()
        ))
    })?;
    let candidate = root.join(path);
    let resolved = match candidate.canonicalize() {
        Ok(real) => real,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => normalize_lexically(&candidate),
        Err(e) => {
            return Err(AvpError::Context(format!(
                "failed to resolve working-tree file {path}: {e}"
            )))
        }
    };
    if !resolved.starts_with(&root) {
        return Err(path_escapes_repo_root(path));
    }
    // Return the RESOLVED (canonicalized-when-present) path rather than the raw
    // join, so the subsequent read does not re-walk symlinks — closing the
    // check-then-read TOCTOU window against a concurrent filesystem swap.
    Ok(resolved)
}

/// Read a path's working-tree content from disk, confined to the repo root.
///
/// The `path` is a repo-relative scope target; it is first resolved through
/// [`confine_to_repo`], which rejects any absolute input or `..`/symlink escape
/// so a `review file` caller can never make the pipeline read a file outside the
/// repository into the review agent's context.
///
/// Returns `Ok(None)` only when the (contained) path is **absent** (the intended
/// deletion/added signal — a file gone from the working tree). Any *other*
/// failure — a permission error, or a binary/non-UTF8 file that
/// [`read_to_string`](std::fs::read_to_string) rejects — is propagated as
/// [`AvpError::Context`] rather than collapsed to `None`, so an unreadable
/// tracked file is never silently diffed as wholly added/removed. A containment
/// violation surfaces as [`AvpError::Validator`].
///
/// # Errors
///
/// [`AvpError::Validator`] when `path` escapes the repository root (see
/// [`confine_to_repo`]); [`AvpError::Context`] for a non-absent read failure.
pub(super) fn read_working(repo_path: &Path, path: &str) -> Result<Option<String>, AvpError> {
    let resolved = confine_to_repo(repo_path, path)?;
    match std::fs::read_to_string(resolved) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AvpError::Context(format!(
            "failed to read working-tree file {path}: {e}"
        ))),
    }
}

/// A git refspec — the revision half of a `refspec:path` blob address. Any
/// commit-ish the engine reads content at: `HEAD` (see [`GitRefSpec::head`]), a
/// sha, a branch, a tag, `HEAD~3`.
///
/// Distinct from [`FilePath`] on purpose. Both halves of a blob address are
/// strings, so the compiler is the only thing that can stop a call site passing
/// them in the wrong order; giving each half its own type makes the
/// transposition a type error instead of a silent mis-read.
///
/// This is deliberately **not** [`swissarmyhammer_git::BranchName`], the
/// workspace's other git-string newtype: that type's validation rejects `~`,
/// `^`, `:` and `..` — exactly the syntax a refspec needs — so it can only hold
/// a refspec via `new_unchecked`, which would defeat the type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct GitRefSpec(pub(super) String);

impl GitRefSpec {
    /// Wrap a commit-ish.
    pub(super) fn new(refspec: impl Into<String>) -> Self {
        Self(refspec.into())
    }

    /// The current checkout tip — the implicit "before" side of a working-tree or
    /// single-file scope, and the implicit "to" side of a bare-ref range. This is
    /// the single place the `HEAD` literal appears; every caller goes through it.
    pub(super) fn head() -> Self {
        Self("HEAD".to_string())
    }

    /// The refspec as libgit2 wants it.
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GitRefSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A repo-relative file path — the path half of a `refspec:path` blob address.
///
/// Distinct from [`GitRefSpec`] so the two halves cannot be transposed at a call
/// site; see that type for why the pair is typed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct FilePath(pub(super) String);

impl FilePath {
    /// Wrap a repo-relative path.
    pub(super) fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Unwrap the path for a consumer that stores it as a plain `String`.
    pub(super) fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for FilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Read a blob at `refspec:path` via libgit2.
///
/// This is the same `git show refspec:path` content read the git tool does, via
/// the shared `swissarmyhammer-git` repository handle instead of a shell-out.
///
/// The two halves of the address are separate types ([`GitRefSpec`],
/// [`FilePath`]) so no call site can transpose them.
///
/// Returns `Ok(None)` only when the path does **not exist** at the ref (the
/// intended Added/Deleted signal — `revparse_single` resolving to not-found, or
/// the object not being a blob). A blob that exists but cannot be read — a
/// binary/non-UTF8 tracked file, or any other libgit2 failure — is propagated as
/// [`AvpError::Context`], so an unreadable tracked file is never silently diffed
/// as wholly added/removed.
pub(super) fn read_at_ref(
    repo: &GitOperations,
    refspec: GitRefSpec,
    path: FilePath,
) -> Result<Option<String>, AvpError> {
    // The blob address, composed once and reused by the read and both failure
    // messages, so the `refspec:path` form lives in a single place.
    let spec = format!("{refspec}:{path}");
    let inner = repo.repository().inner();
    let object = match inner.revparse_single(&spec) {
        Ok(object) => object,
        // The path is absent at this ref — the intended Added/Deleted signal.
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(AvpError::Context(format!("failed to resolve {spec}: {e}"))),
    };
    // Not a blob (e.g. a tree at that path) — there is no file content to read.
    let Some(blob) = object.as_blob() else {
        return Ok(None);
    };
    String::from_utf8(blob.content().to_vec())
        .map(Some)
        .map_err(|e| AvpError::Context(format!("blob {spec} is not valid UTF-8: {e}")))
}

/// Resolve the working-tree scope: uncommitted changes vs HEAD (staged +
/// unstaged + untracked), reusing the git tool's changed-file accounting.
pub(super) fn resolve_working(repo_path: &Path) -> Result<ResolvedScope, AvpError> {
    let repo = open_repo(repo_path)?;
    let status = repo
        .get_status()
        .map_err(|e| AvpError::Context(format!("failed to read git status: {e}")))?;
    // Tracked changes (deliberate edits) keep current behavior — per-validator
    // globs decide what's reviewed. UNTRACKED entries are filtered to code files
    // via the canonical `swissarmyhammer-sem` extension list: brand-new source
    // gets reviewed because it WILL be added, while unignored junk (logs, jsonl,
    // lockfiles) never has its content read into scope.
    let mut files = status.all_changed_files();
    files.extend(status.untracked.iter().filter(|p| is_code_file(p)).cloned());
    files.sort();
    files.dedup();

    // Read each candidate's working-tree content once. A file with no readable
    // content (a deletion) carries `None` here and is diffed as a deletion.
    let after_by_path: BTreeMap<String, Option<String>> = files
        .iter()
        .map(|path| Ok((path.clone(), read_working(repo_path, path)?)))
        .collect::<Result<_, AvpError>>()?;

    let mut builder = FileChangeBuilder::new();
    for path in &files {
        let after = AfterContent::new(after_by_path.get(path).cloned().unwrap_or(None));
        let before =
            BeforeContent::new(read_at_ref(&repo, GitRefSpec::head(), FilePath::new(path))?);
        builder.push(FilePath::new(path), FileVersions { before, after });
    }
    // Blame anchor: pinned to the branch's merge-base with main/master (see
    // `working_tree_blame_anchor`) so the sha column means the same thing on
    // every run for the life of this branch, rather than drifting with every
    // intervening commit. `None` when no stable anchor exists (falls back to
    // HEAD, the pre-existing behavior).
    Ok(builder.finish(
        files,
        auto_purpose("working-tree changes"),
        working_tree_blame_anchor(&repo),
    ))
}

/// Resolve a commit/range scope, reusing the git tool's range semantics
/// (`from..to`, or a single ref treated as `ref..HEAD`).
pub(super) fn resolve_sha(repo_path: &Path, range: &str) -> Result<ResolvedScope, AvpError> {
    let repo = open_repo(repo_path)?;
    let files = repo
        .get_changed_files_from_range(range)
        .map_err(|e| AvpError::Context(format!("failed to resolve range '{range}': {e}")))?;

    let (from_ref, to_ref) = match range.split_once("..") {
        Some((from, to)) => (GitRefSpec::new(from), GitRefSpec::new(to)),
        None => (GitRefSpec::new(range), GitRefSpec::head()),
    };

    let mut builder = FileChangeBuilder::new();
    for path in &files {
        let before = BeforeContent::new(read_at_ref(&repo, from_ref.clone(), FilePath::new(path))?);
        let after = AfterContent::new(read_at_ref(&repo, to_ref.clone(), FilePath::new(path))?);
        builder.push(FilePath::new(path), FileVersions { before, after });
    }

    let purpose = commit_messages(&repo, &to_ref)
        .unwrap_or_else(|| auto_purpose(&format!("changes in range {range}")));
    // Bound blame to the range's "to" endpoint: a historical review must
    // never attribute a line to a commit past the point it reviews.
    Ok(builder.finish(files, purpose, resolve_oid(&repo, &to_ref)))
}

/// Resolve a single-file scope: its working-tree changes if any, else its whole
/// content reviewed as all-added work.
///
/// `path` is repo-relative by contract. Its working-tree read goes through
/// [`read_working`] → [`confine_to_repo`], so a `review file` target that is
/// absolute or escapes the repository root (via `..` or a symlink) is rejected
/// with [`AvpError::Validator`] and its content is never read into scope.
pub(super) fn resolve_file(repo_path: &Path, path: &str) -> Result<ResolvedScope, AvpError> {
    let repo = open_repo(repo_path)?;
    let after = AfterContent::new(read_working(repo_path, path)?);
    let before = BeforeContent::new(read_at_ref(&repo, GitRefSpec::head(), FilePath::new(path))?);

    let mut builder = FileChangeBuilder::new();
    builder.push(FilePath::new(path), FileVersions { before, after });
    // Blame anchor: same stable merge-base pin as `resolve_working` — see
    // `working_tree_blame_anchor`.
    Ok(builder.finish(
        vec![path.to_string()],
        auto_purpose(&format!("review of {path}")),
        working_tree_blame_anchor(&repo),
    ))
}

/// Resolve a glob scope: every matching tracked file as whole-content work (no
/// before side, so each diffs as all-added).
pub(super) fn resolve_glob(repo_path: &Path, pattern: &str) -> Result<ResolvedScope, AvpError> {
    let compiled = glob::Pattern::new(pattern).map_err(|e| AvpError::Validator {
        validator: SCOPE_VALIDATOR.to_string(),
        message: format!("invalid glob pattern '{pattern}': {e}"),
    })?;

    let repo = open_repo(repo_path)?;
    let tracked = repo
        .get_all_tracked_files()
        .map_err(|e| AvpError::Context(format!("failed to list tracked files: {e}")))?;
    let files: Vec<String> = tracked
        .into_iter()
        .filter(|f| compiled.matches_with(f, crate::validators::GLOB_MATCH_OPTIONS))
        .collect();

    let mut builder = FileChangeBuilder::new();
    for path in &files {
        // A glob scope has no base side: every matched file diffs as all-added.
        let after = AfterContent::new(read_working(repo_path, path)?);
        builder.push(
            FilePath::new(path),
            FileVersions {
                before: BeforeContent::absent(),
                after,
            },
        );
    }
    // Blame anchor: same stable merge-base pin as `resolve_working` — see
    // `working_tree_blame_anchor`.
    Ok(builder.finish(
        files,
        auto_purpose(&format!("files matching {pattern}")),
        working_tree_blame_anchor(&repo),
    ))
}

/// Wrap a one-line auto summary as the review-level change purpose.
pub(super) fn auto_purpose(what: &str) -> String {
    format!("Auto summary: reviewing {what}.")
}

/// The stable blame anchor for a working-tree-backed scope ([`Scope::Working`],
/// [`Scope::File`], [`Scope::Glob`]): the merge-base between `HEAD` and the
/// detected `main`/`master` branch.
///
/// Those three scopes read the file's LIVE working-tree content, which can
/// change shape (dirty → committed, tracked → staged) between two runs
/// without the underlying finding changing at all — a `/finish`-style loop
/// commits between iterations (`git add -A && git commit`), which sweeps up
/// every dirty file, not just the one whose finding it resolved. Binding
/// blame to `HEAD` (as `None` does) means every such commit — even one that
/// never touches the file under review — moves the anchor forward, so the
/// SAME still-open, byte-identical line flips from `worktree` to a real
/// commit sha the moment ANY intervening commit lands.
///
/// The merge-base with `main`/`master` does not move for the life of a
/// feature/task branch (main only moves if someone advances it, which a
/// `/finish` loop never does): every commit the loop makes lands strictly
/// AFTER this anchor, so blame bounded here never sees them — a line that
/// is `worktree` on the branch's first review stays `worktree` on every
/// later review, for as long as it postdates the anchor, regardless of how
/// many intervening commits happen. The column then answers one fixed
/// question all session long: "did this line exist before this unit of work
/// started?" — never "what does HEAD say right now?"
///
/// Falls back to `None` (blame against `HEAD`, the pre-existing behavior)
/// when no `main`/`master` branch exists, `HEAD` cannot be resolved, or the
/// two share no common ancestor — including the case of reviewing directly
/// ON `main` itself, where the merge-base IS `HEAD` and this degrades
/// transparently to the old per-run behavior. Blame attribution is always
/// best-effort, never load-bearing for the review itself.
pub(super) fn working_tree_blame_anchor(repo: &GitOperations) -> Option<git2::Oid> {
    let main_branch = repo.main_branch().ok()?;
    let head_oid = resolve_oid(repo, &GitRefSpec::head())?;
    let main_oid = resolve_oid(repo, &GitRefSpec::new(main_branch))?;
    repo.repository()
        .inner()
        .merge_base(head_oid, main_oid)
        .ok()
}

/// Resolve a refspec to its commit [`git2::Oid`] via libgit2, `None` when
/// unresolvable — the blame anchor [`resolve_sha`] binds a bounded historical
/// review's blame calls to. An unresolvable ref degrades to `None` (blame
/// against HEAD) rather than failing the whole scope resolution: blame
/// attribution is best-effort, never load-bearing for the review itself.
pub(super) fn resolve_oid(repo: &GitOperations, refspec: &GitRefSpec) -> Option<git2::Oid> {
    let inner = repo.repository().inner();
    let object = inner.revparse_single(refspec.as_str()).ok()?;
    object.peel_to_commit().ok().map(|c| c.id())
}

/// Read the commit message for a ref via libgit2, `None` when unresolvable.
pub(super) fn commit_messages(repo: &GitOperations, refspec: &GitRefSpec) -> Option<String> {
    let inner = repo.repository().inner();
    let object = inner.revparse_single(refspec.as_str()).ok()?;
    let commit = object.peel_to_commit().ok()?;
    let message = commit.message().unwrap_or("").trim().to_string();
    if message.is_empty() {
        None
    } else {
        Some(message)
    }
}

/// A file's content at the **base** revision of the change — `None` when the
/// file did not exist there (the Added signal).
///
/// Distinct from [`AfterContent`] on purpose, and the sharper case of the same
/// hazard as [`GitRefSpec`]/[`FilePath`]: both sides are `Option<String>` and
/// they arrive together at [`FileChangeBuilder::push`], so nothing but the
/// compiler can stop a call site swapping them — and a swap does not fail
/// loudly, it flips [`FileStatus::Added`] to [`FileStatus::Deleted`] and hands
/// the review a plausible-looking INVERTED diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BeforeContent(pub(super) Option<String>);

impl BeforeContent {
    /// Wrap the base-revision content of a file.
    pub(super) fn new(content: Option<String>) -> Self {
        Self(content)
    }

    /// The absent base side — a file that did not exist before the change.
    pub(super) fn absent() -> Self {
        Self(None)
    }

    /// Unwrap for the sem-diff input.
    pub(super) fn into_inner(self) -> Option<String> {
        self.0
    }
}

/// A file's content **after** the change — `None` when the file no longer
/// exists (the Deleted signal).
///
/// Distinct from [`BeforeContent`] so the two sides cannot be transposed; see
/// that type for what a transposition would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AfterContent(pub(super) Option<String>);

impl AfterContent {
    /// Wrap the post-change content of a file.
    pub(super) fn new(content: Option<String>) -> Self {
        Self(content)
    }

    /// Unwrap for the sem-diff input.
    pub(super) fn into_inner(self) -> Option<String> {
        self.0
    }
}

/// Both sides of one file's change, named rather than positional.
///
/// [`FileChangeBuilder::push`] takes this single argument instead of two
/// `Option<String>`s: the fields name each side at the call site, and their
/// distinct types ([`BeforeContent`], [`AfterContent`]) make a transposed
/// struct literal a compile error rather than an inverted diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileVersions {
    /// The content at the base revision.
    pub(super) before: BeforeContent,
    /// The content after the change.
    pub(super) after: AfterContent,
}

/// Accumulates the per-file sem-diff inputs and after-content as files resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileChangeBuilder {
    pub(super) file_changes: Vec<SemFileChange>,
    pub(super) after_content: BTreeMap<String, String>,
}

impl FileChangeBuilder {
    /// Create a new, empty [`FileChangeBuilder`].
    pub(super) fn new() -> Self {
        Self {
            file_changes: Vec::new(),
            after_content: BTreeMap::new(),
        }
    }

    /// Record one file's before/after content for the sem differ.
    ///
    /// The two sides arrive as one named-field [`FileVersions`], so they cannot
    /// be transposed into an inverted diff.
    pub(super) fn push(&mut self, path: FilePath, versions: FileVersions) {
        let FileVersions { before, after } = versions;
        let (before, after) = (before.into_inner(), after.into_inner());
        let path = path.into_string();
        if let Some(content) = &after {
            self.after_content.insert(path.clone(), content.clone());
        }
        let status = match (&before, &after) {
            (None, Some(_)) => FileStatus::Added,
            (Some(_), None) => FileStatus::Deleted,
            _ => FileStatus::Modified,
        };
        self.file_changes.push(SemFileChange {
            file_path: path,
            status,
            old_file_path: None,
            before_content: before,
            after_content: after,
        });
    }

    /// Finish into a [`ResolvedScope`]. `blame_at` is the commit blame's
    /// history walk is bounded to (see [`ResolvedScope::blame_at`]).
    pub(super) fn finish(
        self,
        files: Vec<String>,
        change_purpose: String,
        blame_at: Option<git2::Oid>,
    ) -> ResolvedScope {
        ResolvedScope {
            files,
            file_changes: self.file_changes,
            after_content: self.after_content,
            change_purpose,
            blame_at,
        }
    }
}
