//! Engine stage 1 — resolve a review scope into a per-validator work-list.
//!
//! This is the first, deterministic, LLM-free stage of the review pipeline. Given
//! a [`Scope`] (exactly one of `working` / `sha` / `file` / `glob`) it produces a
//! [`WorkList`]: the review-level [change purpose](WorkList::change_purpose) plus,
//! per matched validator, the files to review. The **validator is the shard; the
//! file is the grain** — each [`FileWork`] carries that file's structured semantic
//! diff, the changed symbols, a *bounded* [`source_slice`](FileWork::source_slice)
//! (header + changed entities + hunk windows, never the whole file), and the
//! engine-run probe evidence for the validator's declared probes.
//!
//! # Reuse, never reimplement
//!
//! The stage composes existing pieces and adds no git/glob/probe logic of its own:
//!
//! - **Diff scopes** reuse the same library primitives the `git` tool is built on:
//!   [`swissarmyhammer_git::GitOperations`] for the changed-file set (working tree,
//!   range/sha) and [`compute_semantic_diff`] for the entity-level diff. The `git`
//!   MCP tool itself lives in the `swissarmyhammer-tools` crate and is *not*
//!   library-callable from the engine (depending on it would invert the dependency
//!   direction), so — as the task authorizes — this is the factored shared git-ops
//!   call site: it calls the underlying `swissarmyhammer-git` + `swissarmyhammer-sem`
//!   crates directly, exactly as the tool does, never shelling out, never
//!   reimplementing diffing.
//! - **Validator matching** reuses [`crate::match_rules`]' matching code path via a
//!   caller-supplied [`ValidatorLoader`] (`matching_rulesets`), so the loader is
//!   built once rather than reloaded per file.
//! - **Probes** reuse [`crate::review::run_probes`]; each distinct `(file, probe)`
//!   runs exactly once and the shared result is handed to every validator that
//!   declared it — N+M probe calls for a large diff, never N×M.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use model_embedding::TextEmbedder;
use rusqlite::Connection;
use serde::Serialize;

use swissarmyhammer_git::GitOperations;
use swissarmyhammer_sem::git_types::{FileChange as SemFileChange, FileStatus};
use swissarmyhammer_sem::model::change::SemanticChange;
use swissarmyhammer_sem::parser::differ::compute_semantic_diff;
use swissarmyhammer_sem::parser::plugins::create_default_registry;

use crate::error::AvpError;
use crate::review::probes::{run_probes, ChangeEntry, FileChange as ProbeChange, ProbeResult};
use crate::validators::{MatchContext, RuleSet, Severity, ValidatorLoader};

/// How many lines of context to keep on each side of a changed hunk in the
/// bounded [`source_slice`](FileWork::source_slice).
const HUNK_WINDOW_LINES: usize = 40;

/// How many leading lines of a file count as its "header" (imports / module
/// declaration) for the bounded slice.
const HEADER_LINES: usize = 20;

/// The review scope — exactly one of these resolves to a file set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Uncommitted changes vs HEAD (staged + unstaged + untracked). The default.
    Working,
    /// Changes in/since a commit or range.
    Sha(String),
    /// A single file path.
    File(String),
    /// All files matching a glob pattern.
    Glob(String),
}

/// A forgiving scope input that enforces "exactly one of file/glob/working/sha".
#[derive(Debug, Clone, Default)]
pub struct ScopeSpec {
    /// Resolve the working tree.
    pub working: bool,
    /// Resolve a commit or range.
    pub sha: Option<String>,
    /// Resolve a single file path.
    pub file: Option<String>,
    /// Resolve a glob pattern.
    pub glob: Option<String>,
}

impl ScopeSpec {
    /// Resolve to exactly one [`Scope`], erroring on zero or multiple selectors.
    ///
    /// # Errors
    ///
    /// Returns [`AvpError::Validator`] when none of `working`/`sha`/`file`/`glob`
    /// is set, or when more than one is.
    pub fn resolve(self) -> Result<Scope, AvpError> {
        let mut chosen: Vec<Scope> = Vec::new();
        if self.working {
            chosen.push(Scope::Working);
        }
        if let Some(sha) = self.sha {
            chosen.push(Scope::Sha(sha));
        }
        if let Some(file) = self.file {
            chosen.push(Scope::File(file));
        }
        if let Some(glob) = self.glob {
            chosen.push(Scope::Glob(glob));
        }

        match chosen.len() {
            1 => Ok(chosen.into_iter().next().expect("len checked")),
            0 => Err(AvpError::Validator {
                validator: "scope".to_string(),
                message:
                    "a review scope must set exactly one of file/glob/working/sha; none were set"
                        .to_string(),
            }),
            n => Err(AvpError::Validator {
                validator: "scope".to_string(),
                message: format!(
                    "a review scope must set exactly one of file/glob/working/sha; {n} were set"
                ),
            }),
        }
    }
}

/// The per-validator review work-list — the output of [`scope_review`].
#[derive(Debug, Clone, Serialize)]
pub struct WorkList {
    /// The review-level intent.
    pub change_purpose: String,
    /// One entry per validator that matched at least one changed file.
    pub validators: Vec<ValidatorWork>,
}

/// One matched validator's slice of the work-list.
#[derive(Debug, Clone, Serialize)]
pub struct ValidatorWork {
    /// The validator (RuleSet) name.
    pub validator_name: String,
    /// The validator's severity.
    pub severity: Severity,
    /// The rule names inside the validator.
    pub rules: Vec<String>,
    /// The probe names the validator declared.
    pub probes: Vec<String>,
    /// The files this validator must review.
    pub files: Vec<FileWork>,
}

/// One file's worth of work for one validator.
#[derive(Debug, Clone, Serialize)]
pub struct FileWork {
    /// The file path.
    pub path: String,
    /// The changed entities from the semantic diff.
    pub semantic_diff: Vec<SemanticChange>,
    /// The names of the changed symbols.
    pub changed_symbols: Vec<String>,
    /// A bounded slice of source.
    pub source_slice: String,
    /// The shared `(file, probe)` results.
    pub probe_results: Vec<ProbeResult>,
}

/// Resolve a review scope into a per-validator [`WorkList`].
///
/// Deterministic and LLM-free: it resolves `scope` to a changed-file set, diffs
/// each file semantically, matches validators against each file, runs each
/// distinct `(file, probe)` once, and groups the bounded per-file work under the
/// validators that matched.
///
/// `repo_path` is the repository root; `loader` is a fully-loaded
/// [`ValidatorLoader`] (built once via [`crate::load_rules`]); `conn` is the
/// caller-resolved code_context index connection (never `current_dir()`);
/// `embedder` embeds probe query bodies.
///
/// # Change purpose
///
/// [`WorkList::change_purpose`] is the commit message(s) under [`Scope::Sha`] and
/// a one-line [`auto_purpose`] summary otherwise. The "kanban task title+body
/// when invoked task-mode" half of the change-purpose spec is not reachable from
/// this signature: task context is plumbed in a later wiring stage that wraps
/// this call, not derived inside the deterministic scope stage.
///
/// # Errors
///
/// Returns [`AvpError::Context`] on git or index failure, or
/// [`AvpError::Validator`] when a matched validator declares an unknown probe.
pub async fn scope_review(
    scope: Scope,
    repo_path: &Path,
    loader: &ValidatorLoader,
    conn: &Connection,
    embedder: &dyn TextEmbedder,
) -> Result<WorkList, AvpError> {
    let resolved = resolve_scope_files(&scope, repo_path)?;

    // The single semantic-diff pass: one `FileChange` per resolved file fed to
    // the sem differ once. Whole-content files (glob / unchanged single file)
    // carry only `after_content`, so they diff as all-added entities.
    let registry = create_default_registry();
    let diff = compute_semantic_diff(&resolved.file_changes, &registry, None, None);

    // Group the diff's entities by file, and derive the probe change-set (every
    // changed entity across the whole diff) so probes run over the real diff.
    let mut entities_by_file: BTreeMap<String, Vec<SemanticChange>> = BTreeMap::new();
    let mut change_entities: Vec<ChangeEntry> = Vec::new();
    for change in diff.changes {
        change_entities.push(to_probe_entry(&change));
        entities_by_file
            .entry(change.file_path.clone())
            .or_default()
            .push(change);
    }

    // Match validators per file via the shared `matching_rulesets` code path.
    let mut matched_files: BTreeSet<String> = BTreeSet::new();
    let mut validators: BTreeMap<String, MatchedValidator> = BTreeMap::new();
    for file in &resolved.files {
        let ctx = MatchContext::new().with_file(file.clone());
        let rulesets = loader.matching_rulesets(&ctx);
        if rulesets.is_empty() {
            continue;
        }
        matched_files.insert(file.clone());
        for rs in rulesets {
            validators
                .entry(rs.name().to_string())
                .or_insert_with(|| MatchedValidator::from_ruleset(rs))
                .files
                .insert(file.clone());
        }
    }

    // Run probes ONCE over the whole change set with the union of every declared
    // probe name. This is the N+M guarantee: each distinct `(file, probe)` is
    // computed exactly once and the shared result fans out to every validator
    // that declared it (the distribution below is a pure filter, never a re-run).
    let probe_cache = run_probe_cache(&validators, &change_entities, conn, embedder).await?;

    // Pre-compute the bounded slice + changed symbols per file once (shared by
    // every validator that reviews the same file).
    let mut per_file: BTreeMap<String, FileFacts> = BTreeMap::new();
    for file in &matched_files {
        let entities = entities_by_file.get(file).cloned().unwrap_or_default();
        let after = resolved.after_content.get(file).map(String::as_str);
        per_file.insert(
            file.clone(),
            FileFacts {
                changed_symbols: changed_symbols(&entities),
                source_slice: bounded_slice(after, &entities),
                semantic_diff: entities,
            },
        );
    }

    // Assemble the work-list: name-sorted validators, each carrying their matched
    // files (path-sorted) with the shared facts + their probe subset.
    let mut validator_work: Vec<ValidatorWork> = validators
        .into_values()
        .map(|mv| {
            let mut files: Vec<FileWork> = mv
                .files
                .iter()
                .map(|file| {
                    let facts = per_file.get(file).expect("matched file has facts");
                    FileWork {
                        path: file.clone(),
                        semantic_diff: facts.semantic_diff.clone(),
                        changed_symbols: facts.changed_symbols.clone(),
                        source_slice: facts.source_slice.clone(),
                        probe_results: select_probe_results(
                            &probe_cache,
                            file,
                            &facts.changed_symbols,
                            &mv.probes,
                        ),
                    }
                })
                .collect();
            files.sort_by(|a, b| a.path.cmp(&b.path));
            ValidatorWork {
                validator_name: mv.name,
                severity: mv.severity,
                rules: mv.rules,
                probes: mv.probes,
                files,
            }
        })
        .collect();
    validator_work.sort_by(|a, b| a.validator_name.cmp(&b.validator_name));

    Ok(WorkList {
        change_purpose: resolved.change_purpose,
        validators: validator_work,
    })
}

/// A validator matched to one or more files, accumulated during matching.
struct MatchedValidator {
    name: String,
    severity: Severity,
    rules: Vec<String>,
    probes: Vec<String>,
    files: BTreeSet<String>,
}

impl MatchedValidator {
    fn from_ruleset(rs: &RuleSet) -> Self {
        Self {
            name: rs.name().to_string(),
            severity: rs.manifest.severity,
            rules: rs.rules.iter().map(|r| r.name.clone()).collect(),
            probes: rs.manifest.probes.clone(),
            files: BTreeSet::new(),
        }
    }
}

/// The per-file facts shared across every validator that reviews the file.
struct FileFacts {
    semantic_diff: Vec<SemanticChange>,
    changed_symbols: Vec<String>,
    source_slice: String,
}

/// The resolved scope: the changed-file set, the sem-diff inputs, the per-file
/// after-content, and the review-level change purpose.
struct ResolvedScope {
    files: Vec<String>,
    file_changes: Vec<SemFileChange>,
    after_content: BTreeMap<String, String>,
    change_purpose: String,
}

/// Map a semantic-diff [`SemanticChange`] to the probe runner's [`ChangeEntry`].
fn to_probe_entry(change: &SemanticChange) -> ChangeEntry {
    ChangeEntry {
        change_type: change.change_type.to_string(),
        entity_type: change.entity_type.clone(),
        entity_name: change.entity_name.clone(),
        file_path: change.file_path.clone(),
        after_content: change.after_content.clone(),
    }
}

/// Resolve a [`Scope`] to its changed-file set and the inputs every later step
/// needs (sem-diff `FileChange`s, after-content, change purpose).
fn resolve_scope_files(scope: &Scope, repo_path: &Path) -> Result<ResolvedScope, AvpError> {
    match scope {
        Scope::Working => resolve_working(repo_path),
        Scope::Sha(range) => resolve_sha(repo_path, range),
        Scope::File(path) => resolve_file(repo_path, path),
        Scope::Glob(pattern) => resolve_glob(repo_path, pattern),
    }
}

/// Open the repo, mapping git failures to [`AvpError::Context`].
fn open_repo(repo_path: &Path) -> Result<GitOperations, AvpError> {
    GitOperations::with_work_dir(repo_path)
        .map_err(|e| AvpError::Context(format!("failed to open git repo: {e}")))
}

/// Read a path's working-tree content from disk, `None` when absent/unreadable.
fn read_working(repo_path: &Path, path: &str) -> Option<String> {
    std::fs::read_to_string(repo_path.join(path)).ok()
}

/// Read a blob at `ref:path` via libgit2, `None` when it doesn't resolve.
///
/// This is the same `git show ref:path` content read the git tool does, via the
/// shared `swissarmyhammer-git` repository handle instead of a shell-out.
fn read_at_ref(repo: &GitOperations, refspec: &str, path: &str) -> Option<String> {
    let inner = repo.repository().inner();
    let object = inner.revparse_single(&format!("{refspec}:{path}")).ok()?;
    let blob = object.as_blob()?;
    String::from_utf8(blob.content().to_vec()).ok()
}

/// Resolve the working-tree scope: uncommitted changes vs HEAD (staged +
/// unstaged + untracked), reusing the git tool's changed-file accounting.
fn resolve_working(repo_path: &Path) -> Result<ResolvedScope, AvpError> {
    let repo = open_repo(repo_path)?;
    let status = repo
        .get_status()
        .map_err(|e| AvpError::Context(format!("failed to read git status: {e}")))?;
    let mut files = status.all_changed_files();
    files.extend(status.untracked.clone());
    files.sort();
    files.dedup();

    let mut builder = FileChangeBuilder::new();
    for path in &files {
        let after = read_working(repo_path, path);
        let before = read_at_ref(&repo, "HEAD", path);
        builder.push(path, before, after);
    }
    Ok(builder.finish(files, auto_purpose("working-tree changes")))
}

/// Resolve a commit/range scope, reusing the git tool's range semantics
/// (`from..to`, or a single ref treated as `ref..HEAD`).
fn resolve_sha(repo_path: &Path, range: &str) -> Result<ResolvedScope, AvpError> {
    let repo = open_repo(repo_path)?;
    let files = repo
        .get_changed_files_from_range(range)
        .map_err(|e| AvpError::Context(format!("failed to resolve range '{range}': {e}")))?;

    let (from_ref, to_ref) = match range.split_once("..") {
        Some((from, to)) => (from.to_string(), to.to_string()),
        None => (range.to_string(), "HEAD".to_string()),
    };

    let mut builder = FileChangeBuilder::new();
    for path in &files {
        let before = read_at_ref(&repo, &from_ref, path);
        let after = read_at_ref(&repo, &to_ref, path);
        builder.push(path, before, after);
    }

    let purpose = commit_messages(&repo, &to_ref)
        .unwrap_or_else(|| auto_purpose(&format!("changes in range {range}")));
    Ok(builder.finish(files, purpose))
}

/// Resolve a single-file scope: its working-tree changes if any, else its whole
/// content reviewed as all-added work.
fn resolve_file(repo_path: &Path, path: &str) -> Result<ResolvedScope, AvpError> {
    let repo = open_repo(repo_path)?;
    let after = read_working(repo_path, path);
    let before = read_at_ref(&repo, "HEAD", path);

    let mut builder = FileChangeBuilder::new();
    builder.push(path, before, after);
    Ok(builder.finish(
        vec![path.to_string()],
        auto_purpose(&format!("review of {path}")),
    ))
}

/// Resolve a glob scope: every matching tracked file as whole-content work (no
/// before side, so each diffs as all-added).
fn resolve_glob(repo_path: &Path, pattern: &str) -> Result<ResolvedScope, AvpError> {
    let compiled = glob::Pattern::new(pattern).map_err(|e| AvpError::Validator {
        validator: "scope".to_string(),
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
        let after = read_working(repo_path, path);
        builder.push(path, None, after);
    }
    Ok(builder.finish(files, auto_purpose(&format!("files matching {pattern}"))))
}

/// Wrap a one-line auto summary as the review-level change purpose.
fn auto_purpose(what: &str) -> String {
    format!("Auto summary: reviewing {what}.")
}

/// Read the commit message for a ref via libgit2, `None` when unresolvable.
fn commit_messages(repo: &GitOperations, refspec: &str) -> Option<String> {
    let inner = repo.repository().inner();
    let object = inner.revparse_single(refspec).ok()?;
    let commit = object.peel_to_commit().ok()?;
    let message = commit.message().unwrap_or("").trim().to_string();
    if message.is_empty() {
        None
    } else {
        Some(message)
    }
}

/// Accumulates the per-file sem-diff inputs and after-content as files resolve.
struct FileChangeBuilder {
    file_changes: Vec<SemFileChange>,
    after_content: BTreeMap<String, String>,
}

impl FileChangeBuilder {
    fn new() -> Self {
        Self {
            file_changes: Vec::new(),
            after_content: BTreeMap::new(),
        }
    }

    /// Record one file's before/after content for the sem differ.
    fn push(&mut self, path: &str, before: Option<String>, after: Option<String>) {
        if let Some(content) = &after {
            self.after_content.insert(path.to_string(), content.clone());
        }
        let status = match (&before, &after) {
            (None, Some(_)) => FileStatus::Added,
            (Some(_), None) => FileStatus::Deleted,
            _ => FileStatus::Modified,
        };
        self.file_changes.push(SemFileChange {
            file_path: path.to_string(),
            status,
            old_file_path: None,
            before_content: before,
            after_content: after,
        });
    }

    /// Finish into a [`ResolvedScope`].
    fn finish(self, files: Vec<String>, change_purpose: String) -> ResolvedScope {
        ResolvedScope {
            files,
            file_changes: self.file_changes,
            after_content: self.after_content,
            change_purpose,
        }
    }
}

/// Build the shared probe-result cache from a single [`run_probes`] call over the
/// whole change set with the union of every validator's declared probes.
async fn run_probe_cache(
    validators: &BTreeMap<String, MatchedValidator>,
    change_entities: &[ChangeEntry],
    conn: &Connection,
    embedder: &dyn TextEmbedder,
) -> Result<Vec<ProbeResult>, AvpError> {
    let union: BTreeSet<String> = validators
        .values()
        .flat_map(|mv| mv.probes.iter().cloned())
        .collect();
    if union.is_empty() || change_entities.is_empty() {
        return Ok(Vec::new());
    }
    let names: Vec<String> = union.into_iter().collect();
    let change = ProbeChange::new(change_entities.to_vec());
    let results = run_probes(&names, &change, conn, embedder).await?;
    Ok(results.results)
}

/// Select the probe results that belong to `file` and the validator's declared
/// `probes`, from the shared single-run cache.
///
/// `changed_symbols` are this file's changed-entity names (the semantic diff's
/// `entity_name → file_path` mapping, pre-resolved per file), used to attach a
/// symbol-targeted probe result back to the file whose entity bears that name.
fn select_probe_results(
    cache: &[ProbeResult],
    file: &str,
    changed_symbols: &[String],
    probes: &[String],
) -> Vec<ProbeResult> {
    cache
        .iter()
        .filter(|r| probes.contains(&r.name))
        .filter(|r| probe_result_for_file(r, file, changed_symbols))
        .cloned()
        .collect()
}

/// Whether a probe result's bound subject relates to `file`.
///
/// Probe targets come in three shapes and each resolves to its file differently:
/// - **file path** (`duplicates` per-file) matches the path directly;
/// - **`<changed-set>`** (`duplicates` cross-file) is shared evidence and attaches
///   to every file that participated in the change;
/// - **symbol name** (`callers` / `similar`) resolves via the semantic diff's
///   `entity_name → file_path` mapping: it attaches to the file whose changed
///   entity bears that name (`changed_symbols` is that mapping, pre-filtered to
///   this file).
fn probe_result_for_file(result: &ProbeResult, file: &str, changed_symbols: &[String]) -> bool {
    result.target == file
        || result.target == "<changed-set>"
        || changed_symbols.iter().any(|s| s == &result.target)
}

/// The deduped, sorted names of the symbols changed by `entities`.
fn changed_symbols(entities: &[SemanticChange]) -> Vec<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for entity in entities {
        if !entity.entity_name.is_empty() {
            names.insert(entity.entity_name.clone());
        }
    }
    names.into_iter().collect()
}

/// Build the bounded source slice for a file: its header, each changed entity's
/// `after_content`, and a window around each changed entity's location in the
/// after-content — never the whole file.
fn bounded_slice(after: Option<&str>, entities: &[SemanticChange]) -> String {
    let mut sections: Vec<String> = Vec::new();

    // Header: the file's leading lines (imports / module decl).
    if let Some(content) = after {
        let header: Vec<&str> = content.lines().take(HEADER_LINES).collect();
        if !header.is_empty() {
            sections.push(header.join("\n"));
        }
    }

    // Each changed entity's full source, plus a window around its location.
    for entity in entities {
        if let Some(body) = &entity.after_content {
            sections.push(body.clone());
            if let Some(content) = after {
                if let Some(window) = hunk_window(content, body) {
                    sections.push(window);
                }
            }
        }
    }

    dedup_sections(sections).join("\n")
}

/// A ~`HUNK_WINDOW_LINES`-line window of `content` centered on where `body`
/// first appears, `None` when `body` is not found verbatim.
fn hunk_window(content: &str, body: &str) -> Option<String> {
    let first_body_line = body.lines().next()?.trim();
    if first_body_line.is_empty() {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();
    let idx = lines.iter().position(|l| l.trim() == first_body_line)?;
    let start = idx.saturating_sub(HUNK_WINDOW_LINES / 2);
    let end = (idx + HUNK_WINDOW_LINES / 2).min(lines.len());
    Some(lines[start..end].join("\n"))
}

/// Drop duplicate / fully-contained sections so the slice stays bounded and
/// doesn't repeat the same entity body via overlapping windows.
fn dedup_sections(sections: Vec<String>) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for section in sections {
        let trimmed = section.trim();
        if trimmed.is_empty() {
            continue;
        }
        if kept
            .iter()
            .any(|k| k.contains(trimmed) || trimmed.contains(k.as_str()))
        {
            continue;
        }
        kept.push(section);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::{Path, PathBuf};

    use model_embedding::mock::MockEmbedder;
    use rusqlite::Connection;
    use tempfile::TempDir;

    use swissarmyhammer_code_context::db::{configure_connection, create_schema};
    use swissarmyhammer_code_context::serialize_embedding;

    use crate::validators::types::{RuleSet, RuleSetManifest, RuleSetMetadata, ValidatorMatch};
    use crate::validators::{Rule, ValidatorLoader, ValidatorSource};

    /// Embedding dimension shared by the seeded index and the mock embedder.
    const DIM: usize = 4;

    // ---- git repo fixture -------------------------------------------------

    /// A throwaway git repo backed by a [`TempDir`], driven via libgit2 so the
    /// scope stage's real `swissarmyhammer-git` reads see real refs/working-tree.
    struct TestRepo {
        dir: TempDir,
        repo: git2::Repository,
    }

    impl TestRepo {
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let repo = git2::Repository::init(dir.path()).unwrap();
            {
                let mut cfg = repo.config().unwrap();
                cfg.set_str("user.name", "Test").unwrap();
                cfg.set_str("user.email", "test@example.com").unwrap();
            }
            Self { dir, repo }
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        /// Write a file to the working tree (no staging).
        fn write(&self, rel: &str, content: &str) {
            let full = self.dir.path().join(rel);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(full, content).unwrap();
        }

        /// Stage everything and commit, returning the commit sha.
        fn commit(&self, message: &str) -> String {
            let mut index = self.repo.index().unwrap();
            index
                .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
                .unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = self.repo.find_tree(tree_id).unwrap();
            let sig = git2::Signature::now("Test", "test@example.com").unwrap();
            let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
            let parents: Vec<&git2::Commit> = parent.iter().collect();
            let oid = self
                .repo
                .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
                .unwrap();
            oid.to_string()
        }
    }

    // ---- code_context index fixture --------------------------------------

    /// Open a real, schema-applied, in-memory code_context index (same shape the
    /// probe runner uses in production), seeded deterministically.
    fn index_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    /// Register a file so chunk rows can carry their foreign key.
    fn seed_file(conn: &Connection, file_path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed, embedded)
             VALUES (?1, X'DEADBEEF', 1024, 1000, 1, 1, 1)",
            rusqlite::params![file_path],
        )
        .unwrap();
    }

    /// Seed a `ts_chunks` row with an embedding so `find_duplicates` sees it.
    fn seed_chunk(conn: &Connection, file_path: &str, symbol_path: &str, text: &str, emb: &[f32]) {
        seed_file(conn, file_path);
        let blob = serialize_embedding(emb);
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
             VALUES (?1, 0, ?2, 1, 10, ?3, ?4, ?5)",
            rusqlite::params![file_path, text.len() as i64, symbol_path, text, blob],
        )
        .unwrap();
    }

    /// Seed an `lsp_symbols` row so the `callers` probe can resolve a symbol.
    fn seed_symbol(conn: &Connection, id: &str, name: &str, file_path: &str) {
        seed_file(conn, file_path);
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
             VALUES (?1, ?2, 12, ?3, 1, 0, 5, 0, NULL)",
            rusqlite::params![id, name, file_path],
        )
        .unwrap();
    }

    /// Seed an `lsp_call_edges` row (caller -> callee) for the `callers` probe.
    fn seed_call_edge(
        conn: &Connection,
        caller_id: &str,
        callee_id: &str,
        caller_file: &str,
        callee_file: &str,
    ) {
        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, source, from_ranges)
             VALUES (?1, ?2, ?3, ?4, 'lsp', '[]')",
            rusqlite::params![caller_id, callee_id, caller_file, callee_file],
        )
        .unwrap();
    }

    // ---- validator loader fixture ----------------------------------------

    /// A loader carrying one RuleSet named `name` that matches `file_glob` and
    /// declares `probes`. `add_builtin_ruleset` is the deterministic injection
    /// seam (no on-disk validators, so tests don't depend on the machine).
    fn loader_with(name: &str, file_glob: &str, probes: &[&str]) -> ValidatorLoader {
        let mut loader = ValidatorLoader::new();
        loader.add_builtin_ruleset(ruleset(name, file_glob, probes));
        loader
    }

    fn ruleset(name: &str, file_glob: &str, probes: &[&str]) -> RuleSet {
        RuleSet {
            manifest: RuleSetManifest {
                name: name.to_string(),
                description: format!("{name} test ruleset"),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                match_criteria: Some(ValidatorMatch {
                    tools: vec![],
                    files: vec![file_glob.to_string()],
                }),
                trigger_matcher: None,
                tags: vec![],
                probes: probes.iter().map(|p| p.to_string()).collect(),
                severity: Severity::Warn,
                timeout: 30,
                once: false,
            },
            rules: vec![Rule {
                name: format!("{name}-rule"),
                description: "rule".to_string(),
                body: "body".to_string(),
                severity: None,
                timeout: None,
            }],
            source: ValidatorSource::Builtin,
            base_path: PathBuf::from("/test"),
        }
    }

    /// A function body long enough to clear the default `min_chunk_bytes` (100).
    fn body(label: &str) -> String {
        format!(
            "pub fn {label}(input: &[f64]) -> f64 {{\n    let mut total = 0.0;\n    for value in input {{\n        total += value * value;\n    }}\n    total / input.len() as f64\n}}"
        )
    }

    // ---- ScopeSpec::resolve ----------------------------------------------

    #[test]
    fn scope_spec_resolves_exactly_one_selector() {
        let spec = ScopeSpec {
            working: true,
            ..Default::default()
        };
        assert_eq!(spec.resolve().unwrap(), Scope::Working);

        let spec = ScopeSpec {
            sha: Some("HEAD~1".to_string()),
            ..Default::default()
        };
        assert_eq!(spec.resolve().unwrap(), Scope::Sha("HEAD~1".to_string()));
    }

    #[test]
    fn scope_spec_errors_on_zero_selectors() {
        let err = ScopeSpec::default().resolve().unwrap_err();
        match err {
            AvpError::Validator { message, .. } => {
                assert!(message.contains("none"), "got: {message}");
            }
            other => panic!("expected Validator error, got: {other:?}"),
        }
    }

    #[test]
    fn scope_spec_errors_on_multiple_selectors() {
        let spec = ScopeSpec {
            working: true,
            file: Some("a.rs".to_string()),
            ..Default::default()
        };
        let err = spec.resolve().unwrap_err();
        match err {
            AvpError::Validator { message, .. } => {
                assert!(message.contains('2'), "got: {message}");
            }
            other => panic!("expected Validator error, got: {other:?}"),
        }
    }

    // ---- scope_review: working scope, duplicate function ------------------

    #[tokio::test]
    async fn working_scope_groups_duplicate_under_validator_with_bounded_slice() {
        let repo = TestRepo::new();
        // Header = imports; an unrelated marker sits in the MIDDLE of the file,
        // outside both the header window and any changed-hunk window.
        let mid_padding: String = (0..30).map(|i| format!("// mid {i}\n")).collect();
        let tail_padding: String = (0..30).map(|i| format!("// tail {i}\n")).collect();
        let base = format!(
            "use std::fmt;\nuse std::io;\n{mid_padding}fn distant_unrelated_marker() {{}}\n{tail_padding}"
        );
        repo.write("src/lib.rs", &base);
        repo.commit("initial");

        // The working-tree change ADDS a duplicate function at the very bottom.
        let dup = body("compute");
        repo.write("src/lib.rs", &format!("{base}\n{dup}\n"));

        // The index already holds an equivalent function in another file → dup hit.
        let conn = index_conn();
        let dup_emb = vec![1.0, 0.0, 0.0, 0.0];
        seed_chunk(&conn, "src/lib.rs", "compute", &dup, &dup_emb);
        seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &dup_emb);

        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        let validator = work
            .validators
            .iter()
            .find(|v| v.validator_name == "deduplicate")
            .expect("the deduplicate validator matched the .rs change");
        let file = validator
            .files
            .iter()
            .find(|f| f.path == "src/lib.rs")
            .expect("the changed file appears under the validator");

        // Non-empty semantic diff carrying the added function.
        assert!(
            file.semantic_diff
                .iter()
                .any(|c| c.entity_name == "compute"),
            "semantic diff should carry the added `compute`, got: {:?}",
            file.changed_symbols
        );

        // Bounded slice: includes the changed function + the import header, but
        // NOT the distant unrelated marker padded far away.
        assert!(
            file.source_slice.contains("pub fn compute"),
            "slice must include the changed function"
        );
        assert!(
            file.source_slice.contains("use std::fmt"),
            "slice must include the file header"
        );
        assert!(
            !file.source_slice.contains("distant_unrelated_marker"),
            "slice must NOT include unrelated distant code, got:\n{}",
            file.source_slice
        );

        // The duplicates probe hit at the existing file is attached.
        let dup_hit = file
            .probe_results
            .iter()
            .filter(|r| r.name == "duplicates")
            .flat_map(|r| r.rows.iter())
            .any(|row| row.file_path == "src/existing.rs");
        assert!(
            dup_hit,
            "duplicates probe_results should carry the existing.rs hit, got: {:?}",
            file.probe_results
        );
    }

    // ---- scope_review: probe dedupe --------------------------------------

    #[tokio::test]
    async fn two_validators_share_one_probe_run_for_the_same_file() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn placeholder() {}\n");
        repo.commit("initial");
        let dup = body("compute");
        repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

        let conn = index_conn();
        let dup_emb = vec![1.0, 0.0, 0.0, 0.0];
        seed_chunk(&conn, "src/lib.rs", "compute", &dup, &dup_emb);
        seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &dup_emb);

        // Two validators, both declaring `duplicates`, both matching *.rs.
        let mut loader = ValidatorLoader::new();
        loader.add_builtin_ruleset(ruleset("dedupe-a", "*.rs", &["duplicates"]));
        loader.add_builtin_ruleset(ruleset("dedupe-b", "*.rs", &["duplicates"]));
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        let results_for = |name: &str| -> Vec<ProbeResult> {
            work.validators
                .iter()
                .find(|v| v.validator_name == name)
                .and_then(|v| v.files.iter().find(|f| f.path == "src/lib.rs"))
                .map(|f| f.probe_results.clone())
                .unwrap_or_default()
        };

        let a = results_for("dedupe-a");
        let b = results_for("dedupe-b");
        assert!(!a.is_empty(), "validator A should have probe results");
        // Shared result identity: the single (file, probe) run fans out to both
        // validators byte-for-byte — proving the probe was not re-run per validator.
        assert_eq!(
            a, b,
            "both validators must receive the identical shared (file, probe) result"
        );
    }

    // ---- scope_review: symbol-targeted probes reach the work-list --------

    #[tokio::test]
    async fn symbol_targeted_probes_attach_to_the_file_bearing_the_symbol() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn placeholder() {}\n");
        repo.commit("initial");
        // The working-tree change adds `compute` to src/lib.rs.
        let added = body("compute");
        repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{added}\n"));

        let conn = index_conn();
        // `callers` evidence: an indexed inbound caller of `compute`.
        seed_symbol(&conn, "callee-1", "compute", "src/lib.rs");
        seed_symbol(&conn, "caller-1", "uses_compute", "src/caller.rs");
        seed_call_edge(&conn, "caller-1", "callee-1", "src/caller.rs", "src/lib.rs");
        // `similar` evidence: a reuse candidate in another file with the same
        // embedding as the mock embedder's constant query vector.
        let query_vec = vec![0.1_f32; DIM];
        seed_chunk(&conn, "src/util.rs", "existing_util", &added, &query_vec);

        // One validator declaring BOTH symbol-targeted probes on the .rs file.
        let loader = loader_with("reuse", "*.rs", &["callers", "similar"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        let validator = work
            .validators
            .iter()
            .find(|v| v.validator_name == "reuse")
            .expect("the reuse validator matched the .rs change");
        let file = validator
            .files
            .iter()
            .find(|f| f.path == "src/lib.rs")
            .expect("the changed file appears under the validator");

        // The `callers` result (target = symbol name `compute`) must reach the
        // file whose changed entity bears that name.
        let callers = file
            .probe_results
            .iter()
            .find(|r| r.name == "callers")
            .expect("callers result attaches to the file bearing `compute`");
        assert_eq!(callers.target, "compute");
        assert!(
            callers
                .rows
                .iter()
                .any(|row| row.file_path == "src/caller.rs"),
            "callers should carry the inbound caller, got: {:?}",
            callers.rows
        );

        // The `similar` result (also symbol-targeted) must reach the same file.
        let similar = file
            .probe_results
            .iter()
            .find(|r| r.name == "similar")
            .expect("similar result attaches to the file bearing `compute`");
        assert_eq!(similar.target, "compute");
        assert!(
            similar
                .rows
                .iter()
                .any(|row| row.file_path == "src/util.rs"),
            "similar should carry the reuse candidate, got: {:?}",
            similar.rows
        );
    }

    // ---- scope_review: change_purpose from commit message (sha) ----------

    #[tokio::test]
    async fn sha_scope_sets_change_purpose_from_commit_message() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn base() {}\n");
        repo.commit("base commit");
        repo.write("src/lib.rs", "fn base() {}\n\nfn added() {}\n");
        repo.commit("Add the added function for review");

        let conn = index_conn();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(
            Scope::Sha("HEAD~1..HEAD".to_string()),
            repo.path(),
            &loader,
            &conn,
            &embedder,
        )
        .await
        .unwrap();

        assert!(
            work.change_purpose
                .contains("Add the added function for review"),
            "change_purpose should be the commit message, got: {}",
            work.change_purpose
        );
    }

    // ---- scope_review: glob whole-content, no diff -----------------------

    #[tokio::test]
    async fn glob_scope_returns_matched_files_as_whole_content_work() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", &format!("{}\n", body("whole_file_fn")));
        repo.commit("initial");

        let conn = index_conn();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(
            Scope::Glob("src/*.rs".to_string()),
            repo.path(),
            &loader,
            &conn,
            &embedder,
        )
        .await
        .unwrap();

        let validator = work
            .validators
            .iter()
            .find(|v| v.validator_name == "deduplicate")
            .expect("validator matched the globbed .rs file");
        let file = validator
            .files
            .iter()
            .find(|f| f.path == "src/lib.rs")
            .expect("the globbed file is whole-content work");
        // Whole-content (no before side): the file's entity diffs as all-added.
        assert!(
            file.semantic_diff
                .iter()
                .any(|c| c.entity_name == "whole_file_fn"),
            "whole-content work should surface the file's entities as added"
        );
        assert!(
            file.source_slice.contains("whole_file_fn"),
            "the bounded slice should carry the whole-content function"
        );
    }

    // ---- scope_review: unmatched file yields no work ---------------------

    #[tokio::test]
    async fn unmatched_lock_file_yields_no_validator_work() {
        let repo = TestRepo::new();
        repo.write("Cargo.lock", "# lockfile\n");
        repo.commit("initial");
        repo.write("Cargo.lock", "# lockfile\nupdated = true\n");

        let conn = index_conn();
        // The only validator matches *.rs, never a .lock file.
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        assert!(
            work.validators.is_empty(),
            "a changed .lock with no matching validator yields no work, got: {:?}",
            work.validators
        );
    }
}
