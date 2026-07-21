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
use crate::review::fleet::{emit_progress, ReviewProgressEvent, ReviewProgressSender};
use crate::review::probes::{run_probes, ChangeEntry, FileChange as ProbeChange, ProbeResult};
use crate::validators::{MatchContext, RuleSet, ValidatorLoader};

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
    /// The file's **complete** current source, inlined in full into the review
    /// payload so the model never needs to `read_file` the changed file.
    ///
    /// A changed file is always inlined whole: it is the file's complete current
    /// contents (empty only for a deletion, which has no current content — the
    /// removal is carried by [`semantic_diff`](FileWork::semantic_diff)). A file
    /// whose source would exceed the review `batch_size` is never trimmed to a
    /// slice; [`batch_work_list`] rejects it with a hard error instead, so this is
    /// never a partial view.
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
/// `embedder` embeds probe query bodies. `progress` is the optional review
/// progress sender: when wired, one
/// [`ReviewProgressEvent::FileScoped`] is emitted per resolved file BEFORE the
/// semantic-diff + probe pass, so a consumer sees the run's first events within
/// seconds of the call starting; `None` emits nothing.
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
    progress: Option<&ReviewProgressSender>,
) -> Result<WorkList, AvpError> {
    let resolved = resolve_scope_files(&scope, repo_path)?;

    // Announce every resolved file BEFORE the semantic-diff + probe pass —
    // these are the run's FIRST progress events, emitted within seconds of the
    // call starting. The diff and probes below run over the whole set in one
    // pass, which on a large scope can be silent for a long time; a progress
    // consumer keeps the client alive through that stretch by re-sending its
    // latest param, and these events are what give it one.
    for file in &resolved.files {
        emit_progress(
            progress,
            ReviewProgressEvent::FileScoped { file: file.clone() },
        );
    }

    // The single semantic-diff pass: one `FileChange` per resolved file fed to
    // the sem differ once. Whole-content files (glob / unchanged single file)
    // carry only `after_content`, so they diff as all-added entities.
    let registry = create_default_registry();
    let diff = compute_semantic_diff(&resolved.file_changes, &registry, None, None);

    // Group the diff's entities by file, and derive the probe change-set (every
    // changed entity across the whole diff) so probes run over the real diff.
    let grouped = group_entities_by_file(diff.changes);

    // Match validators per file via the shared `matching_rulesets` code path.
    let matched = match_validators_and_files(&resolved.files, loader);

    // Run probes ONCE over the whole change set with the union of every declared
    // probe name. This is the N+M guarantee: each distinct `(file, probe)` is
    // computed exactly once and the shared result fans out to every validator
    // that declared it (the distribution below is a pure filter, never a re-run).
    let probe_cache = run_probe_cache(
        &matched.validators,
        &grouped.change_entities,
        conn,
        embedder,
    )
    .await?;

    // Pre-compute the bounded slice + changed symbols per file once (shared by
    // every validator that reviews the same file).
    let per_file = compute_per_file_facts(
        &matched.matched_files,
        &grouped.entities_by_file,
        &resolved.after_content,
    );

    // Assemble the work-list: name-sorted validators, each carrying their matched
    // files (path-sorted) with the shared facts + their probe subset.
    let validator_work = assemble_validator_work(matched.validators, &per_file, &probe_cache);

    log_scope_selection(&validator_work);

    Ok(WorkList {
        change_purpose: resolved.change_purpose,
        validators: validator_work,
    })
}

/// The semantic diff's entities, grouped by file, plus the flattened probe
/// change-set — the two views [`scope_review`] needs from one pass over the diff.
struct GroupedEntities {
    /// One file path → its changed entities, the input to the per-file facts.
    entities_by_file: BTreeMap<String, Vec<SemanticChange>>,
    /// Every changed entity across the whole diff, as probe-runner inputs.
    change_entities: Vec<ChangeEntry>,
}

/// Group the semantic diff's changes by file path, while flattening every changed
/// entity into the probe runner's change-set so probes run over the real diff.
fn group_entities_by_file(changes: Vec<SemanticChange>) -> GroupedEntities {
    let mut entities_by_file: BTreeMap<String, Vec<SemanticChange>> = BTreeMap::new();
    let mut change_entities: Vec<ChangeEntry> = Vec::new();
    for change in changes {
        change_entities.push(to_probe_entry(&change));
        entities_by_file
            .entry(change.file_path.clone())
            .or_default()
            .push(change);
    }
    GroupedEntities {
        entities_by_file,
        change_entities,
    }
}

/// The validators matched against the scope's files, plus the set of files at
/// least one validator matched.
struct MatchedValidators {
    /// Files that at least one validator matched (the per-file-facts key set).
    matched_files: BTreeSet<String>,
    /// Validator name → its accumulated match (rules, probes, files).
    validators: BTreeMap<String, MatchedValidator>,
}

/// Match every resolved file against the loader's validators via the shared
/// `matching_rulesets` code path, accumulating each validator's matched files.
fn match_validators_and_files(files: &[String], loader: &ValidatorLoader) -> MatchedValidators {
    let mut matched_files: BTreeSet<String> = BTreeSet::new();
    let mut validators: BTreeMap<String, MatchedValidator> = BTreeMap::new();
    for file in files {
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
    MatchedValidators {
        matched_files,
        validators,
    }
}

/// Pre-compute the [`FileFacts`] (full inlined source, changed symbols, semantic
/// diff) once per matched file — shared by every validator that reviews that file.
fn compute_per_file_facts(
    matched_files: &BTreeSet<String>,
    entities_by_file: &BTreeMap<String, Vec<SemanticChange>>,
    after_content: &BTreeMap<String, String>,
) -> BTreeMap<String, FileFacts> {
    let mut per_file: BTreeMap<String, FileFacts> = BTreeMap::new();
    for file in matched_files {
        let entities = entities_by_file.get(file).cloned().unwrap_or_default();
        // The changed file is always inlined in FULL: the model re-reads any file
        // it is not given whole, and those round-trips dominate review wall-clock.
        // A deletion has no current content, so its source is empty (the removal
        // is carried by the semantic diff). A file too large for the review
        // `batch_size` is never trimmed here — [`batch_work_list`] rejects it.
        let source_slice = after_content.get(file).cloned().unwrap_or_default();
        per_file.insert(
            file.clone(),
            FileFacts {
                changed_symbols: changed_symbols(&entities),
                source_slice,
                semantic_diff: entities,
            },
        );
    }
    per_file
}

/// Assemble the final work-list: name-sorted validators, each carrying their
/// matched files (path-sorted) with the shared per-file facts and the validator's
/// probe subset selected from the shared `probe_cache`.
fn assemble_validator_work(
    validators: BTreeMap<String, MatchedValidator>,
    per_file: &BTreeMap<String, FileFacts>,
    probe_cache: &[ProbeResult],
) -> Vec<ValidatorWork> {
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
                            probe_cache,
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
                rules: mv.rules,
                probes: mv.probes,
                files,
            }
        })
        .collect();
    validator_work.sort_by(|a, b| a.validator_name.cmp(&b.validator_name));
    validator_work
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
    rules: Vec<String>,
    probes: Vec<String>,
    files: BTreeSet<String>,
}

impl MatchedValidator {
    fn from_ruleset(rs: &RuleSet) -> Self {
        Self {
            name: rs.name().to_string(),
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

/// Split a [`WorkList`] into content-budgeted batches at **whole-file**
/// granularity, so every batch's primed prefix stays inside `batch_size` bytes.
///
/// Cramming every changed file's full source into one shared prime overflows the
/// review model's context on a large diff — every fan-out validator then fails
/// uniformly. So the run is split into batches and each batch fans out
/// independently. The files are packed greedily, in [`WorkList::distinct_files`]
/// order (the same order the prime renders them): each file is added to the
/// current batch until adding the next file's inlined source would push the batch
/// past `batch_size`, at which point a new batch starts. A file is **atomic** — it
/// is never split across batches.
///
/// Each returned [`WorkList`] carries every validator that has at least one file
/// in that batch, with the validator's files filtered to the batch (validators
/// left with no files in a batch are dropped). The change purpose is carried
/// verbatim so every batch's prime frames the same overall change. A work-list
/// with no files (no validator matched) yields no batches.
///
/// # Errors
///
/// Returns [`AvpError::Validator`] when a single file's inlined source alone
/// exceeds `batch_size`: it cannot be packed without either splitting it
/// (forbidden) or blowing the budget. The error names the file, its byte size, and
/// the limit, and directs the caller to raise `batch_size` or narrow the scope.
/// This is the loud replacement for the old silent slice-degrade of an oversized
/// file.
pub fn batch_work_list(work: &WorkList, batch_size: usize) -> Result<Vec<WorkList>, AvpError> {
    // Pack the distinct files (first-seen order, matching the prime's file set)
    // into byte-budgeted batches; a file is never split across a batch boundary.
    let mut batches: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_bytes = 0usize;
    for file in work.distinct_files() {
        let size = file.source_slice.len();
        if size > batch_size {
            return Err(AvpError::Validator {
                validator: SCOPE_VALIDATOR.to_string(),
                message: format!(
                    "file `{}` inlines {size} bytes, over the {batch_size}-byte review batch_size; \
                     a file is never split across review batches — raise `batch_size` or narrow the review scope",
                    file.path
                ),
            });
        }
        if !current.is_empty() && current_bytes + size > batch_size {
            batches.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current.push(file.path.clone());
        current_bytes += size;
    }
    if !current.is_empty() {
        batches.push(current);
    }

    Ok(batches
        .into_iter()
        .map(|paths| project_onto_files(work, &paths))
        .collect())
}

/// Project a [`WorkList`] onto a subset of file paths: keep every validator that
/// has at least one file in `paths`, with its files filtered to `paths` (order
/// preserved). Validators left with no files are dropped. The change purpose is
/// carried verbatim so the batch's prime still frames the whole change.
fn project_onto_files(work: &WorkList, paths: &[String]) -> WorkList {
    let keep: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
    let validators = work
        .validators
        .iter()
        .filter_map(|validator| {
            let files: Vec<FileWork> = validator
                .files
                .iter()
                .filter(|file| keep.contains(file.path.as_str()))
                .cloned()
                .collect();
            if files.is_empty() {
                return None;
            }
            Some(ValidatorWork {
                validator_name: validator.validator_name.clone(),
                rules: validator.rules.clone(),
                probes: validator.probes.clone(),
                files,
            })
        })
        .collect();
    WorkList {
        change_purpose: work.change_purpose.clone(),
        validators,
    }
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

    // ---- scope_review: scope-phase progress events -------------------------

    #[tokio::test]
    async fn scope_review_emits_one_file_scoped_event_per_resolved_file() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "pub fn base() {}\n");
        repo.commit("initial");
        // Two changed files in the working tree — the multi-file scope.
        repo.write("src/alpha.rs", &format!("{}\n", body("alpha")));
        repo.write("src/beta.rs", &format!("{}\n", body("beta")));

        let conn = index_conn();
        let loader = loader_with("scoped", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        scope_review(
            Scope::Working,
            repo.path(),
            &loader,
            &conn,
            &embedder,
            Some(&tx),
        )
        .await
        .unwrap();
        drop(tx);

        // The scope stage announces each resolved file exactly once — its
        // events are the run's FIRST progress (they exist before any fleet
        // work), so their emission from `scope_review` itself is the contract.
        let mut scoped_files = Vec::new();
        while let Some(event) = rx.recv().await {
            match event {
                ReviewProgressEvent::FileScoped { file } => scoped_files.push(file),
                other => panic!("the scope stage emits only FileScoped events, got: {other:?}"),
            }
        }
        scoped_files.sort();
        assert_eq!(
            scoped_files,
            vec!["src/alpha.rs".to_string(), "src/beta.rs".to_string()],
            "one FileScoped event per resolved file"
        );
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

        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
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

        // Full source: a changed file is always inlined whole, so the model never
        // re-reads it. The changed function, the header, AND a distant unrelated
        // marker are all present (nothing is trimmed to a slice).
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
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
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
        let loader = loader_with("everything", "*", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
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
        let loader = loader_with("everything", "*", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
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
        file_sized(path, 0)
    }

    /// A `FileWork` whose inlined `source_slice` is exactly `bytes` bytes — the
    /// knob [`batch_work_list`] packs against.
    fn file_sized(path: &str, bytes: usize) -> FileWork {
        FileWork {
            path: path.to_string(),
            semantic_diff: vec![],
            changed_symbols: vec![],
            source_slice: "x".repeat(bytes),
            probe_results: vec![],
        }
    }

    fn validator_over(name: &str, paths: &[&str]) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            rules: vec![format!("{name}-rule")],
            probes: vec![],
            files: paths.iter().map(|p| file_at(p)).collect(),
        }
    }

    /// A validator over `(path, byte-size)` files, for [`batch_work_list`] packing
    /// assertions.
    fn validator_sized(name: &str, files: &[(&str, usize)]) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            rules: vec![format!("{name}-rule")],
            probes: vec![],
            files: files.iter().map(|(p, n)| file_sized(p, *n)).collect(),
        }
    }

    /// The validator names a batch carries, in order.
    fn batch_validators(batch: &WorkList) -> Vec<String> {
        batch
            .validators
            .iter()
            .map(|v| v.validator_name.clone())
            .collect()
    }

    /// The file paths a batch carries (distinct, prime order).
    fn batch_paths(batch: &WorkList) -> Vec<String> {
        batch.distinct_files().map(|f| f.path.clone()).collect()
    }

    #[test]
    fn batch_work_list_packs_whole_files_within_the_byte_budget() {
        // Three 10-byte files, budget 25 → greedy packing gives [a,b],[c]; the
        // running total never exceeds the budget and no file is split.
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![validator_sized(
                "v",
                &[("a.rs", 10), ("b.rs", 10), ("c.rs", 10)],
            )],
        };

        let batches = batch_work_list(&work, 25).expect("packs within budget");

        assert_eq!(
            batches.iter().map(batch_paths).collect::<Vec<_>>(),
            vec![vec!["a.rs", "b.rs"], vec!["c.rs"]],
            "files pack greedily into whole-file batches under the budget"
        );
        for batch in &batches {
            let total: usize = batch.distinct_files().map(|f| f.source_slice.len()).sum();
            assert!(total <= 25, "every batch stays within the byte budget");
        }
    }

    #[test]
    fn batch_work_list_errors_on_a_single_file_over_the_budget() {
        // One file larger than the budget cannot be packed without splitting it
        // (forbidden) — it is a hard error, not a slice, not a spill.
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![validator_sized("v", &[("big.rs", 100)])],
        };

        let err = batch_work_list(&work, 32).expect_err("an oversized file errors");
        let msg = err.to_string();
        assert!(msg.contains("big.rs"), "names the offending file: {msg}");
        assert!(msg.contains("100"), "names the file's size: {msg}");
        assert!(msg.contains("32"), "names the limit: {msg}");
        assert!(
            msg.contains("batch_size") && msg.contains("narrow"),
            "directs the caller to raise batch_size or narrow scope: {msg}"
        );
    }

    #[test]
    fn batch_work_list_small_diff_is_exactly_one_batch() {
        // Today's fast path: a small diff fits one batch, unchanged.
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![validator_sized("v", &[("a.rs", 10), ("b.rs", 10)])],
        };

        let batches = batch_work_list(&work, 32 * 1024).expect("small diff packs");

        assert_eq!(batches.len(), 1, "a small diff is a single batch");
        assert_eq!(batch_paths(&batches[0]), vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn batch_work_list_projects_each_validator_onto_its_batch_files() {
        // v1 owns a.rs,b.rs; v2 owns c.rs. Budget 25 splits into [a,b],[c], so v1
        // lands wholly in batch 1 and v2 wholly in batch 2 — a validator with no
        // files in a batch is dropped from it.
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![
                validator_sized("v1", &[("a.rs", 10), ("b.rs", 10)]),
                validator_sized("v2", &[("c.rs", 10)]),
            ],
        };

        let batches = batch_work_list(&work, 25).expect("packs within budget");

        assert_eq!(batches.len(), 2);
        assert_eq!(batch_validators(&batches[0]), vec!["v1"]);
        assert_eq!(batch_paths(&batches[0]), vec!["a.rs", "b.rs"]);
        assert_eq!(batch_validators(&batches[1]), vec!["v2"]);
        assert_eq!(batch_paths(&batches[1]), vec!["c.rs"]);
    }

    #[test]
    fn batch_work_list_keeps_a_shared_file_atomic_in_one_batch() {
        // `shared.rs` is matched by two validators but is ONE distinct file: it is
        // packed once, into a single batch, never duplicated or split.
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![
                validator_sized("v1", &[("shared.rs", 10)]),
                validator_sized("v2", &[("shared.rs", 10)]),
            ],
        };

        let batches = batch_work_list(&work, 25).expect("packs within budget");

        assert_eq!(batches.len(), 1, "the one distinct file is one batch");
        assert_eq!(batch_paths(&batches[0]), vec!["shared.rs"]);
        assert_eq!(
            batch_validators(&batches[0]),
            vec!["v1", "v2"],
            "both validators that matched the shared file ride the same batch"
        );
    }

    #[test]
    fn batch_work_list_empty_work_yields_no_batches() {
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![],
        };
        assert!(batch_work_list(&work, 32 * 1024).unwrap().is_empty());
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

        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let _work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
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
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let _work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
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
        let single = loader_with("dedupe-a", "*.rs", &["duplicates"]);
        scope_review(
            Scope::Working,
            repo.path(),
            &single,
            &conn,
            &baseline_embedder,
            None,
        )
        .await
        .unwrap();
        let baseline = baseline_embedder.call_count();
        assert!(baseline > 0, "the duplicates probe must drive the embedder");

        // Two validators, both declaring `duplicates`, both matching *.rs.
        let mut loader = ValidatorLoader::new();
        loader.add_builtin_ruleset(ruleset("dedupe-a", "*.rs", &["duplicates"]));
        loader.add_builtin_ruleset(ruleset("dedupe-b", "*.rs", &["duplicates"]));
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
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
        let loader = loader_with("reuse", "*.rs", &["callers", "similar"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
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
            None,
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
            None,
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

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();

        assert!(
            work.validators.is_empty(),
            "a changed .lock with no matching validator yields no work, got: {:?}",
            work.validators
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
}
