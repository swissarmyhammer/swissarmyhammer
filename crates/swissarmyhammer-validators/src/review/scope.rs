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
use swissarmyhammer_sem::parser::plugins::code::is_code_file;
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

/// Maximum byte length of a changed file's source to inline **in full** in the
/// review payload.
///
/// The binding constraint is the review model's **context window**, not the
/// per-call generation cap ([`crate::validators::DEFAULT_MAX_TOKENS`] = 16 Ki,
/// which bounds the *reply*, never the input). The fan-out's primed prefix is
/// only ~5k tokens, so a context window of 32k+ tokens has ample headroom to
/// carry a typical source file whole alongside its prefix and the reply budget.
///
/// This is the inline budget expressed in **bytes** (~1 byte ≈ ¼ token for code,
/// so this corresponds to roughly the per-file slice of an ~8k-token inline
/// budget — well inside a 32k context window after the prefix and reply). Only a
/// pathologically large file relative to the window exceeds it; such a file
/// falls back to the bounded [`bounded_slice`] plus an explicit note directing
/// the model to `read_file` for the remainder. Typical source files inline whole.
const MAX_INLINE_SOURCE_BYTES: usize = 32 * 1024;

/// The synthetic validator name carried on scope-stage [`AvpError::Validator`]s.
///
/// The scope stage is not a real loaded validator, so its failures are attributed
/// to this fixed name rather than any user RuleSet.
const SCOPE_VALIDATOR: &str = "scope";

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
                validator: SCOPE_VALIDATOR.to_string(),
                message:
                    "a review scope must set exactly one of file/glob/working/sha; none were set"
                        .to_string(),
            }),
            n => Err(AvpError::Validator {
                validator: SCOPE_VALIDATOR.to_string(),
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

impl WorkList {
    /// The distinct files under review across every validator, in first-seen
    /// order, de-duplicated by path.
    ///
    /// Several validators can match the same file; this yields each file once,
    /// the first time its path appears. It is the single dedup the fan-out prime
    /// ([`render_run_prime`](crate::review::fleet::render_run_prime)) builds its
    /// file set from. First-seen order keeps the rendered prime byte-stable
    /// across calls.
    pub fn distinct_files(&self) -> impl Iterator<Item = &FileWork> {
        let mut seen = std::collections::BTreeSet::new();
        self.validators
            .iter()
            .flat_map(|validator| validator.files.iter())
            .filter(move |file| seen.insert(file.path.clone()))
    }
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
    /// The file's source for the review payload.
    ///
    /// When [`inlined_full`](FileWork::inlined_full) is `true` this is the file's
    /// **complete** current contents, so the model never needs to `read_file` the
    /// changed file. When `false` (a file too large for the inline budget,
    /// [`MAX_INLINE_SOURCE_BYTES`]) it is the bounded [`bounded_slice`] plus a note
    /// directing the model to `read_file` for the remainder.
    pub source_slice: String,
    /// Whether [`source_slice`](FileWork::source_slice) is the file's complete
    /// contents (`true`) or the bounded-slice fallback for an oversized file
    /// (`false`).
    pub inlined_full: bool,
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
        let (source_slice, inlined_full) = inline_or_slice(after, &entities);
        per_file.insert(
            file.clone(),
            FileFacts {
                changed_symbols: changed_symbols(&entities),
                source_slice,
                inlined_full,
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
                        inlined_full: facts.inlined_full,
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

    log_scope_selection(&validator_work);

    Ok(WorkList {
        change_purpose: resolved.change_purpose,
        validators: validator_work,
    })
}

/// Log the resolved review scope: an INFO summary naming the matched validators
/// and the total file count, plus a per-validator DEBUG line carrying each
/// validator's file count, declared probes, and rule names.
///
/// The summary fires even when nothing matched (reporting an empty set) so a
/// `review` run always shows what the scope stage selected; per-validator detail
/// stays at DEBUG so a default-level run sees the selection without per-rule
/// noise.
fn log_scope_selection(validators: &[ValidatorWork]) {
    let names: Vec<&str> = validators
        .iter()
        .map(|v| v.validator_name.as_str())
        .collect();
    let total_files: usize = validators.iter().map(|v| v.files.len()).sum();
    tracing::info!(
        validators = ?names,
        validator_count = validators.len(),
        files = total_files,
        "review scope resolved"
    );
    for validator in validators {
        let files: Vec<&str> = validator.files.iter().map(|f| f.path.as_str()).collect();
        tracing::debug!(
            validator = %validator.validator_name,
            files = ?files,
            probes = ?validator.probes,
            rules = ?validator.rules,
            "review scope: validator matched"
        );
    }
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
    inlined_full: bool,
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

/// Read a path's working-tree content from disk.
///
/// Returns `Ok(None)` only when the path is **absent** (the intended
/// deletion/added signal — a file gone from the working tree). Any *other*
/// failure — a permission error, or a binary/non-UTF8 file that
/// [`read_to_string`](std::fs::read_to_string) rejects — is propagated as
/// [`AvpError::Context`] rather than collapsed to `None`, so an unreadable
/// tracked file is never silently diffed as wholly added/removed.
fn read_working(repo_path: &Path, path: &str) -> Result<Option<String>, AvpError> {
    match std::fs::read_to_string(repo_path.join(path)) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AvpError::Context(format!(
            "failed to read working-tree file {path}: {e}"
        ))),
    }
}

/// Read a blob at `ref:path` via libgit2.
///
/// This is the same `git show ref:path` content read the git tool does, via the
/// shared `swissarmyhammer-git` repository handle instead of a shell-out.
///
/// Returns `Ok(None)` only when the path does **not exist** at the ref (the
/// intended Added/Deleted signal — `revparse_single` resolving to not-found, or
/// the object not being a blob). A blob that exists but cannot be read — a
/// binary/non-UTF8 tracked file, or any other libgit2 failure — is propagated as
/// [`AvpError::Context`], so an unreadable tracked file is never silently diffed
/// as wholly added/removed.
fn read_at_ref(
    repo: &GitOperations,
    refspec: &str,
    path: &str,
) -> Result<Option<String>, AvpError> {
    let inner = repo.repository().inner();
    let object = match inner.revparse_single(&format!("{refspec}:{path}")) {
        Ok(object) => object,
        // The path is absent at this ref — the intended Added/Deleted signal.
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => {
            return Err(AvpError::Context(format!(
                "failed to resolve {refspec}:{path}: {e}"
            )))
        }
    };
    // Not a blob (e.g. a tree at that path) — there is no file content to read.
    let Some(blob) = object.as_blob() else {
        return Ok(None);
    };
    String::from_utf8(blob.content().to_vec())
        .map(Some)
        .map_err(|e| AvpError::Context(format!("blob {refspec}:{path} is not valid UTF-8: {e}")))
}

/// Resolve the working-tree scope: uncommitted changes vs HEAD (staged +
/// unstaged + untracked), reusing the git tool's changed-file accounting.
fn resolve_working(repo_path: &Path) -> Result<ResolvedScope, AvpError> {
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
        let after = after_by_path.get(path).cloned().unwrap_or(None);
        let before = read_at_ref(&repo, "HEAD", path)?;
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
        let before = read_at_ref(&repo, &from_ref, path)?;
        let after = read_at_ref(&repo, &to_ref, path)?;
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
    let after = read_working(repo_path, path)?;
    let before = read_at_ref(&repo, "HEAD", path)?;

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
        let after = read_working(repo_path, path)?;
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

/// The note appended to the bounded-slice fallback, directing the model to read
/// the rest of an oversized changed file rather than reasoning from the slice
/// alone. Names `read_file` explicitly so the model knows which tool to reach for.
const OVERSIZED_FILE_READ_NOTE: &str =
    "\n\n// NOTE: this file is too large to inline in full; the slice above is bounded. \
Use `read_file` on this path to see the remainder before reasoning about it.";

/// Resolve a changed file's review source, returning the source text and whether
/// it is the file's **complete** contents.
///
/// The model re-reads any file it is not given in full, and those tool
/// round-trips dominate review wall-clock — so the changed file is inlined whole
/// whenever it fits the inline budget ([`MAX_INLINE_SOURCE_BYTES`], keyed off the
/// model's context window, NOT the generation cap). Returns `(full_source, true)`
/// in that common case.
///
/// Only a pathologically large file relative to the window exceeds the budget; it
/// falls back to the bounded [`bounded_slice`] plus [`OVERSIZED_FILE_READ_NOTE`]
/// and returns `(slice_with_note, false)` so the caller frames it as a partial
/// view the model should `read_file` to complete.
fn inline_or_slice(after: Option<&str>, entities: &[SemanticChange]) -> (String, bool) {
    match after {
        Some(content) if content.len() <= MAX_INLINE_SOURCE_BYTES => (content.to_string(), true),
        _ => {
            let mut slice = bounded_slice(after, entities);
            slice.push_str(OVERSIZED_FILE_READ_NOTE);
            (slice, false)
        }
    }
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

    use model_embedding::mock::MockEmbedder;

    use crate::review::test_support::{
        body, dup_emb, index_conn, loader_with, ruleset, seed_call_edge, seed_chunk, seed_symbol,
        TestRepo, DIM,
    };
    use crate::validators::ValidatorLoader;

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
    async fn working_scope_groups_duplicate_under_validator_with_full_source() {
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
        let emb = dup_emb();
        seed_chunk(&conn, "src/lib.rs", "compute", &dup, &emb);
        seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &emb);

        let loader = loader_with("deduplicate", "*.rs", &["duplicates"], Severity::Warn);
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

        // Full source: a small changed file is inlined whole, so the model never
        // re-reads it. The changed function, the header, AND the distant unrelated
        // marker (which the old bounded slice trimmed) are all present.
        assert!(file.inlined_full, "a small changed file inlines in full");
        assert!(
            file.source_slice.contains("pub fn compute"),
            "full source must include the changed function"
        );
        assert!(
            file.source_slice.contains("use std::fmt"),
            "full source must include the file header"
        );
        assert!(
            file.source_slice.contains("distant_unrelated_marker"),
            "the full inline carries even distant code, got:\n{}",
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

    // ---- scope_review: untracked files in the working scope ---------------

    #[tokio::test]
    async fn working_scope_includes_untracked_nested_source_files() {
        let repo = TestRepo::new();
        repo.write("README.md", "# base\n");
        repo.commit("initial");

        // A brand-new untracked directory of source files — the calcutron shape.
        repo.write("src/new.rs", &format!("{}\n", body("brand_new")));

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[], Severity::Warn);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        let validator = work
            .validators
            .iter()
            .find(|v| v.validator_name == "rust")
            .expect("the rust validator must match the untracked .rs file");
        assert!(
            validator.files.iter().any(|f| f.path == "src/new.rs"),
            "the untracked nested source file must be in scope, got: {:?}",
            validator
                .files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn working_scope_excludes_untracked_non_code_files() {
        let repo = TestRepo::new();
        repo.write("README.md", "# base\n");
        repo.commit("initial");

        // Untracked junk: a log file in a new directory. Even a match-everything
        // validator must never see it — the code-extension filter drops it
        // before matching, so its content is never read.
        repo.write("logs/run.log", "lots of noise\n");

        let conn = index_conn();
        let loader = loader_with("everything", "*", &[], Severity::Warn);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        assert!(
            work.validators.is_empty(),
            "untracked non-code files must not enter the working scope, got: {:?}",
            work.validators
                .iter()
                .flat_map(|v| v.files.iter().map(|f| f.path.as_str()))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn working_scope_keeps_tracked_non_code_modifications() {
        let repo = TestRepo::new();
        repo.write("notes.txt", "original\n");
        repo.commit("initial");

        // A deliberate edit to a tracked non-code file keeps current behavior:
        // it stays in scope and per-validator globs decide whether it's reviewed.
        repo.write("notes.txt", "original\nedited\n");

        let conn = index_conn();
        let loader = loader_with("everything", "*", &[], Severity::Warn);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        let validator = work
            .validators
            .iter()
            .find(|v| v.validator_name == "everything")
            .expect("tracked modifications must stay in scope regardless of extension");
        assert!(
            validator.files.iter().any(|f| f.path == "notes.txt"),
            "the tracked modified file must be in scope, got: {:?}",
            validator
                .files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>()
        );
    }

    // ---- WorkList::distinct_files ----------------------------------------

    /// A minimal `FileWork` carrying only a path — enough to assert the
    /// dedup/order semantics of [`WorkList::distinct_files`].
    fn file_at(path: &str) -> FileWork {
        FileWork {
            path: path.to_string(),
            semantic_diff: vec![],
            changed_symbols: vec![],
            source_slice: String::new(),
            inlined_full: true,
            probe_results: vec![],
        }
    }

    fn validator_over(name: &str, paths: &[&str]) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            severity: Severity::Warn,
            rules: vec![],
            probes: vec![],
            files: paths.iter().map(|p| file_at(p)).collect(),
        }
    }

    #[test]
    fn distinct_files_dedups_by_path_in_first_seen_order() {
        // Three validators; `src/shared.rs` is matched by two of them, and the
        // overall first-seen order is b, shared, a.
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![
                validator_over("v1", &["src/b.rs", "src/shared.rs"]),
                validator_over("v2", &["src/shared.rs", "src/a.rs"]),
            ],
        };

        let distinct: Vec<&str> = work.distinct_files().map(|f| f.path.as_str()).collect();
        assert_eq!(
            distinct,
            vec!["src/b.rs", "src/shared.rs", "src/a.rs"],
            "distinct_files dedups by path and preserves first-seen order"
        );
    }

    // ---- scope_review: observability tracing -----------------------------

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn scope_review_logs_the_selected_validators_and_their_rules() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn placeholder() {}\n");
        repo.commit("initial");
        let dup = body("compute");
        repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

        let conn = index_conn();
        let emb = dup_emb();
        seed_chunk(&conn, "src/lib.rs", "compute", &dup, &emb);

        let loader = loader_with("deduplicate", "*.rs", &["duplicates"], Severity::Warn);
        let embedder = MockEmbedder::new(DIM);

        let _work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        // The selection summary names the matched validator and file count.
        assert!(logs_contain("review scope resolved"));
        assert!(logs_contain("validators=[\"deduplicate\"]"));
        // The per-validator detail line names the validator, its files, and its
        // declared probes/rules.
        assert!(logs_contain("validator=deduplicate"));
        assert!(logs_contain("deduplicate-rule"));
        assert!(logs_contain("duplicates"));
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn scope_review_logs_a_summary_even_when_nothing_matches() {
        let repo = TestRepo::new();
        repo.write("Cargo.lock", "# lockfile\n");
        repo.commit("initial");
        repo.write("Cargo.lock", "# lockfile\nupdated = true\n");

        let conn = index_conn();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"], Severity::Warn);
        let embedder = MockEmbedder::new(DIM);

        let _work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        // The summary still fires, reporting zero matched validators.
        assert!(logs_contain("review scope resolved"));
        assert!(logs_contain("validators=[]"));
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
        let emb = dup_emb();
        seed_chunk(&conn, "src/lib.rs", "compute", &dup, &emb);
        seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &emb);

        // Baseline: ONE validator declaring `duplicates` drives the embedder a
        // fixed number of times for this change set. The embedder call count is
        // the probe runner's observable execution count — a re-run repeats the
        // changed-set embedding work.
        let baseline_embedder = MockEmbedder::new(DIM);
        let single = loader_with("dedupe-a", "*.rs", &["duplicates"], Severity::Warn);
        scope_review(
            Scope::Working,
            repo.path(),
            &single,
            &conn,
            &baseline_embedder,
        )
        .await
        .unwrap();
        let baseline = baseline_embedder.call_count();
        assert!(baseline > 0, "the duplicates probe must drive the embedder");

        // Two validators, both declaring `duplicates`, both matching *.rs.
        let mut loader = ValidatorLoader::new();
        loader.add_builtin_ruleset(ruleset("dedupe-a", "*.rs", &["duplicates"], Severity::Warn));
        loader.add_builtin_ruleset(ruleset("dedupe-b", "*.rs", &["duplicates"], Severity::Warn));
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder)
            .await
            .unwrap();

        // Execution count: the shared (file, probe) run embeds exactly as often
        // as the single-validator baseline — a per-validator re-run would
        // multiply it.
        assert_eq!(
            embedder.call_count(),
            baseline,
            "two validators declaring the same probe must not re-run it"
        );

        let results_for = |name: &str| -> Vec<ProbeResult> {
            work.validators
                .iter()
                .find(|v| v.validator_name == name)
                .and_then(|v| v.files.iter().find(|f| f.path == "src/lib.rs"))
                .map(|f| f.probe_results.clone())
                .unwrap_or_default()
        };

        // Secondary check: the single shared run's result fans out to both
        // validators byte-for-byte.
        let a = results_for("dedupe-a");
        let b = results_for("dedupe-b");
        assert!(!a.is_empty(), "validator A should have probe results");
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
        let loader = loader_with("reuse", "*.rs", &["callers", "similar"], Severity::Warn);
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
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"], Severity::Warn);
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
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"], Severity::Warn);
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
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"], Severity::Warn);
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

    // ---- inline_or_slice: full source under the cap, bounded fallback over ----

    /// A small changed file inlines its COMPLETE source (every line, including
    /// ones the bounded slice would have trimmed) and reports `inlined_full`.
    #[test]
    fn small_file_inlines_full_source_and_reports_inlined_full() {
        // A distant marker far from the header and the changed hunk — exactly
        // what `bounded_slice` trims away, so its presence proves the FULL file
        // is inlined, not the slice.
        let mid_padding: String = (0..30).map(|i| format!("// mid {i}\n")).collect();
        let after = format!(
            "use std::fmt;\n{mid_padding}fn distant_unrelated_marker() {{}}\npub fn compute() {{}}\n"
        );
        let entities = vec![added_entity("compute", "pub fn compute() {}")];

        let (source, inlined_full) = inline_or_slice(Some(&after), &entities);

        assert!(inlined_full, "a small file must inline in full");
        assert!(
            source.contains("distant_unrelated_marker"),
            "the full inline must carry the distant line the bounded slice trims, got:\n{source}"
        );
        assert!(
            source.contains("pub fn compute"),
            "the full inline must carry the changed function, got:\n{source}"
        );
    }

    /// A changed file whose full content exceeds [`MAX_INLINE_SOURCE_BYTES`]
    /// falls back to the bounded slice plus an explicit read-the-rest note, and
    /// reports `inlined_full == false`.
    #[test]
    fn oversized_file_falls_back_to_bounded_slice_with_read_note() {
        // A short header (imports), then a marker placed in the MIDDLE of a huge
        // body — outside both the header window (`HEADER_LINES`) and the changed
        // hunk — so the bounded slice trims it. Its absence proves the fallback
        // is the slice, not the whole file.
        let header = "use std::fmt;\n";
        // Many newline-separated lines so the marker sits far past the header
        // window (line > HEADER_LINES) and far from the changed hunk, and the
        // whole file still blows past the inline byte cap.
        let lead: String = (0..MAX_INLINE_SOURCE_BYTES)
            .map(|i| format!("// lead {i}\n"))
            .collect();
        let trail: String = (0..MAX_INLINE_SOURCE_BYTES)
            .map(|i| format!("// trail {i}\n"))
            .collect();
        let after = format!(
            "{header}{lead}fn distant_unrelated_marker() {{}}\n{trail}pub fn compute() {{}}\n"
        );
        assert!(
            after.len() > MAX_INLINE_SOURCE_BYTES,
            "fixture must exceed the inline cap"
        );
        let entities = vec![added_entity("compute", "pub fn compute() {}")];

        let (source, inlined_full) = inline_or_slice(Some(&after), &entities);

        assert!(!inlined_full, "an oversized file must NOT inline in full");
        assert!(
            source.contains("pub fn compute"),
            "the fallback slice must still carry the changed function, got:\n{source}"
        );
        assert!(
            !source.contains("distant_unrelated_marker"),
            "the fallback must be the bounded slice, not the whole file, got:\n{source}"
        );
        assert!(
            source.contains("read_file"),
            "the fallback must direct the model to read_file for the remainder, got:\n{source}"
        );
    }

    // ---- read_working / read_at_ref error discipline --------------------

    /// A non-UTF8 byte sequence: a lone continuation byte that is invalid as
    /// the start of a UTF-8 sequence, so `read_to_string` / `from_utf8` reject
    /// it. Models a binary/unreadable tracked blob.
    const BINARY_BYTES: &[u8] = &[0xff, 0xfe, 0x00, 0x01];

    /// An absent working-tree path resolves to `Ok(None)` — the intended
    /// deletion signal — not an error.
    #[test]
    fn read_working_maps_an_absent_path_to_ok_none() {
        let repo = TestRepo::new();
        let got = read_working(repo.path(), "src/does_not_exist.rs")
            .expect("an absent path must not be an error");
        assert_eq!(got, None, "an absent path is the deletion signal: Ok(None)");
    }

    /// A present, readable working-tree path resolves to `Ok(Some(content))`.
    #[test]
    fn read_working_reads_a_present_file() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "pub fn compute() {}\n");
        let got = read_working(repo.path(), "src/lib.rs").expect("a readable file must succeed");
        assert_eq!(got.as_deref(), Some("pub fn compute() {}\n"));
    }

    /// A binary/non-UTF8 working-tree file is a genuine read failure, NOT the
    /// deletion signal — it must surface as an error, never as `Ok(None)`.
    #[test]
    fn read_working_propagates_a_non_utf8_file_as_an_error() {
        let repo = TestRepo::new();
        std::fs::write(repo.path().join("blob.bin"), BINARY_BYTES).unwrap();

        let err = read_working(repo.path(), "blob.bin")
            .expect_err("a non-UTF8 file must not be silently treated as absent");
        match err {
            AvpError::Context(msg) => {
                assert!(
                    msg.contains("blob.bin"),
                    "the error must name the path: {msg}"
                );
            }
            other => panic!("expected AvpError::Context, got: {other:?}"),
        }
    }

    /// A path absent at the requested ref resolves to `Ok(None)`.
    #[test]
    fn read_at_ref_maps_a_path_absent_at_ref_to_ok_none() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "pub fn compute() {}\n");
        repo.commit("initial");
        let git = open_repo(repo.path()).unwrap();

        let got = read_at_ref(&git, "HEAD", "src/never_committed.rs")
            .expect("a path absent at the ref must not be an error");
        assert_eq!(got, None, "absent at the ref is Ok(None)");
    }

    /// A blob present at the ref resolves to `Ok(Some(content))`.
    #[test]
    fn read_at_ref_reads_a_committed_blob() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "pub fn compute() {}\n");
        repo.commit("initial");
        let git = open_repo(repo.path()).unwrap();

        let got = read_at_ref(&git, "HEAD", "src/lib.rs").expect("a committed blob must succeed");
        assert_eq!(got.as_deref(), Some("pub fn compute() {}\n"));
    }

    /// A binary/non-UTF8 blob committed at the ref is a genuine read failure,
    /// NOT a missing-path signal — it must surface as an error so the file is
    /// never silently diffed as wholly added/removed.
    #[test]
    fn read_at_ref_propagates_a_non_utf8_blob_as_an_error() {
        let repo = TestRepo::new();
        std::fs::write(repo.path().join("blob.bin"), BINARY_BYTES).unwrap();
        repo.commit("add a binary blob");
        let git = open_repo(repo.path()).unwrap();

        let err = read_at_ref(&git, "HEAD", "blob.bin")
            .expect_err("a non-UTF8 blob must not be silently treated as absent");
        match err {
            AvpError::Context(msg) => {
                assert!(
                    msg.contains("blob.bin"),
                    "the error must name the path: {msg}"
                );
            }
            other => panic!("expected AvpError::Context, got: {other:?}"),
        }
    }

    /// A small `SemanticChange` carrying just an added entity's body, enough to
    /// drive the bounded-slice path in the helper tests.
    fn added_entity(name: &str, body: &str) -> SemanticChange {
        use swissarmyhammer_sem::model::change::ChangeType;
        SemanticChange {
            id: format!("test:{name}"),
            entity_id: name.to_string(),
            change_type: ChangeType::Added,
            entity_type: "function".to_string(),
            entity_name: name.to_string(),
            file_path: "src/lib.rs".to_string(),
            old_file_path: None,
            before_content: None,
            after_content: Some(body.to_string()),
            commit_sha: None,
            author: None,
            timestamp: None,
            structural_change: None,
        }
    }
}
