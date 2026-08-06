//! Engine stage 1 — resolve a review scope into a per-validator work-list.
//!
//! This is the first, deterministic, LLM-free stage of the review pipeline. Given
//! a [`Scope`] (exactly one of `working` / `sha` / `file` / `glob`) it produces a
//! [`WorkList`]: the review-level [change purpose](WorkList::change_purpose()) plus,
//! per matched validator, the files to review. The **validator is the shard; the
//! file is the grain** — each [`FileWork`] carries that file's structured semantic
//! diff, the changed symbols, a *bounded* [`source_slice`](FileWork::source_slice())
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
use std::path::{Path, PathBuf};

use model_embedding::TextEmbedder;
use rusqlite::Connection;
use serde::Serialize;

use swissarmyhammer_git::{GitOperations, LineBlame};
use swissarmyhammer_sem::git_types::{FileChange as SemFileChange, FileStatus};
use swissarmyhammer_sem::model::change::SemanticChange;
use swissarmyhammer_sem::parser::differ::compute_semantic_diff;
use swissarmyhammer_sem::parser::plugins::code::is_code_file;
use swissarmyhammer_sem::parser::plugins::create_default_registry;

use ::ignore::gitignore::Gitignore;

use swissarmyhammer_project_detection::{detect_projects, spec_for};

use crate::error::AvpError;
use crate::review::fleet::{emit_progress, ReviewProgressEvent, ReviewProgressSender};
use crate::review::ignore::{
    ensure_reviewignore, load_review_ignore_matcher, review_ignore_reason,
};
use crate::review::probes::{run_probes, ChangeEntry, FileChange as ProbeChange, ProbeResult};
use crate::validators::{MatchContext, RuleSet, ValidatorLoader};

/// The synthetic validator name carried on scope-stage [`AvpError::Validator`]s.
///
/// The scope stage is not a real loaded validator, so its failures are attributed
/// to this fixed name rather than any user RuleSet.
const SCOPE_VALIDATOR: &str = "scope";

/// The shared prefix of [`ScopeSpec::resolve`]'s exactly-one-selector errors.
///
/// Both the zero-selector and the many-selector message are built from this one
/// constant, so adding or renaming a selector edits the list in a single place
/// instead of requiring synchronized edits to two literals that must agree.
const SCOPE_SELECTOR_ERROR_PREFIX: &str =
    "a review scope must set exactly one of file/glob/working/sha";

/// The review scope — exactly one of these resolves to a file set.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
///
/// Fields are deliberately public: this is a forgiving-input builder surface —
/// callers set any subset of selectors on a [`Default`] value via a struct
/// literal, and the exactly-one invariant is enforced by
/// [`resolve`](ScopeSpec::resolve) at resolution time, never at construction.
/// Private fields would protect no invariant here while costing the
/// struct-literal ergonomics this input type exists for.
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
                message: format!("{SCOPE_SELECTOR_ERROR_PREFIX}; none were set"),
            }),
            n => Err(AvpError::Validator {
                validator: SCOPE_VALIDATOR.to_string(),
                message: format!("{SCOPE_SELECTOR_ERROR_PREFIX}; {n} were set"),
            }),
        }
    }
}

/// The per-validator review work-list — the output of [`scope_review`].
#[derive(Debug, Clone, Serialize)]
pub struct WorkList {
    /// The review-level intent.
    change_purpose: String,
    /// One entry per validator that matched at least one changed file.
    validators: Vec<ValidatorWork>,
}

impl WorkList {
    /// Assemble a work-list from the review-level intent and the per-validator
    /// work entries.
    pub fn new(
        change_purpose: impl Into<String>,
        validators: impl IntoIterator<Item = ValidatorWork>,
    ) -> Self {
        Self {
            change_purpose: change_purpose.into(),
            validators: validators.into_iter().collect(),
        }
    }

    /// The review-level intent.
    pub fn change_purpose(&self) -> &str {
        &self.change_purpose
    }

    /// One entry per validator that matched at least one changed file.
    pub fn validators(&self) -> &[ValidatorWork] {
        &self.validators
    }

    /// The distinct files under review across every validator, in first-seen
    /// order, de-duplicated by path.
    ///
    /// Several validators can match the same file; this yields each file once,
    /// the first time its path appears. It is the single dedup the fan-out prime
    /// ([`render_run_prime`](crate::review::fleet::render_run_prime)) builds its
    /// file set from. First-seen order keeps the rendered prime byte-stable
    /// across calls.
    pub fn distinct_files(&self) -> impl Iterator<Item = &FileWork> {
        dedup_by_key(
            self.validators
                .iter()
                .flat_map(|validator| validator.files.iter()),
            |file| file.path.clone(),
        )
    }

    /// The batch-scoped shared probe evidence — currently just the
    /// `<changed-set>` `duplicates` comparison — carried by any validator in
    /// this work-list, deduped by `(probe name, target)` in first-seen
    /// (validator-order) order.
    ///
    /// This evidence spans the WHOLE change under review, not any single
    /// file, so [`ValidatorWork::shared_probe_results`] carries it ONCE per
    /// validator rather than once per file that validator matched. When two
    /// or more validators both declare the same probe, this dedup is what
    /// keeps the fan-out prime ([`render_run_prime`](crate::review::fleet::render_run_prime))
    /// from rendering the identical shared block twice — the same discipline
    /// [`distinct_files`](Self::distinct_files) applies to per-file content.
    pub fn shared_probe_results(&self) -> Vec<ProbeResult> {
        dedup_by_key(
            self.validators
                .iter()
                .flat_map(|validator| validator.shared_probe_results.iter()),
            |result| (result.name.clone(), result.target.clone()),
        )
        .cloned()
        .collect()
    }
}

/// Filter `items` down to the first occurrence of each `key`, in `items`'
/// order — the "yield each distinct key once" discipline shared by
/// [`WorkList::distinct_files`] (keyed by path) and
/// [`WorkList::shared_probe_results`] (keyed by `(probe name, target)`), so
/// the dedup mechanics live in exactly one place.
fn dedup_by_key<T, K: Ord>(
    items: impl Iterator<Item = T>,
    mut key: impl FnMut(&T) -> K,
) -> impl Iterator<Item = T> {
    let mut seen = BTreeSet::new();
    items.filter(move |item| seen.insert(key(item)))
}

/// The rule names inside a validator — what a review fork applies to a file.
///
/// Distinct from [`ProbeNames`] on purpose. Both lists are name lists, and they
/// sit next to each other in [`ValidatorWork::new`], so the compiler is the only
/// thing that can stop a call site passing them in the wrong order; giving each
/// list its own type makes the transposition a type error instead of a validator
/// that reviews against probe names and declares its rules as evidence.
///
/// Serializes as the bare list, so the work-list payload is unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct RuleNames(Vec<String>);

impl RuleNames {
    /// Collect one validator's rule names.
    pub fn new(names: impl IntoIterator<Item = String>) -> Self {
        Self(names.into_iter().collect())
    }

    /// The rule names, in declaration order.
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

/// The probe names a validator declared — the engine-run evidence it wants.
///
/// Distinct from [`RuleNames`] so the two name lists cannot be transposed at a
/// call site; see that type for why the pair is typed.
///
/// Serializes as the bare list, so the work-list payload is unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ProbeNames(Vec<String>);

impl ProbeNames {
    /// Collect one validator's declared probe names.
    pub fn new(names: impl IntoIterator<Item = String>) -> Self {
        Self(names.into_iter().collect())
    }

    /// The probe names, in declaration order.
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

/// One matched validator's slice of the work-list.
#[derive(Debug, Clone, Serialize)]
pub struct ValidatorWork {
    /// The validator (RuleSet) name.
    validator_name: String,
    /// The rule names inside the validator.
    rules: RuleNames,
    /// The probe names the validator declared.
    probes: ProbeNames,
    /// The files this validator must review.
    files: Vec<FileWork>,
    /// The validator's batch-scoped shared probe evidence (currently just the
    /// `<changed-set>` `duplicates` comparison, when this validator declared
    /// `duplicates`) — computed and attached ONCE per validator, never once
    /// per file it matched. See
    /// [`shared_probe_results`](Self::shared_probe_results).
    shared_probe_results: Vec<ProbeResult>,
}

impl ValidatorWork {
    /// Assemble one validator's slice of the work-list, with no shared probe
    /// evidence attached (use
    /// [`with_shared_probe_results`](Self::with_shared_probe_results) when it
    /// matters — the production `scope_review` path always does).
    ///
    /// The two name lists are separate types ([`RuleNames`], [`ProbeNames`]) so
    /// no call site can transpose them.
    pub fn new(
        validator_name: impl Into<String>,
        rules: RuleNames,
        probes: ProbeNames,
        files: impl IntoIterator<Item = FileWork>,
    ) -> Self {
        Self {
            validator_name: validator_name.into(),
            rules,
            probes,
            files: files.into_iter().collect(),
            shared_probe_results: Vec::new(),
        }
    }

    /// Attach the validator's batch-scoped shared probe evidence, computed
    /// once for the whole validator rather than once per file it matched.
    pub fn with_shared_probe_results(
        mut self,
        shared_probe_results: impl IntoIterator<Item = ProbeResult>,
    ) -> Self {
        self.shared_probe_results = shared_probe_results.into_iter().collect();
        self
    }

    /// The validator (RuleSet) name.
    pub fn validator_name(&self) -> &str {
        &self.validator_name
    }

    /// The rule names inside the validator.
    pub fn rules(&self) -> &[String] {
        self.rules.as_slice()
    }

    /// The probe names the validator declared.
    pub fn probes(&self) -> &[String] {
        self.probes.as_slice()
    }

    /// The files this validator must review.
    pub fn files(&self) -> &[FileWork] {
        &self.files
    }

    /// The validator's batch-scoped shared probe evidence — currently just
    /// the `<changed-set>` `duplicates` comparison, when this validator
    /// declared `duplicates`.
    ///
    /// This evidence spans the WHOLE change under review, not any single
    /// file, so it is carried ONCE here rather than cloned onto every
    /// [`FileWork`] the validator matched — see
    /// [`render_shared_probe_evidence`](crate::review::fleet::render_shared_probe_evidence)
    /// for where it renders: once per prompt, never once per file.
    pub fn shared_probe_results(&self) -> &[ProbeResult] {
        &self.shared_probe_results
    }
}

/// One source line's blame + change annotation — the `sha` and `mark` columns
/// [`render_file_block`](crate::review::fleet::render_file_block) renders next
/// to each numbered line of a file's inlined source.
///
/// Computed ONCE per review run by [`scope_review`] (never per finding, never
/// per validator): [`sha`](Self::sha) comes from a single blame call per file
/// (see [`swissarmyhammer_git::GitOperations::blame_lines`]), and
/// [`touched`](Self::touched) comes from the scope stage's own before/after
/// diff — never from blame, which attributes a line to a commit but says
/// nothing about whether THIS review's change touched it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineAnnotation {
    /// The fixed 8-character sha-column label: the first 8 characters of the
    /// commit that last changed this line, or a fixed 8-character sentinel
    /// (`worktree`, `untrackd`, `????????`) — see
    /// [`swissarmyhammer_git::LineBlame::sha_label`].
    sha: String,
    /// Whether THIS review's diff touched this line (renders `+` rather than
    /// a space).
    touched: bool,
}

impl LineAnnotation {
    /// Pair a sha-column label with whether this review's diff touched the
    /// line.
    pub fn new(sha: impl Into<String>, touched: bool) -> Self {
        Self {
            sha: sha.into(),
            touched,
        }
    }

    /// The fixed 8-character sha-column label.
    pub fn sha(&self) -> &str {
        &self.sha
    }

    /// Whether this review's diff touched the line.
    pub fn touched(&self) -> bool {
        self.touched
    }
}

/// One file's worth of work for one validator.
#[derive(Debug, Clone, Serialize)]
pub struct FileWork {
    /// The file path.
    path: String,
    /// The changed entities from the semantic diff.
    semantic_diff: Vec<SemanticChange>,
    /// The names of the changed symbols.
    changed_symbols: Vec<String>,
    /// The file's **complete** current source, inlined in full into the review
    /// payload so the model never needs to `read_file` the changed file.
    ///
    /// A changed file is always inlined whole: it is the file's complete current
    /// contents (empty only for a deletion, which has no current content — the
    /// removal is carried by [`semantic_diff`](FileWork::semantic_diff())). A file
    /// whose rendered block would exceed the review batch budget is never trimmed
    /// to a slice; [`batch_work_list`] excludes that (validator, file) pair and
    /// reports it as a [`SkippedFile`] instead, so this is never a partial view.
    source_slice: String,
    /// The shared `(file, probe)` results.
    probe_results: Vec<ProbeResult>,
    /// One [`LineAnnotation`] per line of `source_slice.trim_end()` (empty for
    /// a deletion, which has no lines to annotate). Attached via
    /// [`with_line_annotations`](Self::with_line_annotations) after
    /// [`new`](Self::new) so the one-shot-per-run blame/diff computation stays
    /// out of the plain constructor every test fixture already calls.
    line_annotations: Vec<LineAnnotation>,
}

impl FileWork {
    /// Assemble one file's worth of work for one validator, with no line
    /// annotations attached (use [`with_line_annotations`](Self::with_line_annotations)
    /// when they matter — the production `scope_review` path always does).
    pub fn new(
        path: impl Into<String>,
        semantic_diff: impl IntoIterator<Item = SemanticChange>,
        changed_symbols: impl IntoIterator<Item = String>,
        source_slice: impl Into<String>,
        probe_results: impl IntoIterator<Item = ProbeResult>,
    ) -> Self {
        Self {
            path: path.into(),
            semantic_diff: semantic_diff.into_iter().collect(),
            changed_symbols: changed_symbols.into_iter().collect(),
            source_slice: source_slice.into(),
            probe_results: probe_results.into_iter().collect(),
            line_annotations: Vec::new(),
        }
    }

    /// Attach the per-line blame/change annotations computed once for this
    /// review run.
    pub fn with_line_annotations(
        mut self,
        line_annotations: impl IntoIterator<Item = LineAnnotation>,
    ) -> Self {
        self.line_annotations = line_annotations.into_iter().collect();
        self
    }

    /// The file path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The changed entities from the semantic diff.
    pub fn semantic_diff(&self) -> &[SemanticChange] {
        &self.semantic_diff
    }

    /// The names of the changed symbols.
    pub fn changed_symbols(&self) -> &[String] {
        &self.changed_symbols
    }

    /// The file's **complete** current source, inlined in full into the review
    /// payload so the model never needs to `read_file` the changed file (see
    /// the field's invariants on wholeness and how [`batch_work_list`] excludes
    /// an oversized file as a [`SkippedFile`] gap instead of trimming it).
    pub fn source_slice(&self) -> &str {
        &self.source_slice
    }

    /// The shared `(file, probe)` results.
    pub fn probe_results(&self) -> &[ProbeResult] {
        &self.probe_results
    }

    /// One [`LineAnnotation`] per line of `source_slice.trim_end()`.
    pub fn line_annotations(&self) -> &[LineAnnotation] {
        &self.line_annotations
    }
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
/// [`WorkList::change_purpose()`] is the commit message(s) under [`Scope::Sha`] and
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

    // The base-revision content per file, keyed for the line-mark diff below.
    // Built from the same `file_changes` the sem differ reads (a borrow, not a
    // move), so this never drifts from what the semantic diff itself saw.
    let before_by_path: BTreeMap<String, Option<String>> = resolved
        .file_changes
        .iter()
        .map(|fc| (fc.file_path.clone(), fc.before_content.clone()))
        .collect();

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

    // Match validators per file via the shared `matching_rulesets` code path,
    // with the workspace's detected project types resolved once for the run.
    let project_types = detected_project_type_keys(repo_path);
    let matched = match_validators_and_files(&resolved.files, loader, &project_types);

    // Run probes ONCE over the whole change set with the union of every declared
    // probe name. This is the N+M guarantee: each distinct `(file, probe)` is
    // computed exactly once and the shared result fans out to every validator
    // that declared it (the distribution below is a pure filter, never a re-run).
    let probe_cache = run_probe_cache(
        &matched.validators,
        &grouped.change_entities,
        &matched.matched_files,
        &resolved.after_content,
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

    // Blame + change-mark every matched file ONCE for the whole run (never per
    // finding, never per validator): one blame call per file, run concurrently.
    let line_annotations = compute_line_annotations(
        repo_path,
        &matched.matched_files,
        &resolved.after_content,
        &before_by_path,
        resolved.blame_at,
    )
    .await;

    // Assemble the work-list: name-sorted validators, each carrying their matched
    // files (path-sorted) with the shared facts + their probe subset.
    let validator_work = assemble_validator_work(
        matched.validators,
        &per_file,
        &probe_cache,
        &line_annotations,
    );

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

/// The distinct detected project type keys for the workspace at `repo_path`
/// (e.g. "rust", "python"), resolved once per review run from the
/// `PROJECT_TYPE_SPECS` detection.
///
/// Detection failure (an unreadable or vanished root) logs a warning and
/// resolves to no types, so a `project_types`-keyed validator simply does not
/// match rather than failing the review.
fn detected_project_type_keys(repo_path: &Path) -> Vec<String> {
    match detect_projects(repo_path, None) {
        Ok(projects) => {
            let keys: BTreeSet<String> = projects
                .iter()
                .map(|project| spec_for(project.project_type).key.to_string())
                .collect();
            keys.into_iter().collect()
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                path = %repo_path.display(),
                "review scope: project type detection failed; \
                 project_types match keys will not match"
            );
            Vec::new()
        }
    }
}

/// Match every resolved file against the loader's validators via the shared
/// `matching_rulesets` code path, accumulating each validator's matched files.
///
/// `project_types` carries the workspace's detected project type keys, so a
/// validator keyed on `match.project_types` is evaluated against the workspace
/// under review.
fn match_validators_and_files(
    files: &[String],
    loader: &ValidatorLoader,
    project_types: &[String],
) -> MatchedValidators {
    let mut matched_files: BTreeSet<String> = BTreeSet::new();
    let mut validators: BTreeMap<String, MatchedValidator> = BTreeMap::new();
    for file in files {
        let ctx = MatchContext::new()
            .with_file(file.clone())
            .with_project_types(project_types.iter().cloned());
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

/// The validator names the engine pairs with `file`, in name order.
///
/// A thin wrapper over `match_validators_and_files` — the pairing every review
/// run performs — so a test (in this crate or downstream, behind the
/// `test-support` feature) can assert against the engine itself instead of a
/// re-implementation that could drift from it. It carries no workspace, so it
/// resolves no project types: a validator keyed on `match.project_types` does
/// not pair here.
#[cfg(any(test, feature = "test-support"))]
pub fn engine_matched_validator_names(file: &str, loader: &ValidatorLoader) -> Vec<String> {
    match_validators_and_files(&[file.to_string()], loader, &[])
        .validators
        .into_keys()
        .collect()
}

/// The 1-based line numbers in `after` this review's change touched, from a
/// pure in-memory diff of `before` against `after` — never from blame, which
/// only attributes a line to a commit and says nothing about whether THIS
/// change touched it.
///
/// `before` is `None` for a file with no base side (a brand-new file, or a
/// glob/single-file scope with no "before" — every line in `after` is then
/// marked, since the whole file is new relative to the review). Identical
/// `before`/`after` (a validator matched a file the diff otherwise left
/// untouched) short-circuits to no marks without invoking git2 at all.
fn compute_line_marks(before: Option<&str>, after: &str) -> BTreeSet<u32> {
    if after.is_empty() {
        return BTreeSet::new();
    }
    let Some(before) = before else {
        return (1..=after.lines().count() as u32).collect();
    };
    if before == after {
        return BTreeSet::new();
    }

    match git2::Patch::from_buffers(before.as_bytes(), None, after.as_bytes(), None, None) {
        Ok(patch) => collect_added_lines(&patch),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "review: failed to diff before/after content for change marks; \
                 no line in this file will be marked as touched"
            );
            BTreeSet::new()
        }
    }
}

/// The new-side 1-based line numbers every `+` line in `patch`'s hunks maps
/// to.
///
/// Extracted from [`compute_line_marks`] so the outer function reduces to a
/// single match on the diff result. A hunk or line that fails to resolve (an
/// out-of-range index, or a `+` line with no new-side line number) is
/// skipped rather than treated as an error — partial hunk data still marks
/// every line it CAN resolve. The per-line resolution itself is factored into
/// [`added_line_number`] so this function stays two loops deep instead of
/// nesting a let-else inside a let-else inside an if inside an if-let.
fn collect_added_lines(patch: &git2::Patch<'_>) -> BTreeSet<u32> {
    let mut marks = BTreeSet::new();
    for hunk_idx in 0..patch.num_hunks() {
        let Ok(lines) = patch.num_lines_in_hunk(hunk_idx) else {
            continue;
        };
        for line_idx in 0..lines {
            if let Some(new_lineno) = added_line_number(patch, hunk_idx, line_idx) {
                marks.insert(new_lineno);
            }
        }
    }
    marks
}

/// The new-side line number `hunk_idx`/`line_idx` maps to, when that line is
/// an added (`+`) line with a resolvable new-side line number.
///
/// `None` for any line that fails to resolve (an out-of-range index), is not
/// an added line, or has no new-side line number — [`collect_added_lines`]
/// treats every `None` identically: skip it, never an error.
fn added_line_number(patch: &git2::Patch<'_>, hunk_idx: usize, line_idx: usize) -> Option<u32> {
    let line = patch.line_in_hunk(hunk_idx, line_idx).ok()?;
    if line.origin() != '+' {
        return None;
    }
    line.new_lineno()
}

/// Blame every matched file's current content ONCE for the whole review run
/// (never per finding, never per validator) and combine it with each file's
/// change marks ([`compute_line_marks`]) into [`LineAnnotation`]s — the `sha`
/// and `mark` columns [`render_file_block`](crate::review::fleet::render_file_block)
/// renders.
///
/// Every matched file's blame call runs CONCURRENTLY: each is a
/// [`tokio::task::spawn_blocking`] that opens its own [`GitOperations`] handle
/// (git2's `Repository` is `Send` but not `Sync`, so a fresh handle per task —
/// cheap, a local `git2_open` — is how independent files blame in parallel
/// without sharing one connection across threads). A file with no content
/// (a deletion) is skipped entirely — nothing to blame, nothing to annotate,
/// matching [`render_file_block`]'s existing empty-block behavior.
///
/// A blame failure for one file is caught, logged with `tracing::warn!`, and
/// degrades that file's every line to [`LineBlame::Failed`]
/// (`????????`) — a blame failure must never abort the review.
async fn compute_line_annotations(
    repo_path: &Path,
    matched_files: &BTreeSet<String>,
    after_content: &BTreeMap<String, String>,
    before_by_path: &BTreeMap<String, Option<String>>,
    blame_at: Option<git2::Oid>,
) -> BTreeMap<String, Vec<LineAnnotation>> {
    // Blame and the change-mark diff both need the file's content EXACTLY as
    // git (and the sem differ) see it, trailing newline and all: a byte diff
    // against a committed blob that ends in `\n` reads a trimmed copy's final
    // line as changed, which would falsely mark the file's last line dirty
    // (blame) or touched (marks) on every file that happens to end in a
    // newline — i.e. nearly every source file. So this stage diffs the RAW
    // content; only the render-facing line COUNT (and thus how many
    // annotations survive) is trimmed, to match
    // [`FileWork::source_slice`]'s `.trim_end()` at render time. `.lines()`
    // ignores a single trailing newline either way, so a normal file's line
    // count is identical between the raw and trimmed forms — this only
    // shortens the annotation list for the rare file with several trailing
    // blank lines, exactly as the pre-existing renderer already discarded them.
    let raw_contents: BTreeMap<String, String> = matched_files
        .iter()
        .filter_map(|file| after_content.get(file).map(|c| (file.clone(), c.clone())))
        .collect();

    let mut tasks = Vec::new();
    for (file, content) in &raw_contents {
        if content.trim_end().is_empty() {
            continue;
        }
        let file = file.clone();
        let content = content.clone();
        let repo_path = repo_path.to_path_buf();
        tasks.push(tokio::task::spawn_blocking(move || {
            let blame = GitOperations::with_work_dir(&repo_path)
                .and_then(|ops| ops.blame_lines(&file, &content, blame_at));
            (file, blame)
        }));
    }

    let mut blame_by_path: BTreeMap<String, Vec<LineBlame>> = BTreeMap::new();
    for task in tasks {
        match task.await {
            Ok((file, Ok(lines))) => {
                blame_by_path.insert(file, lines);
            }
            Ok((file, Err(err))) => {
                let line_count = raw_contents
                    .get(&file)
                    .map(|c| c.trim_end().lines().count())
                    .unwrap_or(0);
                tracing::warn!(
                    file = %file,
                    error = %err,
                    "review: blame failed for file; every line will show as unattributed"
                );
                blame_by_path.insert(file, vec![LineBlame::Failed; line_count]);
            }
            Err(join_err) => {
                tracing::warn!(
                    error = %join_err,
                    "review: a blame task panicked or was cancelled; the affected \
                     file's lines will show as unattributed"
                );
            }
        }
    }

    let mut out = BTreeMap::new();
    for file in matched_files {
        let raw = raw_contents.get(file).map(String::as_str).unwrap_or("");
        let trimmed = raw.trim_end();
        if trimmed.is_empty() {
            out.insert(file.clone(), Vec::new());
            continue;
        }
        let before = before_by_path.get(file).and_then(|b| b.as_deref());
        // Diffed against the RAW after-content (see the function doc for why),
        // so `marks` is keyed by line numbers in `raw`, not `trimmed` — safe to
        // reuse below since `.trim_end()` only ever shortens the tail, never
        // renumbers a leading line.
        let marks = compute_line_marks(before, raw);
        let blame = blame_by_path.get(file);
        let annotations: Vec<LineAnnotation> = trimmed
            .lines()
            .enumerate()
            .map(|(i, _)| {
                let n = (i + 1) as u32;
                let sha = blame
                    .and_then(|b| b.get(i))
                    .map(LineBlame::sha_label)
                    .unwrap_or_else(|| LineBlame::Failed.sha_label());
                LineAnnotation::new(sha, marks.contains(&n))
            })
            .collect();
        out.insert(file.clone(), annotations);
    }
    out
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
        // is carried by the semantic diff). A file whose rendered block would
        // exceed the batch budget is never trimmed here either — [`batch_work_list`]
        // excludes it and reports it as a [`SkippedFile`] gap instead.
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
    line_annotations: &BTreeMap<String, Vec<LineAnnotation>>,
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
                        line_annotations: line_annotations.get(file).cloned().unwrap_or_default(),
                    }
                })
                .collect();
            files.sort_by(|a, b| a.path.cmp(&b.path));
            let shared_probe_results = select_shared_probe_results(probe_cache, &mv.probes);
            ValidatorWork {
                validator_name: mv.name,
                rules: mv.rules,
                probes: mv.probes,
                files,
                shared_probe_results,
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
            probes = ?validator.probes.as_slice(),
            rules = ?validator.rules.as_slice(),
            "review scope: validator matched"
        );
    }
}

/// A validator matched to one or more files, accumulated during matching.
struct MatchedValidator {
    name: String,
    rules: RuleNames,
    probes: ProbeNames,
    files: BTreeSet<String>,
}

impl MatchedValidator {
    fn from_ruleset(rs: &RuleSet) -> Self {
        Self {
            name: rs.name().to_string(),
            rules: RuleNames::new(rs.rules.iter().map(|r| r.name.clone())),
            probes: ProbeNames::new(rs.manifest.probes.iter().cloned()),
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
/// after-content, the review-level change purpose, and blame's history anchor.
struct ResolvedScope {
    files: Vec<String>,
    file_changes: Vec<SemFileChange>,
    after_content: BTreeMap<String, String>,
    change_purpose: String,
    /// The commit blame's history walk is bounded to, mirroring `git blame
    /// <blame_at> -- path`. [`Scope::Working`], [`Scope::File`], and
    /// [`Scope::Glob`] set this to [`working_tree_blame_anchor`]'s merge-base
    /// pin (a stable anchor for the life of a branch), falling back to `None`
    /// (blame against HEAD) only when no such anchor exists. [`Scope::Sha`]
    /// sets this to the range's "to" commit so a bounded historical review
    /// never attributes a line to a commit past that point.
    blame_at: Option<git2::Oid>,
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
fn filter_resolved_scope(resolved: ResolvedScope, matcher: &Gitignore) -> ResolvedScope {
    let ResolvedScope {
        files,
        file_changes,
        after_content,
        change_purpose,
        blame_at,
    } = resolved;

    let mut kept: Vec<String> = Vec::with_capacity(files.len());
    for path in files {
        match review_ignore_reason(matcher, &path) {
            Some(pattern) => tracing::debug!(
                path = %path,
                pattern = %pattern,
                "review scope: excluded ignored path"
            ),
            None => kept.push(path),
        }
    }

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
fn open_repo(repo_path: &Path) -> Result<GitOperations, AvpError> {
    GitOperations::with_work_dir(repo_path)
        .map_err(|e| AvpError::Context(format!("failed to open git repo: {e}")))
}

/// The [`AvpError::Validator`] raised for a scope path that resolves outside the
/// repository root. Carries the FULL, untruncated offending path so the caller
/// can see exactly what was rejected; the message is lowercase and unpunctuated.
fn path_escapes_repo_root(path: &str) -> AvpError {
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
fn normalize_lexically(path: &Path) -> PathBuf {
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
fn confine_to_repo(repo_path: &Path, path: &str) -> Result<PathBuf, AvpError> {
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
fn read_working(repo_path: &Path, path: &str) -> Result<Option<String>, AvpError> {
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
struct GitRefSpec(String);

impl GitRefSpec {
    /// Wrap a commit-ish.
    fn new(refspec: impl Into<String>) -> Self {
        Self(refspec.into())
    }

    /// The current checkout tip — the implicit "before" side of a working-tree or
    /// single-file scope, and the implicit "to" side of a bare-ref range. This is
    /// the single place the `HEAD` literal appears; every caller goes through it.
    fn head() -> Self {
        Self("HEAD".to_string())
    }

    /// The refspec as libgit2 wants it.
    fn as_str(&self) -> &str {
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
struct FilePath(String);

impl FilePath {
    /// Wrap a repo-relative path.
    fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Unwrap the path for a consumer that stores it as a plain `String`.
    fn into_string(self) -> String {
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
fn read_at_ref(
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
fn resolve_sha(repo_path: &Path, range: &str) -> Result<ResolvedScope, AvpError> {
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
fn resolve_file(repo_path: &Path, path: &str) -> Result<ResolvedScope, AvpError> {
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
fn auto_purpose(what: &str) -> String {
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
fn working_tree_blame_anchor(repo: &GitOperations) -> Option<git2::Oid> {
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
fn resolve_oid(repo: &GitOperations, refspec: &GitRefSpec) -> Option<git2::Oid> {
    let inner = repo.repository().inner();
    let object = inner.revparse_single(refspec.as_str()).ok()?;
    object.peel_to_commit().ok().map(|c| c.id())
}

/// Read the commit message for a ref via libgit2, `None` when unresolvable.
fn commit_messages(repo: &GitOperations, refspec: &GitRefSpec) -> Option<String> {
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
struct BeforeContent(Option<String>);

impl BeforeContent {
    /// Wrap the base-revision content of a file.
    fn new(content: Option<String>) -> Self {
        Self(content)
    }

    /// The absent base side — a file that did not exist before the change.
    fn absent() -> Self {
        Self(None)
    }

    /// Unwrap for the sem-diff input.
    fn into_inner(self) -> Option<String> {
        self.0
    }
}

/// A file's content **after** the change — `None` when the file no longer
/// exists (the Deleted signal).
///
/// Distinct from [`BeforeContent`] so the two sides cannot be transposed; see
/// that type for what a transposition would do.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AfterContent(Option<String>);

impl AfterContent {
    /// Wrap the post-change content of a file.
    fn new(content: Option<String>) -> Self {
        Self(content)
    }

    /// Unwrap for the sem-diff input.
    fn into_inner(self) -> Option<String> {
        self.0
    }
}

/// Both sides of one file's change, named rather than positional.
///
/// [`FileChangeBuilder::push`] takes this single argument instead of two
/// `Option<String>`s: the fields name each side at the call site, and their
/// distinct types ([`BeforeContent`], [`AfterContent`]) make a transposed
/// struct literal a compile error rather than an inverted diff.
struct FileVersions {
    /// The content at the base revision.
    before: BeforeContent,
    /// The content after the change.
    after: AfterContent,
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
    ///
    /// The two sides arrive as one named-field [`FileVersions`], so they cannot
    /// be transposed into an inverted diff.
    fn push(&mut self, path: FilePath, versions: FileVersions) {
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
    fn finish(
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

/// Build the shared probe-result cache from a single [`run_probes`] call over the
/// whole change set with the union of every validator's declared probes.
///
/// Entity-bound probes read `change_entities`; file-bound probes (`complexity`)
/// read the current source of every matched file, so they measure the whole
/// review boundary rather than only the entities the diff touched.
async fn run_probe_cache(
    validators: &BTreeMap<String, MatchedValidator>,
    change_entities: &[ChangeEntry],
    matched_files: &BTreeSet<String>,
    after_content: &BTreeMap<String, String>,
    conn: &Connection,
    embedder: &dyn TextEmbedder,
) -> Result<Vec<ProbeResult>, AvpError> {
    let union: BTreeSet<String> = validators
        .values()
        .flat_map(|mv| mv.probes.as_slice().iter().cloned())
        .collect();
    let sources: BTreeMap<String, String> = matched_files
        .iter()
        .filter_map(|file| {
            after_content
                .get(file)
                .map(|content| (file.clone(), content.clone()))
        })
        .collect();
    // A file-bound probe still has work when the diff produced no entities, so
    // an empty entity list alone must not short-circuit the whole cache.
    if union.is_empty() || (change_entities.is_empty() && sources.is_empty()) {
        return Ok(Vec::new());
    }
    let names: Vec<String> = union.into_iter().collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let change = ProbeChange::new(change_entities.to_vec()).with_sources(sources);
    let results = run_probes(&name_refs, &change, conn, embedder).await?;
    Ok(results.results)
}

/// Select the probes results in `cache` that the validator's declared
/// `probes` pulls, further narrowed to whichever results `matches` accepts.
///
/// The shared filter chain — declared-probe-name, then a caller-supplied
/// predicate, then clone-and-collect — behind [`select_probe_results`] (file-
/// scoped, by [`probe_result_for_file`]) and [`select_shared_probe_results`]
/// (batch-scoped, by the `<changed-set>` target), so the two selection paths
/// cannot drift apart on anything but their predicate.
fn select_probe_results_by(
    cache: &[ProbeResult],
    probes: &ProbeNames,
    matches: impl Fn(&ProbeResult) -> bool,
) -> Vec<ProbeResult> {
    cache
        .iter()
        .filter(|r| probes.as_slice().contains(&r.name))
        .filter(|r| matches(r))
        .cloned()
        .collect()
}

/// Select the probe results that belong to `file` and the validator's declared
/// `probes`, from the shared single-run cache.
///
/// `changed_symbols` are this file's changed-entity names (the semantic diff's
/// `entity_name → file_path` mapping, pre-resolved per file), used to attach a
/// symbol-targeted probe result back to the file whose entity bears that name.
///
/// The two name lists are deliberately different types: the declared probes
/// arrive as [`ProbeNames`] and the changed symbols as a plain slice, so a call
/// site cannot transpose them into filtering probe names against symbols.
fn select_probe_results(
    cache: &[ProbeResult],
    file: &str,
    changed_symbols: &[String],
    probes: &ProbeNames,
) -> Vec<ProbeResult> {
    select_probe_results_by(cache, probes, |r| {
        probe_result_for_file(r, file, changed_symbols)
    })
}

/// Whether a probe result's bound subject relates to `file`.
///
/// Probe targets come in two shapes and each resolves to its file differently:
/// - **file path** (`duplicates` per-file) matches the path directly;
/// - **symbol name** (`callers` / `similar`) resolves via the semantic diff's
///   `entity_name → file_path` mapping: it attaches to the file whose changed
///   entity bears that name (`changed_symbols` is that mapping, pre-filtered to
///   this file).
///
/// **`<changed-set>`** (`duplicates` cross-file) is deliberately NOT one of
/// these shapes: it is batch-scoped shared evidence spanning the WHOLE change,
/// not any single file, so it never attaches to a [`FileWork`] here. Cloning
/// it onto every file's `probe_results` used to multiply its bytes by the
/// batch's file count for zero additional information (^t7f5fqf); it is
/// selected once per validator instead, by
/// [`select_shared_probe_results`], and carried on
/// [`ValidatorWork::shared_probe_results`].
fn probe_result_for_file(result: &ProbeResult, file: &str, changed_symbols: &[String]) -> bool {
    result.target == file || changed_symbols.iter().any(|s| s == &result.target)
}

/// Select the batch-scoped shared probe results (currently just the
/// `<changed-set>` `duplicates` comparison) a validator's declared `probes`
/// pulls from the shared single-run cache.
///
/// Computed ONCE per validator — never once per file — because this evidence
/// spans the WHOLE change under review rather than any one file
/// (see [`probe_result_for_file`] for why `<changed-set>` is excluded there).
fn select_shared_probe_results(cache: &[ProbeResult], probes: &ProbeNames) -> Vec<ProbeResult> {
    select_probe_results_by(cache, probes, |r| r.target == "<changed-set>")
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

/// A (validator, file) pair [`batch_work_list`] could not pack into any batch
/// because the file's RENDERED block alone exceeds the batch budget.
///
/// A file is atomic — it is never split across batches — so an oversized block
/// cannot be packed at all. Rather than a hard error that would block review of
/// every OTHER file in the scope, `batch_work_list` excludes the pair and
/// reports it here; [`run_review`](crate::review::run_review) carries it through
/// to the final [`ReviewReport`](crate::review::ReviewReport) as a named "not
/// reviewed, too large" gap.
///
/// The gap is a **pair**, not a path. A file's rendered block carries the probe
/// evidence selected for one validator, so the same path can cost kilobytes for
/// one validator and megabytes for another; dropping the path from the batch
/// would cost every other validator a file it could easily afford. The fields
/// are private (read through the getters) so the shape can evolve without a
/// field-level API commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    /// The oversized file's repo-relative path.
    path: String,
    /// The validator whose rendering of the file did not fit.
    validator: String,
    /// The rendered size of that validator's block for the file, in bytes.
    size: usize,
    /// The per-batch rendered budget it exceeded, in bytes.
    budget: usize,
}

impl SkippedFile {
    /// Construct a [`SkippedFile`] directly for a synthesis-layer test fixture
    /// (`crate::review::synthesize`'s tests), which asserts on rendering given a
    /// skip list rather than driving the whole packer to produce one.
    #[cfg(test)]
    pub(crate) fn for_test(path: &str, validator: &str, size: usize, budget: usize) -> Self {
        Self {
            path: path.to_string(),
            validator: validator.to_string(),
            size,
            budget,
        }
    }

    /// The oversized file's repo-relative path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The validator whose rendering of the file did not fit.
    pub fn validator(&self) -> &str {
        &self.validator
    }

    /// The rendered size of that validator's block for the file, in bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// The per-batch rendered budget it exceeded, in bytes.
    pub fn budget(&self) -> usize {
        self.budget
    }
}

/// Split a [`WorkList`] into budgeted batches at **whole-file** granularity, so
/// every prompt a batch sends stays inside `budget` bytes of file content.
///
/// Cramming every changed file into one shared prime overflows the review
/// model's context on a large diff — every fan-out validator then fails
/// uniformly. So the run is split into batches and each batch fans out
/// independently. A file is **atomic**: it is never split across batches.
///
/// # The cost function is the budget's unit
///
/// `cost` measures what one [`FileWork`] contributes to a prompt, and the
/// budget is denominated in whatever it returns. The fleet passes
/// [`rendered_file_block_bytes`](crate::review::fleet::rendered_file_block_bytes),
/// which renders the block through the very renderer the prompt uses, so the
/// measured bytes and the sent bytes are the same bytes. Taking it as a
/// parameter is what keeps this stage (stage 1, deterministic) from having to
/// know how the fleet stage (stage 2) formats a block, while still budgeting
/// the real thing rather than a proxy for it.
///
/// # Pairs, then paths
///
/// The cost is per **(validator, file) pair** — a block carries the probe
/// evidence selected for that validator, so the same path can cost kilobytes
/// for one validator and megabytes for another. So:
///
/// 1. Any pair whose own cost exceeds `budget` is dropped and reported as a
///    [`SkippedFile`]. It could not be packed without either splitting the file
///    (forbidden) or blowing the budget, and dropping the whole PATH would cost
///    every other validator a file it could easily afford.
/// 2. The surviving distinct files are packed greedily in
///    [`WorkList::distinct_files`] order (the order the prime renders them),
///    each charged the LARGEST surviving cost any validator has for it — the
///    bound that covers both the shared prime and any one validator's
///    monolithic fallback.
///
/// Each returned [`WorkList`] carries every validator that has at least one file
/// in that batch, with the validator's files filtered to the batch (validators
/// left with no files in a batch are dropped). The change purpose is carried
/// verbatim so every batch's prime frames the same overall change. A work-list
/// with no files (no validator matched) yields no batches.
///
/// This never errors: a caller that wants a hard stop on an oversized file
/// checks the returned skip list itself.
pub fn batch_work_list(
    work: &WorkList,
    budget: usize,
    cost: &dyn Fn(&FileWork) -> usize,
) -> (Vec<WorkList>, Vec<SkippedFile>) {
    // Step 1: cost every (validator, file) pair once, dropping the pairs no
    // batch could ever carry and keeping the largest surviving cost per path.
    let mut skipped: Vec<SkippedFile> = Vec::new();
    let mut affordable: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut path_cost: BTreeMap<&str, usize> = BTreeMap::new();
    for validator in &work.validators {
        for file in &validator.files {
            let size = cost(file);
            if size > budget {
                skipped.push(SkippedFile {
                    path: file.path.clone(),
                    validator: validator.validator_name.clone(),
                    size,
                    budget,
                });
                continue;
            }
            affordable.insert((validator.validator_name.as_str(), file.path.as_str()));
            let entry = path_cost.entry(file.path.as_str()).or_insert(0);
            *entry = (*entry).max(size);
        }
    }

    // Step 2: pack the surviving distinct files (first-seen order, matching the
    // prime's file set); a file is never split across a batch boundary.
    let mut batches: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_bytes = 0usize;
    for file in work.distinct_files() {
        let Some(&size) = path_cost.get(file.path.as_str()) else {
            continue;
        };
        if !current.is_empty() && current_bytes + size > budget {
            batches.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current.push(file.path.clone());
        current_bytes += size;
    }
    if !current.is_empty() {
        batches.push(current);
    }

    let batches = batches
        .into_iter()
        .map(|paths| project_onto_files(work, &paths, &affordable))
        .collect();
    (batches, skipped)
}

/// Project a [`WorkList`] onto a subset of file paths: keep every validator that
/// has at least one file in `paths`, with its files filtered to `paths` (order
/// preserved) AND to the `affordable` (validator, path) pairs. Validators left
/// with no files are dropped. The change purpose is carried verbatim so the
/// batch's prime still frames the whole change.
///
/// The pair filter is what keeps a dropped pair out of the batch entirely —
/// including out of [`WorkList::distinct_files`], which the prime renders from
/// and which would otherwise pick the very [`FileWork`] whose rendering did not
/// fit.
fn project_onto_files(
    work: &WorkList,
    paths: &[String],
    affordable: &BTreeSet<(&str, &str)>,
) -> WorkList {
    let keep: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
    let validators = work
        .validators
        .iter()
        .filter_map(|validator| {
            let files: Vec<FileWork> = validator
                .files
                .iter()
                .filter(|file| keep.contains(file.path.as_str()))
                .filter(|file| {
                    affordable.contains(&(validator.validator_name.as_str(), file.path.as_str()))
                })
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
                // Carried verbatim, never re-filtered by `paths`: this
                // evidence is batch-scoped (spans the WHOLE change), not
                // file-scoped, so it does not shrink when a batch subsets the
                // work-list's files.
                shared_probe_results: validator.shared_probe_results.clone(),
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

    use crate::review::probes::ProbeKind;
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

    /// The zero-selector message is pinned to its literal text on purpose. It is
    /// a user-facing contract, and the assertion is deliberately NOT built from
    /// [`SCOPE_SELECTOR_ERROR_PREFIX`] — an expectation composed from the same
    /// constant the code uses would move with it and prove nothing. Editing the
    /// selector list must break this test.
    #[test]
    fn scope_spec_errors_on_zero_selectors() {
        let err = ScopeSpec::default().resolve().unwrap_err();
        match err {
            AvpError::Validator { message, .. } => {
                assert_eq!(
                    message,
                    "a review scope must set exactly one of file/glob/working/sha; none were set"
                );
            }
            other => panic!("expected Validator error, got: {other:?}"),
        }
    }

    /// The many-selector message reports the count AND shares the zero-selector
    /// message's prefix. The prefix half is asserted against
    /// [`SCOPE_SELECTOR_ERROR_PREFIX`] deliberately: together with the literal
    /// pinned in `scope_spec_errors_on_zero_selectors`, that catches BOTH ways
    /// the pair can rot — editing the selector list (the literal there) and
    /// re-introducing a divergent hardcoded message in this branch.
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
                assert!(
                    message.starts_with(SCOPE_SELECTOR_ERROR_PREFIX),
                    "both selector errors must share one prefix, got: {message}"
                );
                assert_eq!(
                    message,
                    format!("{SCOPE_SELECTOR_ERROR_PREFIX}; 2 were set")
                );
            }
            other => panic!("expected Validator error, got: {other:?}"),
        }
    }

    /// `Scope` is a value key: callers cache and de-duplicate resolved scopes,
    /// so it must work in hash-based collections, and its `Hash` must agree with
    /// its `Eq` — equal scopes collapse to one entry, distinct ones do not.
    #[test]
    fn scope_is_usable_as_a_hash_key() {
        use std::collections::HashSet;

        let mut set: HashSet<Scope> = HashSet::new();
        assert!(set.insert(Scope::Working));
        assert!(set.insert(Scope::Sha("HEAD~1".to_string())));
        assert!(set.insert(Scope::File("a.rs".to_string())));
        assert!(set.insert(Scope::Glob("**/*.rs".to_string())));

        // Eq-equal scopes must hash equal: re-inserting is a no-op.
        assert!(!set.insert(Scope::Working));
        assert!(!set.insert(Scope::Sha("HEAD~1".to_string())));
        assert_eq!(set.len(), 4);

        // Same payload, different variant, stays distinct.
        assert!(set.insert(Scope::Glob("a.rs".to_string())));
        assert_eq!(set.len(), 5);
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

        // ^t7f5fqf: the batch-scoped `<changed-set>` result is carried on the
        // VALIDATOR now (once), never cloned onto the file's own
        // `probe_results` — through the REAL `scope_review` path, not just
        // the isolated selection helpers.
        assert!(
            !file
                .probe_results
                .iter()
                .any(|r| r.target == "<changed-set>"),
            "the shared <changed-set> result must not attach to the file's own \
             probe_results; it is carried once on the validator instead, got: {:?}",
            file.probe_results
        );
        assert!(
            validator
                .shared_probe_results()
                .iter()
                .any(|r| r.name == "duplicates" && r.target == "<changed-set>"),
            "the validator's shared_probe_results must carry the changed-set \
             duplicates comparison, got: {:?}",
            validator.shared_probe_results()
        );
    }

    // ---- scope_review: line annotations (blame + change marks) ------------

    /// Production-path test: a real two-commit git history feeds `scope_review`
    /// (`Scope::Sha`), and the resulting `FileWork::line_annotations` must carry
    /// the CORRECT 1-based line number (by position), the correct 8-char blame
    /// sha per line, and the correct `touched` mark from the diff — not from
    /// blame. Line 2 (edited by the second commit) is attributed to the second
    /// commit AND marked touched; lines 1 and 3 (untouched) keep the first
    /// commit's sha and are NOT marked. The real rendered prime block is then
    /// asserted to show line 2 with exactly that number, sha, and `+` mark —
    /// closing the loop from `scope_review` through to what the model reads.
    #[tokio::test]
    async fn sha_scope_line_annotations_carry_correct_number_sha_and_mark() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "line one\nline two\nline three\n");
        let first_sha = repo.commit("first");
        repo.write("src/lib.rs", "line one\nEDITED two\nline three\n");
        let second_sha = repo.commit("second");

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(
            Scope::Sha(format!("{first_sha}..{second_sha}")),
            repo.path(),
            &loader,
            &conn,
            &embedder,
            None,
        )
        .await
        .unwrap();

        let file = work
            .validators()
            .iter()
            .find(|v| v.validator_name() == "rust")
            .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
            .expect("src/lib.rs must be under review");

        let annotations = file.line_annotations();
        assert_eq!(annotations.len(), 3, "one annotation per source line");
        assert_eq!(annotations[0].sha(), &first_sha[..8], "line 1 untouched");
        assert!(!annotations[0].touched());
        assert_eq!(
            annotations[1].sha(),
            &second_sha[..8],
            "line 2 was changed by the second commit"
        );
        assert!(
            annotations[1].touched(),
            "line 2 is exactly what this review's diff touched"
        );
        assert_eq!(annotations[2].sha(), &first_sha[..8], "line 3 untouched");
        assert!(!annotations[2].touched());

        // The actual rendered prime block a model reads must show line 2 with
        // this exact number, sha, and `+` mark — the numbering the model is
        // told to READ rather than count.
        let rendered = crate::review::fleet::render_file_payload(std::slice::from_ref(file));
        let expected_line_2 = format!("     2 | {} + | EDITED two", &second_sha[..8]);
        assert!(
            rendered.contains(&expected_line_2),
            "rendered block must show line 2 numbered with its blame sha and a `+` mark, got:\n{rendered}"
        );
        // An untouched line renders a space, never a `+`.
        let expected_line_1 = format!("     1 | {}   | line one", &first_sha[..8]);
        assert!(
            rendered.contains(&expected_line_1),
            "untouched line 1 must render a space in the mark column, got:\n{rendered}"
        );
    }

    /// A dirty, uncommitted edit in the `review working` scope has no committed
    /// history at that content: `LineBlame::Worktree` (the same sentinel `git
    /// blame` itself uses for "not committed yet"), rendered as `worktree` in
    /// the sha column. The SAME line is also marked touched, since the
    /// before/after diff sees it as changed — showing that `touched` and `sha`
    /// are computed independently and can legitimately agree.
    #[tokio::test]
    async fn working_scope_dirty_line_gets_worktree_sha() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "alpha\nbeta\ngamma\n");
        let sha = repo.commit("initial");

        // Dirty, uncommitted edit to line 2.
        repo.write("src/lib.rs", "alpha\nBETA-DIRTY\ngamma\n");

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();

        let file = work
            .validators()
            .iter()
            .find(|v| v.validator_name() == "rust")
            .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
            .expect("src/lib.rs must be under review");

        let annotations = file.line_annotations();
        assert_eq!(annotations.len(), 3);
        assert_eq!(annotations[0].sha(), &sha[..8]);
        assert!(!annotations[0].touched());
        assert_eq!(
            annotations[1].sha(),
            "worktree",
            "an uncommitted dirty line must show worktree in the sha column, got: {:?}",
            annotations[1]
        );
        assert!(annotations[1].touched());
        assert_eq!(annotations[2].sha(), &sha[..8]);
        assert!(!annotations[2].touched());
    }

    /// A brand-new untracked file (never `git add`ed) has no git history at
    /// all: every line shows `untrackd`, and the whole file is marked touched
    /// (there is no "before" side — it is entirely new relative to the review).
    #[tokio::test]
    async fn working_scope_untracked_new_file_gets_untrackd_sha() {
        let repo = TestRepo::new();
        repo.write("README.md", "# base\n");
        repo.commit("initial");

        repo.write("src/new.rs", "pub fn brand_new() {}\n");

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();

        let file = work
            .validators()
            .iter()
            .find(|v| v.validator_name() == "rust")
            .and_then(|v| v.files().iter().find(|f| f.path() == "src/new.rs"))
            .expect("the untracked new file must be under review");

        let annotations = file.line_annotations();
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].sha(), "untrackd");
        assert!(
            annotations[0].touched(),
            "every line of a brand-new file is touched by this review"
        );
    }

    /// Regression test for ^8p6kjmw: the `/finish` loop's commit step commits
    /// EVERY dirty file together (`git add -A && git commit`), not just the
    /// one file whose finding it actually resolved. That sweeps up a
    /// STILL-UNRESOLVED dirty line in some OTHER file right along with it —
    /// the exact "intervening commit of unrelated files" the task describes.
    ///
    /// Here `src/lib.rs` line 2 (`BETA-DIRTY`) is a still-open, unresolved
    /// finding: its BYTES never change across the two runs. Between the runs,
    /// a commit lands that (a) genuinely fixes `src/lib.rs` line 4 and (b)
    /// incidentally sweeps up line 2's untouched dirty edit too, because
    /// `commit_only` — like a real `git add -A` — stages the WHOLE file, not
    /// individual lines. A third, freshly dirty line 1 keeps the file in
    /// `review working` scope for run 2, isolating the assertion to line 2
    /// alone: with `blame_at: None` (binding to HEAD), line 2 flips from
    /// `worktree` (run 1) to a real committed sha (run 2) even though nothing
    /// about that specific finding changed — the prompt drift the task
    /// reports. Pinning the blame anchor to the branch's merge-base with
    /// `main` (fixed for the whole session, since `main` never moves here)
    /// closes this: the sweep commit postdates the anchor, so it stays
    /// invisible to blame and line 2 keeps reading `worktree` in both runs.
    #[tokio::test]
    async fn working_scope_sha_is_stable_across_a_sweep_commit_of_an_unresolved_line() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "alpha\nbeta\ngamma\ndelta\n");
        repo.commit("initial");
        repo.rename_current_branch_to("main");
        repo.checkout_new_branch("task");

        // A still-open, unresolved dirty edit to line 2 — this exact edit
        // survives, byte-for-byte, across both runs.
        repo.write("src/lib.rs", "alpha\nBETA-DIRTY\ngamma\ndelta\n");

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work1 = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();
        let file1 = work1
            .validators()
            .iter()
            .find(|v| v.validator_name() == "rust")
            .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
            .expect("src/lib.rs must be under review");
        let line2_run1 = file1.line_annotations()[1].sha().to_string();
        assert_eq!(
            line2_run1, "worktree",
            "line 2's still-open dirty edit must show worktree before the sweep commit"
        );

        // A second, DIFFERENT finding on line 4 gets genuinely fixed and
        // committed — but the commit step stages the WHOLE file, sweeping up
        // line 2's still-unresolved edit right along with it.
        repo.write("src/lib.rs", "alpha\nBETA-DIRTY\ngamma\nDELTA-FIXED\n");
        repo.commit_only(&["src/lib.rs"], "fix line 4's finding");

        // A THIRD, freshly dirty line keeps the file in `review working`
        // scope for run 2, without touching line 2 at all.
        repo.write(
            "src/lib.rs",
            "ALPHA-DIRTY\nBETA-DIRTY\ngamma\nDELTA-FIXED\n",
        );

        let work2 = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();
        let file2 = work2
            .validators()
            .iter()
            .find(|v| v.validator_name() == "rust")
            .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
            .expect("src/lib.rs must still be under review (line 1 is freshly dirty)");
        let line2_run2 = file2.line_annotations()[1].sha().to_string();

        assert_eq!(
            line2_run1, line2_run2,
            "line 2's sha must not drift across the sweep commit: it is the SAME \
             unresolved finding, byte-for-byte, in both runs — run1={line2_run1:?} run2={line2_run2:?}"
        );
    }

    /// A blame failure for any other reason (here: the repo handle itself
    /// cannot be opened) must never abort the review: every affected line
    /// degrades to the `????????` sentinel and `compute_line_annotations`
    /// still returns a complete, non-panicking result — proving the
    /// review-completes contract structurally (the function has no `Result`
    /// to propagate a failure through in the first place).
    #[tokio::test]
    async fn blame_failure_degrades_to_unknown_marker_without_aborting() {
        let mut matched_files = BTreeSet::new();
        matched_files.insert("src/lib.rs".to_string());
        let mut after_content = BTreeMap::new();
        after_content.insert("src/lib.rs".to_string(), "line one\nline two\n".to_string());
        let before_by_path = BTreeMap::new();

        // A path with no git repository at all: `GitOperations::with_work_dir`
        // fails for every file, exercising the "blame fails for any other
        // reason" arm rather than the untracked/brand-new short-circuits.
        let not_a_repo = tempfile::tempdir().unwrap();

        let annotations = compute_line_annotations(
            not_a_repo.path(),
            &matched_files,
            &after_content,
            &before_by_path,
            None,
        )
        .await;

        let file_annotations = annotations
            .get("src/lib.rs")
            .expect("the file is still annotated even though blame failed");
        assert_eq!(file_annotations.len(), 2);
        for annotation in file_annotations {
            assert_eq!(
                annotation.sha(),
                "????????",
                "a blame failure must degrade to the unknown sentinel, got: {annotation:?}"
            );
        }
    }

    /// End-to-end: the line number the model READS off the real rendered prime
    /// (not counted, not derived) survives, byte-for-byte, all the way through
    /// `Finding.line` and into `synthesize`'s final report — no stage in
    /// between renumbers it. This is the numbering half of the task closed
    /// loop: [`sha_scope_line_annotations_carry_correct_number_sha_and_mark`]
    /// proves the printed number is correct; this proves it is never lost.
    #[tokio::test]
    async fn a_findings_line_number_survives_from_the_prime_to_the_report() {
        let repo = TestRepo::new();
        repo.write(
            "src/lib.rs",
            "fn one() {}\nfn two() {}\nfn three() {}\nfn four() {}\n",
        );
        repo.commit("initial");

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
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
        let file = work
            .validators()
            .iter()
            .find(|v| v.validator_name() == "rust")
            .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
            .expect("src/lib.rs must be under review");

        let rendered = crate::review::fleet::render_file_payload(std::slice::from_ref(file));
        // The exact numbered line the model would read for line 3 — pulled
        // from the REAL render, not hand-typed, so this test would fail if the
        // format ever drifted.
        let printed_line_3 = rendered
            .lines()
            .find(|l| l.trim_start().starts_with("3 |") || l.trim_start().starts_with("     3 |"))
            .expect("the rendered block must number line 3");
        assert!(
            printed_line_3.ends_with("fn three() {}"),
            "line 3 as printed must be line 3 of the real source: {printed_line_3}"
        );

        // The model "reads" the number 3 off that exact line and cites it in
        // its findings JSON — simulated here via the real parse path rather
        // than constructing a `Finding` by hand, so the whole textual
        // round-trip is exercised.
        let agent_response = crate::review::test_support::findings_json(
            "src/lib.rs",
            3,
            "no-unused",
            "`three` looks unused",
        );
        let findings = crate::review::types::parse_findings(&agent_response).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].line, 3,
            "the parsed finding must keep the exact line the model cited"
        );

        // Verify + synthesize: the line must reach the final report unchanged.
        let verified = vec![crate::review::types::VerifiedFinding {
            finding: findings[0].clone(),
            confirmed: true,
            reason: "confirmed".to_string(),
            decided_by: None,
        }];
        let report = crate::review::synthesize::synthesize(
            verified,
            &crate::review::synthesize::FleetTally::new(
                crate::review::synthesize::TasksAttempted(1),
                crate::review::synthesize::TasksFailed(0),
            ),
            &[],
            "2026-04-11 13:08",
        );
        assert!(
            report.markdown().contains("`src/lib.rs:3`"),
            "the final report must cite the SAME line the model read off the prime: {}",
            report.markdown()
        );
    }

    /// Regression test for ^j4d2613: a real, known two-commit history through
    /// `Scope::Sha`, on a file with many untouched lines ABOVE the one edited
    /// line — the exact shape the bug report cited ("a commit that touches a
    /// file with many edits above the changed region, since that is where
    /// drift shows"). The old, unnumbered render made the model COUNT lines,
    /// and the miscount grew with depth; this proves the edited line's number,
    /// blame sha, and touched mark survive correctly at depth, and that a
    /// finding citing that number round-trips, unchanged, all the way to the
    /// final report — closing the gap the earlier small fixture tests (e.g.
    /// [`a_findings_line_number_survives_from_the_prime_to_the_report`]) do
    /// not cover at scale.
    #[tokio::test]
    async fn a_known_commit_with_many_lines_above_the_change_resolves_the_correct_symbol() {
        const LINES_ABOVE: usize = 190;

        let repo = TestRepo::new();
        let filler = || -> String {
            (0..LINES_ABOVE)
                .map(|i| format!("fn filler_{i}() {{}}\n"))
                .collect()
        };

        let mut before = filler();
        before.push_str("fn target() { old_body(); }\n");
        repo.write("src/big.rs", &before);
        let first_sha = repo.commit("first");

        let mut after = filler();
        after.push_str("fn target() { new_body(); }\n");
        repo.write("src/big.rs", &after);
        let second_sha = repo.commit("second");

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(
            Scope::Sha(format!("{first_sha}..{second_sha}")),
            repo.path(),
            &loader,
            &conn,
            &embedder,
            None,
        )
        .await
        .unwrap();

        let file = work
            .validators()
            .iter()
            .find(|v| v.validator_name() == "rust")
            .and_then(|v| v.files().iter().find(|f| f.path() == "src/big.rs"))
            .expect("src/big.rs must be under review");

        // 1-based line number of the edited `target` function: LINES_ABOVE
        // filler lines precede it.
        let changed_line = LINES_ABOVE + 1;
        let annotations = file.line_annotations();
        assert_eq!(
            annotations.len(),
            changed_line,
            "one annotation per source line"
        );

        // Every filler line above the change keeps the FIRST commit's blame
        // and stays unmarked — the large untouched block must not shift or
        // misattribute the edited line below it.
        for (i, annotation) in annotations.iter().take(LINES_ABOVE).enumerate() {
            assert_eq!(
                annotation.sha(),
                &first_sha[..8],
                "line {} sits above the change and must keep the first commit's blame",
                i + 1
            );
            assert!(
                !annotation.touched(),
                "line {} must not be marked touched",
                i + 1
            );
        }

        // The edited line itself blames to the SECOND commit and is touched.
        let changed = &annotations[changed_line - 1];
        assert_eq!(
            changed.sha(),
            &second_sha[..8],
            "the edited line must blame to the commit that edited it, not one of the {LINES_ABOVE} untouched lines above it"
        );
        assert!(changed.touched(), "the edited line must be marked touched");

        // The exact numbered line the model would read for the edited
        // function — pulled from the REAL render, not hand-typed, so this
        // test fails if the numbering ever drifts at depth.
        let rendered = crate::review::fleet::render_file_payload(std::slice::from_ref(file));
        let expected_printed_line = format!(
            "{changed_line:>6} | {} + | fn target() {{ new_body(); }}",
            &second_sha[..8]
        );
        assert!(
            rendered.contains(&expected_printed_line),
            "rendered block must number the edited line at {changed_line} with the second commit's sha and a `+` mark, got:\n{rendered}"
        );

        // The model "reads" that number off the render and cites it in its
        // findings JSON — simulated via the real parse path, on a file large
        // enough that a counting-based miscite would show up as drift.
        let agent_response = crate::review::test_support::findings_json(
            "src/big.rs",
            changed_line as u32,
            "no-dead-code",
            "`target` calls `new_body`, which is unused",
        );
        let findings = crate::review::types::parse_findings(&agent_response).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].line,
            changed_line as u32,
            "the parsed finding must keep the exact line the model cited, {LINES_ABOVE} lines of untouched content notwithstanding"
        );

        // Verify + synthesize: the line must reach the final report unchanged,
        // still resolving to the same `target` symbol the diff actually
        // touched.
        let verified = vec![crate::review::types::VerifiedFinding {
            finding: findings[0].clone(),
            confirmed: true,
            reason: "confirmed".to_string(),
            decided_by: None,
        }];
        let report = crate::review::synthesize::synthesize(
            verified,
            &crate::review::synthesize::FleetTally::new(
                crate::review::synthesize::TasksAttempted(1),
                crate::review::synthesize::TasksFailed(0),
            ),
            &[],
            "2026-08-03 12:00",
        );
        let expected_citation = format!("`src/big.rs:{changed_line}`");
        assert!(
            report.markdown().contains(&expected_citation),
            "the final report must cite the SAME line the model read off the prime, {LINES_ABOVE} lines deep: {}",
            report.markdown()
        );
    }

    /// Measures (does not tightly pin — wall-clock is inherently machine-
    /// dependent) the blame overhead `scope_review` now pays on a
    /// representative "normal commit": 8 changed files of ~150 lines each,
    /// each with one dirty edit. Blame runs ONCE per file, concurrently
    /// (`compute_line_annotations`'s `tokio::task::spawn_blocking` fan-out) —
    /// this asserts only the coarse sanity bound that 8 files blame well
    /// under a second, which would fail loudly if concurrency regressed to
    /// sequential. The printed wall-clock (`cargo test -- --nocapture`) is
    /// the number recorded on the task as the measured added cost.
    #[tokio::test]
    async fn blame_overhead_on_a_representative_commit_is_small_and_concurrent() {
        const FILE_COUNT: usize = 8;
        const LINES_PER_FILE: usize = 150;

        let repo = TestRepo::new();
        let body = |seed: usize| -> String {
            (0..LINES_PER_FILE)
                .map(|i| format!("fn line_{seed}_{i}() {{}}\n"))
                .collect()
        };
        for i in 0..FILE_COUNT {
            repo.write(&format!("src/file_{i}.rs"), &body(i));
        }
        repo.commit("initial");
        // A dirty, uncommitted one-line edit per file — a realistic "normal
        // commit" shape (every file touched, not wholesale rewritten).
        for i in 0..FILE_COUNT {
            let mut content = body(i);
            content.push_str("fn dirty_edit() {}\n");
            repo.write(&format!("src/file_{i}.rs"), &content);
        }

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let started = std::time::Instant::now();
        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();
        let elapsed = started.elapsed();
        println!(
            "scope_review with blame over {FILE_COUNT} files x {LINES_PER_FILE} lines took {elapsed:?}"
        );

        let validator = work
            .validators()
            .iter()
            .find(|v| v.validator_name() == "rust")
            .expect("the rust validator must match");
        assert_eq!(validator.files().len(), FILE_COUNT);
        for file in validator.files() {
            assert!(
                !file.line_annotations().is_empty(),
                "every file must carry line annotations"
            );
        }
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "blame across {FILE_COUNT} small files run concurrently must not take {elapsed:?}"
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
            line_annotations: vec![],
        }
    }

    fn validator_over(name: &str, paths: &[&str]) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            rules: RuleNames::new([format!("{name}-rule")]),
            probes: ProbeNames::default(),
            files: paths.iter().map(|p| file_at(p)).collect(),
            shared_probe_results: vec![],
        }
    }

    /// A cost function that charges a file its raw source bytes.
    ///
    /// The packing tests below fix each fixture file's size through
    /// [`file_sized`], so charging raw bytes keeps their arithmetic legible.
    /// Production passes
    /// [`rendered_file_block_bytes`](crate::review::fleet::rendered_file_block_bytes)
    /// instead — the packer is agnostic to which, which is the point of taking
    /// the cost as a parameter.
    fn raw_source_bytes(file: &FileWork) -> usize {
        file.source_slice.len()
    }

    /// A validator over `(path, byte-size)` files, for [`batch_work_list`] packing
    /// assertions.
    fn validator_sized(name: &str, files: &[(&str, usize)]) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            rules: RuleNames::new([format!("{name}-rule")]),
            probes: ProbeNames::default(),
            files: files.iter().map(|(p, n)| file_sized(p, *n)).collect(),
            shared_probe_results: vec![],
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
    fn work_list_getters_and_constructors_round_trip_the_private_fields() {
        let file = FileWork::new(
            "src/a.rs".to_string(),
            vec![],
            vec!["alpha".to_string()],
            "fn alpha() {}".to_string(),
            vec![],
        );
        assert_eq!(file.path(), "src/a.rs");
        assert!(file.semantic_diff().is_empty());
        assert_eq!(file.changed_symbols(), ["alpha".to_string()]);
        assert_eq!(file.source_slice(), "fn alpha() {}");
        assert!(file.probe_results().is_empty());

        let validator = ValidatorWork::new(
            "dedup".to_string(),
            RuleNames::new(["dedup-rule".to_string()]),
            ProbeNames::new(["similar".to_string()]),
            vec![file],
        );
        assert_eq!(validator.validator_name(), "dedup");
        assert_eq!(validator.rules(), ["dedup-rule".to_string()]);
        assert_eq!(validator.probes(), ["similar".to_string()]);
        assert_eq!(validator.files().len(), 1);

        let work = WorkList::new("purpose".to_string(), vec![validator]);
        assert_eq!(work.change_purpose(), "purpose");
        assert_eq!(work.validators().len(), 1);

        let spec = ScopeSpec {
            working: true,
            ..Default::default()
        };
        assert_eq!(spec.resolve().unwrap(), Scope::Working);
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

        let (batches, skipped) = batch_work_list(&work, 25, &raw_source_bytes);

        assert_eq!(
            batches.iter().map(batch_paths).collect::<Vec<_>>(),
            vec![vec!["a.rs", "b.rs"], vec!["c.rs"]],
            "files pack greedily into whole-file batches under the budget"
        );
        assert!(skipped.is_empty(), "no file is oversized: {skipped:?}");
        for batch in &batches {
            let total: usize = batch.distinct_files().map(|f| f.source_slice.len()).sum();
            assert!(total <= 25, "every batch stays within the byte budget");
        }
    }

    #[test]
    fn batch_work_list_skips_a_single_file_over_the_budget_and_packs_the_rest() {
        // One file larger than the budget cannot be packed without splitting it
        // (forbidden) — it is excluded and reported, never a hard error that
        // blocks the rest of the scope. `small.rs` still packs normally.
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![validator_sized("v", &[("big.rs", 100), ("small.rs", 10)])],
        };

        let (batches, skipped) = batch_work_list(&work, 32, &raw_source_bytes);

        assert_eq!(
            batches.iter().map(batch_paths).collect::<Vec<_>>(),
            vec![vec!["small.rs"]],
            "the non-oversized file still packs and reviews: {batches:?}"
        );
        assert_eq!(skipped.len(), 1, "exactly the oversized file is skipped");
        assert_eq!(skipped[0].path(), "big.rs", "names the offending file");
        assert_eq!(skipped[0].size(), 100, "names the file's size");
        assert_eq!(
            skipped[0].validator(),
            "v",
            "names the validator that could not carry it"
        );
        assert_eq!(skipped[0].budget(), 32, "names the limit");
    }

    #[test]
    fn batch_work_list_small_diff_is_exactly_one_batch() {
        // Today's fast path: a small diff fits one batch, unchanged.
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![validator_sized("v", &[("a.rs", 10), ("b.rs", 10)])],
        };

        let (batches, skipped) = batch_work_list(&work, 32 * 1024, &raw_source_bytes);

        assert_eq!(batches.len(), 1, "a small diff is a single batch");
        assert_eq!(batch_paths(&batches[0]), vec!["a.rs", "b.rs"]);
        assert!(skipped.is_empty());
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

        let (batches, skipped) = batch_work_list(&work, 25, &raw_source_bytes);

        assert_eq!(batches.len(), 2);
        assert_eq!(batch_validators(&batches[0]), vec!["v1"]);
        assert_eq!(batch_paths(&batches[0]), vec!["a.rs", "b.rs"]);
        assert_eq!(batch_validators(&batches[1]), vec!["v2"]);
        assert_eq!(batch_paths(&batches[1]), vec!["c.rs"]);
        assert!(skipped.is_empty());
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

        let (batches, skipped) = batch_work_list(&work, 25, &raw_source_bytes);

        assert_eq!(batches.len(), 1, "the one distinct file is one batch");
        assert_eq!(batch_paths(&batches[0]), vec!["shared.rs"]);
        assert_eq!(
            batch_validators(&batches[0]),
            vec!["v1", "v2"],
            "both validators that matched the shared file ride the same batch"
        );
        assert!(skipped.is_empty());
    }

    #[test]
    fn batch_work_list_empty_work_yields_no_batches() {
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![],
        };
        let (batches, skipped) = batch_work_list(&work, 32 * 1024, &raw_source_bytes);
        assert!(batches.is_empty());
        assert!(skipped.is_empty());
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

    // ---- scope_review: project-type matching ------------------------------

    /// A single-rule RuleSet matching `file_glob` files in workspaces with any
    /// of the named detected project types.
    fn project_typed_ruleset(name: &str, file_glob: &str, project_types: &[&str]) -> RuleSet {
        let mut rs = ruleset(name, file_glob, &[]);
        rs.manifest
            .match_criteria
            .as_mut()
            .expect("the ruleset fixture always carries match criteria")
            .project_types = project_types.iter().map(|t| t.to_string()).collect();
        rs
    }

    #[tokio::test]
    async fn scope_review_resolves_workspace_project_types_for_matching() {
        let repo = TestRepo::new();
        repo.write(
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
        );
        repo.write("src/lib.rs", "fn placeholder() {}\n");
        repo.commit("initial");
        repo.write("src/lib.rs", "fn placeholder() {}\n\nfn added() {}\n");

        let conn = index_conn();
        let embedder = MockEmbedder::new(DIM);

        let mut loader = ValidatorLoader::new();
        // The repo carries a Cargo.toml, so the rust-keyed validator applies.
        loader.add_builtin_ruleset(project_typed_ruleset("rust-keyed", "*.rs", &["rust"]));
        // No python markers exist, so the python-keyed validator does not.
        loader.add_builtin_ruleset(project_typed_ruleset("python-keyed", "*.rs", &["python"]));

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();

        let names: Vec<&str> = work
            .validators
            .iter()
            .map(|v| v.validator_name.as_str())
            .collect();
        assert!(
            names.contains(&"rust-keyed"),
            "a rust-keyed validator must match in a rust workspace, got {names:?}"
        );
        assert!(
            !names.contains(&"python-keyed"),
            "a python-keyed validator must not match in a non-python workspace, got {names:?}"
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

    // ---- scope_review: complexity probe evidence -------------------------

    /// A file with one function far over the nesting gate and one well under it.
    const MIXED_COMPLEXITY_SOURCE: &str = r#"fn deep(a: bool, b: bool, items: &[u8]) -> u8 {
    if a {
        for item in items {
            while b {
                if *item > 0 {
                    return 1;
                }
            }
        }
    }
    0
}

fn shallow(a: Option<u8>) -> u8 {
    match a {
        Some(v) => v,
        None => 0,
    }
}
"#;

    /// Drive the real `scope_review` over a repo holding `source`, and return the
    /// `complexity` probe evidence the pipeline attached to the file.
    async fn complexity_evidence_for(source: &str) -> Vec<ProbeResult> {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn placeholder() {}\n");
        repo.commit("initial");
        repo.write("src/lib.rs", source);

        let conn = index_conn();
        let loader = loader_with("complexity", "*.rs", &["complexity"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .expect("the working scope resolves");

        work.validators
            .iter()
            .find(|v| v.validator_name == "complexity")
            .expect("the complexity validator matched the changed .rs file")
            .files
            .iter()
            .find(|f| f.path == "src/lib.rs")
            .expect("the changed file is work")
            .probe_results
            .clone()
    }

    #[tokio::test]
    async fn scope_review_attaches_computed_complexity_evidence_to_the_file() {
        // The production path: a real repo, the real loader, the real probe
        // runner. The agent must receive the measured numbers, not be asked for
        // them.
        let evidence = complexity_evidence_for(MIXED_COMPLEXITY_SOURCE).await;

        let result = evidence
            .iter()
            .find(|r| r.name == "complexity")
            .expect("the complexity probe result reaches the work item");
        let symbols: Vec<&str> = result
            .rows
            .iter()
            .filter_map(|row| row.symbol.as_deref())
            .collect();

        assert_eq!(
            symbols,
            vec!["deep"],
            "only the over-gate function is listed, got: {:?}",
            result.rows
        );
        assert!(
            result.rows[0]
                .detail
                .as_deref()
                .is_some_and(|d| d.contains("max condition-nesting depth 4 (gate 4)")),
            "the row carries the measured depth and its gate, got: {:?}",
            result.rows[0].detail
        );
    }

    #[tokio::test]
    async fn scope_review_reports_an_empty_complexity_result_for_a_simple_file() {
        // An empty result is the deterministic fact the verify guard refutes a
        // complexity claim with. It must survive the whole pipeline, not be
        // dropped as "no evidence".
        let evidence = complexity_evidence_for(
            "fn shallow(a: Option<u8>) -> u8 {\n    match a {\n        Some(v) => v,\n        None => 0,\n    }\n}\n",
        )
        .await;

        let result = evidence
            .iter()
            .find(|r| r.name == "complexity")
            .expect("a simple file still gets a complexity result");
        assert!(
            result.rows.is_empty(),
            "no function is over a gate, got: {:?}",
            result.rows
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

    // ---- scope_review: .reviewignore + .gitignore exclusion --------------

    /// The distinct file paths any validator carries in a resolved work-list.
    fn work_paths(work: &WorkList) -> Vec<String> {
        work.validators
            .iter()
            .flat_map(|v| v.files.iter().map(|f| f.path.clone()))
            .collect()
    }

    /// A tracked, edited `.kanban/board.md` — the finish-loop churn the default
    /// `.reviewignore` exists to suppress — must be dropped from the working
    /// scope even though a tracked non-code modification would otherwise stay.
    #[tokio::test]
    async fn working_scope_excludes_tracked_kanban_board_edits() {
        let repo = TestRepo::new();
        repo.write(".kanban/board.md", "# board\n");
        repo.write("src/lib.rs", "fn base() {}\n");
        repo.commit("initial");
        // A finish-loop comment edits the tracked board file.
        repo.write(".kanban/board.md", "# board\n- new comment\n");

        let conn = index_conn();
        // A match-everything validator would otherwise pull the board into scope.
        let loader = loader_with("everything", "*", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();

        let paths = work_paths(&work);
        assert!(
            !paths.iter().any(|p| p.starts_with(".kanban/")),
            "the auto-ignored .kanban board must not enter working scope, got: {paths:?}"
        );
    }

    /// A committed, tracked file matched by the repo's `.gitignore` must be
    /// excluded from a `Scope::Sha` range while a real source edit in the same
    /// range stays — gitignored artifacts are not source even when tracked.
    #[tokio::test]
    async fn sha_scope_excludes_a_gitignored_committed_file() {
        let repo = TestRepo::new();
        // The generated file is committed BEFORE the ignore exists, so it is
        // tracked; git ignores only untracked paths, so it stays tracked after.
        repo.write("src/lib.rs", "fn base() {}\n");
        repo.write("src/schema.gen.rs", "fn generated() {}\n");
        repo.commit("initial");
        repo.write(".gitignore", "*.gen.rs\n");
        repo.write("src/lib.rs", "fn base() {}\n\nfn added() {}\n");
        repo.write("src/schema.gen.rs", "fn generated() {}\n\nfn more() {}\n");
        repo.commit("second");

        let conn = index_conn();
        let loader = loader_with("everything", "*", &[]);
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

        let paths = work_paths(&work);
        assert!(
            !paths.iter().any(|p| p.ends_with(".gen.rs")),
            "a gitignored committed file must be excluded from sha scope, got: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p == "src/lib.rs"),
            "the real source edit must stay in sha scope, got: {paths:?}"
        );
    }

    /// A `Scope::Glob` result is filtered through the ignore matcher too: a
    /// user's `.reviewignore` directory pattern drops vendored files a broad
    /// glob would otherwise sweep in.
    #[tokio::test]
    async fn glob_scope_excludes_reviewignored_files() {
        let repo = TestRepo::new();
        repo.write(".reviewignore", "src/vendor/\n");
        repo.write("src/lib.rs", &format!("{}\n", body("real")));
        repo.write("src/vendor/dep.rs", &format!("{}\n", body("vendored")));
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

        let paths = work_paths(&work);
        assert!(
            paths.iter().any(|p| p == "src/lib.rs"),
            "the real source file must stay in glob scope, got: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("vendor")),
            "the reviewignored vendored file must be excluded from glob scope, got: {paths:?}"
        );
    }

    /// A `!` negation in `.reviewignore` re-includes a file a broader directory
    /// pattern excluded — full gitignore semantics carry through the scope filter.
    #[tokio::test]
    async fn working_scope_negation_reincludes_a_broadly_excluded_file() {
        let repo = TestRepo::new();
        repo.write(".reviewignore", "src/gen/\n!src/gen/keep.rs\n");
        repo.write("src/lib.rs", "fn base() {}\n");
        repo.commit("initial");
        // Two brand-new untracked source files under the excluded directory.
        repo.write("src/gen/noise.rs", &format!("{}\n", body("noise")));
        repo.write("src/gen/keep.rs", &format!("{}\n", body("keep")));

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();

        let paths = work_paths(&work);
        assert!(
            paths.iter().any(|p| p == "src/gen/keep.rs"),
            "the `!` negation must re-include keep.rs, got: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p == "src/gen/noise.rs"),
            "the broad directory pattern must still exclude noise.rs, got: {paths:?}"
        );
    }

    /// The first review of a repo without a `.reviewignore` auto-generates the
    /// default (with `.kanban/`); a second review preserves a user-edited file
    /// byte-for-byte.
    #[tokio::test]
    async fn scope_review_autogenerates_reviewignore_then_preserves_edits() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn base() {}\n");
        repo.commit("initial");

        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);

        scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();
        let generated = std::fs::read_to_string(repo.path().join(".reviewignore"))
            .expect("the first review must auto-generate .reviewignore");
        assert!(
            generated.contains(".kanban/"),
            "the generated default must ignore .kanban/, got:\n{generated}"
        );

        // A user edits it; a second review must leave it byte-identical.
        let edited = "# custom rules\nvendor/\n";
        std::fs::write(repo.path().join(".reviewignore"), edited).unwrap();
        scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();
        let after = std::fs::read_to_string(repo.path().join(".reviewignore")).unwrap();
        assert_eq!(
            after, edited,
            "a second review must not rewrite the user's .reviewignore"
        );
    }

    /// A `Scope::File` naming an ignored path resolves to an empty scope — no
    /// findings, no error — the same exclusion the other scopes apply.
    #[tokio::test]
    async fn file_scope_of_an_ignored_path_yields_empty_scope() {
        let repo = TestRepo::new();
        repo.write(".reviewignore", ".kanban/\n");
        repo.write(".kanban/board.md", "# board\n");
        repo.commit("initial");

        let conn = index_conn();
        let loader = loader_with("everything", "*", &[]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(
            Scope::File(".kanban/board.md".to_string()),
            repo.path(),
            &loader,
            &conn,
            &embedder,
            None,
        )
        .await
        .unwrap();

        assert!(
            work.validators.is_empty(),
            "an explicitly named ignored file must resolve to an empty scope, got: {:?}",
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

    // ---- read_working containment: reject paths escaping the repo root --

    /// A `..` traversal path must be rejected — never read — with a typed
    /// [`AvpError::Validator`] naming the full offending path. Without the
    /// containment guard, `repo_path.join("../x")` climbs out of the repo and
    /// reads an arbitrary file into the review agent's context.
    #[test]
    fn read_working_rejects_a_parent_traversal_path() {
        let repo = TestRepo::new();
        // A secret file just ABOVE the repo dir, named uniquely so a parallel
        // test or a leftover can never make this assertion flaky.
        let marker = format!(
            "outside_secret_{}.txt",
            repo.path().file_name().unwrap().to_string_lossy()
        );
        let outside = repo.path().parent().unwrap().join(&marker);
        std::fs::write(&outside, "TOP SECRET").unwrap();

        let got = read_working(repo.path(), &format!("../{marker}"));
        let _ = std::fs::remove_file(&outside);

        match got {
            Err(AvpError::Validator { message, .. }) => {
                assert!(
                    message.contains(&format!("../{marker}")),
                    "the error must carry the full offending path: {message}"
                );
                assert!(
                    message.contains("escapes the repository root"),
                    "the error must explain the escape: {message}"
                );
            }
            other => panic!("a `..` traversal must be a Validator error, got: {other:?}"),
        }
    }

    /// An absolute input path must be rejected outright: `Path::join` with an
    /// absolute argument REPLACES the repo root entirely, so a naive join reads
    /// straight from the given absolute path (e.g. `/etc/passwd`).
    #[test]
    fn read_working_rejects_an_absolute_path() {
        let repo = TestRepo::new();
        let mut secret = std::env::temp_dir();
        secret.push(format!(
            "abs_secret_{}.txt",
            repo.path().file_name().unwrap().to_string_lossy()
        ));
        std::fs::write(&secret, "TOP SECRET").unwrap();

        let got = read_working(repo.path(), secret.to_str().unwrap());
        let _ = std::fs::remove_file(&secret);

        match got {
            Err(AvpError::Validator { message, .. }) => {
                assert!(
                    message.contains(secret.to_str().unwrap()),
                    "the error must carry the full offending path: {message}"
                );
            }
            other => panic!("an absolute path must be a Validator error, got: {other:?}"),
        }
    }

    /// A repo-internal symlink whose target lies OUTSIDE the repo root must be
    /// rejected: canonicalization resolves the link to its real path, which the
    /// containment check finds is not under the root.
    #[cfg(unix)]
    #[test]
    fn read_working_rejects_a_symlink_escaping_the_repo_root() {
        let repo = TestRepo::new();
        let outside_dir = tempfile::TempDir::new().unwrap();
        let secret = outside_dir.path().join("secret.txt");
        std::fs::write(&secret, "TOP SECRET").unwrap();
        // A link living INSIDE the repo but pointing at the outside secret.
        std::os::unix::fs::symlink(&secret, repo.path().join("link.txt")).unwrap();

        let got = read_working(repo.path(), "link.txt");
        match got {
            Err(AvpError::Validator { message, .. }) => {
                assert!(
                    message.contains("link.txt"),
                    "the error must name the offending path: {message}"
                );
                assert!(
                    message.contains("escapes the repository root"),
                    "the error must explain the escape: {message}"
                );
            }
            other => panic!("an escaping symlink must be a Validator error, got: {other:?}"),
        }
    }

    /// A normal NESTED relative path under the repo root is read exactly as
    /// before the containment guard — the guard must not reject legitimate
    /// repo-relative targets.
    #[test]
    fn read_working_reads_a_nested_relative_path_unchanged() {
        let repo = TestRepo::new();
        repo.write("src/deep/nested.rs", "pub fn nested() {}\n");
        let got = read_working(repo.path(), "src/deep/nested.rs")
            .expect("a nested repo-relative file must read normally");
        assert_eq!(got.as_deref(), Some("pub fn nested() {}\n"));
    }

    /// A path absent at the requested ref resolves to `Ok(None)`.
    #[test]
    fn read_at_ref_maps_a_path_absent_at_ref_to_ok_none() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "pub fn compute() {}\n");
        repo.commit("initial");
        let git = open_repo(repo.path()).unwrap();

        let got = read_at_ref(
            &git,
            GitRefSpec::head(),
            FilePath::new("src/never_committed.rs"),
        )
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

        let got = read_at_ref(&git, GitRefSpec::head(), FilePath::new("src/lib.rs"))
            .expect("a committed blob must succeed");
        assert_eq!(got.as_deref(), Some("pub fn compute() {}\n"));
    }

    /// The two halves of a `refspec:path` blob address are DISTINCT types, so a
    /// call site cannot transpose them by accident — the compiler rejects it.
    /// This pins the order the types carry: the refspec selects the revision,
    /// the path selects the file within it, and the transposition addresses
    /// nothing.
    #[test]
    fn read_at_ref_addresses_the_path_within_the_refspec_never_the_transposition() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "pub fn compute() {}\n");
        repo.commit("initial");
        let git = open_repo(repo.path()).unwrap();

        let got = read_at_ref(&git, GitRefSpec::head(), FilePath::new("src/lib.rs"))
            .expect("the path within the refspec must read");
        assert_eq!(got.as_deref(), Some("pub fn compute() {}\n"));

        // Swapping the halves yields the meaningless spec `src/lib.rs:HEAD`,
        // whose revision half resolves to nothing — the absent-at-ref signal,
        // never this file's content.
        let transposed = read_at_ref(&git, GitRefSpec::new("src/lib.rs"), FilePath::new("HEAD"));
        assert!(
            matches!(transposed, Ok(None)),
            "a transposed refspec/path addresses nothing: {transposed:?}"
        );
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

        let err = read_at_ref(&git, GitRefSpec::head(), FilePath::new("blob.bin"))
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

    // ---- typed parameter pairs: a transposition must not compile ----------

    /// The two sides of a file's change are DISTINCT types ([`BeforeContent`]
    /// and [`AfterContent`]) carried by one named-field [`FileVersions`], so no
    /// call site can transpose them: writing
    /// `FileVersions { before: AfterContent::new(a), after: BeforeContent::new(b) }`
    /// is `error[E0308]: mismatched types`, never a silently INVERTED diff.
    /// This pins the direction each side carries into the semantic differ.
    #[test]
    fn file_change_builder_records_the_before_side_as_before_never_the_transposition() {
        let mut builder = FileChangeBuilder::new();
        builder.push(
            FilePath::new("src/lib.rs"),
            FileVersions {
                before: BeforeContent::new(Some("fn old() {}\n".to_string())),
                after: AfterContent::new(Some("fn new() {}\n".to_string())),
            },
        );
        let resolved = builder.finish(vec!["src/lib.rs".to_string()], "purpose".to_string(), None);

        let change = &resolved.file_changes[0];
        assert_eq!(change.file_path, "src/lib.rs");
        assert_eq!(change.before_content.as_deref(), Some("fn old() {}\n"));
        assert_eq!(change.after_content.as_deref(), Some("fn new() {}\n"));
        assert_eq!(change.status, FileStatus::Modified);
        // Only the AFTER side is the file's current content.
        assert_eq!(
            resolved.after_content.get("src/lib.rs").map(String::as_str),
            Some("fn new() {}\n")
        );
    }

    /// A file absent on the before side is an ADDITION; one absent on the after
    /// side is a DELETION. Transposing the two sides inverts exactly this pair
    /// — a plausible-looking but wholly wrong diff — which is why the sides are
    /// separate types.
    #[test]
    fn an_absent_before_side_is_an_addition_and_an_absent_after_side_a_deletion() {
        let mut builder = FileChangeBuilder::new();
        builder.push(
            FilePath::new("added.rs"),
            FileVersions {
                before: BeforeContent::absent(),
                after: AfterContent::new(Some("fn added() {}\n".to_string())),
            },
        );
        builder.push(
            FilePath::new("deleted.rs"),
            FileVersions {
                before: BeforeContent::new(Some("fn deleted() {}\n".to_string())),
                // A deleted file has no post-change content.
                after: AfterContent::new(None),
            },
        );
        let resolved = builder.finish(
            vec!["added.rs".to_string(), "deleted.rs".to_string()],
            "purpose".to_string(),
            None,
        );

        assert_eq!(resolved.file_changes[0].status, FileStatus::Added);
        assert_eq!(resolved.file_changes[1].status, FileStatus::Deleted);
        // A deleted file has no current content to inline.
        assert!(!resolved.after_content.contains_key("deleted.rs"));
    }

    /// A validator's rule names and probe names are DISTINCT types
    /// ([`RuleNames`], [`ProbeNames`]), so [`ValidatorWork::new`] cannot take
    /// them transposed — the compiler rejects it. This pins which list each
    /// accessor reports.
    #[test]
    fn validator_work_names_its_rules_and_probes_never_the_transposition() {
        let validator = ValidatorWork::new(
            "dedup".to_string(),
            RuleNames::new(["dedup-rule".to_string()]),
            ProbeNames::new(["similar".to_string()]),
            vec![],
        );
        assert_eq!(validator.rules(), ["dedup-rule".to_string()]);
        assert_eq!(validator.probes(), ["similar".to_string()]);
    }

    /// [`select_probe_results`] filters by PROBE NAME and attaches by changed
    /// SYMBOL. The two lists are distinct types ([`ProbeNames`] against a plain
    /// symbol slice), so they cannot be transposed; this pins which list each
    /// filter reads.
    #[test]
    fn select_probe_results_filters_by_probe_name_and_attaches_by_changed_symbol() {
        let probe = |name: &str, target: &str| ProbeResult {
            name: name.to_string(),
            kind: ProbeKind::Fact,
            target: target.to_string(),
            rows: vec![],
        };
        let cache = vec![probe("similar", "compute"), probe("callers", "compute")];
        let declared = ProbeNames::new(["similar".to_string()]);
        let changed = vec!["compute".to_string()];

        let got = select_probe_results(&cache, "src/lib.rs", &changed, &declared);
        let names: Vec<&str> = got.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["similar"],
            "only the validator's DECLARED probe is selected"
        );

        // A symbol-targeted probe never attaches to a file whose changed
        // symbols do not name that target.
        let got = select_probe_results(&cache, "src/lib.rs", &[], &declared);
        assert!(
            got.is_empty(),
            "an unrelated symbol target attaches to no file: {got:?}"
        );
    }

    /// ^t7f5fqf: the batch-scoped `<changed-set>` `duplicates` comparison must
    /// NEVER attach to an individual file's `probe_results` — it used to
    /// match every file unconditionally, which multiplied its (potentially
    /// ~1.43 MB) bytes by the batch's file count. It is shared evidence,
    /// selected once per validator by [`select_shared_probe_results`] instead.
    #[test]
    fn probe_result_for_file_never_matches_the_shared_changed_set_result() {
        let changed_set = ProbeResult {
            name: "duplicates".to_string(),
            kind: ProbeKind::Fact,
            target: "<changed-set>".to_string(),
            rows: vec![],
        };
        assert!(
            !probe_result_for_file(&changed_set, "src/lib.rs", &[]),
            "the shared changed-set result must not attach to any individual file"
        );
        assert!(
            !probe_result_for_file(&changed_set, "src/lib.rs", &["lib".to_string()]),
            "a changed symbol name must not accidentally match the <changed-set> \
             sentinel target either"
        );
    }

    /// [`select_shared_probe_results`] is the sole path a validator's
    /// batch-scoped shared evidence reaches [`ValidatorWork`] through: it
    /// selects only the DECLARED probe's `<changed-set>` result, never a
    /// per-file one, and never an undeclared probe's shared result.
    #[test]
    fn select_shared_probe_results_selects_only_the_declared_probes_changed_set_result() {
        let per_file = ProbeResult {
            name: "duplicates".to_string(),
            kind: ProbeKind::Fact,
            target: "src/a.rs".to_string(),
            rows: vec![],
        };
        let shared = ProbeResult {
            name: "duplicates".to_string(),
            kind: ProbeKind::Fact,
            target: "<changed-set>".to_string(),
            rows: vec![],
        };
        let undeclared_shared = ProbeResult {
            name: "similar".to_string(),
            kind: ProbeKind::Candidate,
            target: "<changed-set>".to_string(),
            rows: vec![],
        };
        let cache = vec![per_file, shared.clone(), undeclared_shared];
        let declared = ProbeNames::new(["duplicates".to_string()]);

        let got = select_shared_probe_results(&cache, &declared);
        assert_eq!(
            got,
            vec![shared],
            "only the declared probe's <changed-set> result is selected, never a \
             per-file result and never an undeclared probe's shared result"
        );
    }

    /// [`WorkList::shared_probe_results`] dedups the shared evidence across
    /// validators by `(probe name, target)`, so two validators that both
    /// declare `duplicates` do not double the shared block in the rendered
    /// prime.
    #[test]
    fn work_list_shared_probe_results_dedups_across_validators() {
        let shared = ProbeResult {
            name: "duplicates".to_string(),
            kind: ProbeKind::Fact,
            target: "<changed-set>".to_string(),
            rows: vec![],
        };
        let v1 = ValidatorWork::new(
            "one".to_string(),
            RuleNames::default(),
            ProbeNames::new(["duplicates".to_string()]),
            vec![],
        )
        .with_shared_probe_results(vec![shared.clone()]);
        let v2 = ValidatorWork::new(
            "two".to_string(),
            RuleNames::default(),
            ProbeNames::new(["duplicates".to_string()]),
            vec![],
        )
        .with_shared_probe_results(vec![shared.clone()]);

        let work = WorkList::new("purpose".to_string(), vec![v1, v2]);

        assert_eq!(
            work.shared_probe_results(),
            vec![shared],
            "the identical shared result declared by two validators is carried once"
        );
    }
}
