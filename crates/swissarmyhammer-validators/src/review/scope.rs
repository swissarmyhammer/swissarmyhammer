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
use std::path::Path;

use model_embedding::TextEmbedder;
use rusqlite::Connection;
use serde::Serialize;

use swissarmyhammer_git::{GitOperations, LineBlame};
use swissarmyhammer_sem::model::change::SemanticChange;
use swissarmyhammer_sem::parser::differ::compute_semantic_diff;
use swissarmyhammer_sem::parser::plugins::create_default_registry;

use swissarmyhammer_project_detection::{detect_projects, spec_for};

use crate::error::AvpError;
use crate::review::fleet::{emit_progress, ReviewProgressEvent, ReviewProgressSender};
use crate::review::probes::{
    run_probes, ChangeEntry, FileChange as ProbeChange, ProbeResult, CHANGED_SET_TARGET,
};
use crate::validators::{MatchContext, RuleSet, ValidatorLoader};

mod batch;
mod excluded;
mod fixtures;
mod resolve;

pub use batch::{batch_work_list, BatchBudget, BatchBytes, FileCapBytes, SkippedFile};
pub use excluded::{ExcludedFile, ExclusionKind};
use fixtures::split_validator_fixtures;
use resolve::*;

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

impl Scope {
    /// What this scope names as the review's subject.
    ///
    /// The op already carries the intent, so nothing else selects it: asking
    /// about the working tree or a sha is asking about a CHANGE, and asking
    /// about a path or a glob is asking about FILES.
    pub fn subject(&self) -> ReviewSubject {
        match self {
            Scope::Working | Scope::Sha(_) => ReviewSubject::Diffs,
            Scope::File(_) | Scope::Glob(_) => ReviewSubject::Files,
        }
    }

    /// The op that produced this scope, written as the caller typed it — the
    /// scope line every report names so a narrowed scope can never read as a
    /// clean result.
    ///
    /// A glob and a path both arrive on the `review file` op, so both describe
    /// as `review file`.
    pub fn describe(&self) -> String {
        match self {
            Scope::Working => "review working".to_string(),
            Scope::Sha(range) => format!("review sha {range}"),
            Scope::File(path) => format!("review file {path}"),
            Scope::Glob(pattern) => format!("review file {pattern}"),
        }
    }
}

/// What a review op names as its subject — the change, or the files.
///
/// This is the one distinction every prompt in the engine states, in the same
/// two words:
///
/// - **REVIEW** — the subject. Under [`Diffs`](ReviewSubject::Diffs) only the
///   lines the change added or modified; under [`Files`](ReviewSubject::Files)
///   every line of each named file. A finding must land on one of them.
/// - **CONSIDER** — context. Surrounding pre-existing code, read to judge the
///   subject correctly, never itself the subject of a finding. A pre-existing
///   defect is out of scope under [`Diffs`](ReviewSubject::Diffs) even when it
///   is real, and even when a validator flags it.
///
/// [`Scope::subject`] is the only thing that chooses between the two: there is
/// no argument and no tool-surface field for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReviewSubject {
    /// Review the diffs only — the lines the change added or modified.
    ///
    /// The subject of [`Scope::Working`] and [`Scope::Sha`]. Surrounding
    /// pre-existing code is rendered as context to CONSIDER, and a finding
    /// that does not land on a changed line is refuted by the verify guard.
    Diffs,
    /// Review the named files whole — every line of each.
    ///
    /// The subject of [`Scope::File`] and [`Scope::Glob`]. The caller named
    /// these files on purpose, so nothing narrows the answer: a test file
    /// reviewed this way is reviewed like any other file.
    Files,
}

impl ReviewSubject {
    /// The one-line statement of what this subject reviews, rendered into the
    /// report's scope line.
    pub fn describe(self) -> &'static str {
        match self {
            ReviewSubject::Diffs => "the diffs only — lines this change added or modified",
            ReviewSubject::Files => "the whole of each named file",
        }
    }
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
    /// The changed files the scope stage dropped before any validator paired
    /// with them, each with its reason.
    excluded: Vec<ExcludedFile>,
    /// How many files the scope stage resolved, before any exclusion.
    resolved_files: usize,
    /// What this run REVIEWS — the diffs, or the files whole.
    subject: ReviewSubject,
}

impl WorkList {
    /// Assemble a work-list from the review-level intent and the per-validator
    /// work entries, with nothing excluded.
    pub fn new(
        change_purpose: impl Into<String>,
        validators: impl IntoIterator<Item = ValidatorWork>,
    ) -> Self {
        Self {
            change_purpose: change_purpose.into(),
            validators: validators.into_iter().collect(),
            excluded: Vec::new(),
            // A hand-assembled work-list resolved nothing: it names its files
            // directly. `scope_review` always calls `with_resolved_files`.
            resolved_files: 0,
            // A hand-assembled work-list names its files directly, with no
            // change behind them — which IS the `Files` subject. The
            // production path never relies on this: `scope_review` always
            // calls `with_subject` with the op's own subject.
            subject: ReviewSubject::Files,
        }
    }

    /// Attach what this run REVIEWS, taken from the op via [`Scope::subject`].
    ///
    /// Attached after [`new`](Self::new), the way
    /// [`with_excluded`](Self::with_excluded) attaches its own once-per-run
    /// data, so every hand-built work-list keeps the plain constructor.
    pub fn with_subject(mut self, subject: ReviewSubject) -> Self {
        self.subject = subject;
        self
    }

    /// What this run REVIEWS — the diffs, or the files whole.
    ///
    /// Every prompt the engine renders reads this: it decides what a file
    /// block shows, what the output contract calls the review boundary, and
    /// whether the verify guard refutes a finding that misses the change.
    pub fn subject(&self) -> ReviewSubject {
        self.subject
    }

    /// Attach the files the scope stage dropped before any validator paired
    /// with them.
    ///
    /// Attached after [`new`](Self::new), the way
    /// [`FileWork::with_line_annotations`] attaches its own once-per-run data,
    /// so every hand-built work-list keeps the plain constructor.
    pub fn with_excluded(mut self, excluded: impl IntoIterator<Item = ExcludedFile>) -> Self {
        self.excluded = excluded.into_iter().collect();
        self
    }

    /// Attach how many files the scope stage resolved, before any exclusion.
    ///
    /// Attached after [`new`](Self::new), the way
    /// [`with_excluded`](Self::with_excluded) attaches its own once-per-run
    /// data, so every hand-built work-list keeps the plain constructor.
    pub fn with_resolved_files(mut self, resolved_files: usize) -> Self {
        self.resolved_files = resolved_files;
        self
    }

    /// How many files the scope stage resolved, before any exclusion.
    ///
    /// The denominator the report reads: a run whose exclusions cover this
    /// whole count excluded EVERY file in scope, which is a clean review that
    /// names its exclusions rather than an empty scope. Reviewed plus excluded
    /// need not reach it — a resolved file no validator matched is neither.
    pub fn resolved_files(&self) -> usize {
        self.resolved_files
    }

    /// The review-level intent.
    pub fn change_purpose(&self) -> &str {
        &self.change_purpose
    }

    /// The changed files the scope stage dropped before any validator paired
    /// with them, each with its reason.
    ///
    /// This is a RUN-level fact, so it rides on the work-list
    /// [`scope_review`] produced and not on the per-batch work-lists
    /// [`batch_work_list`] projects out of it.
    pub fn excluded(&self) -> &[ExcludedFile] {
        &self.excluded
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

/// Whether the 1-based `line` of a file with these `annotations` is a line this
/// review REVIEWS.
///
/// The single predicate behind the REVIEW/CONSIDER boundary, so the two places
/// that enforce it cannot drift: the verify guard, which refutes a fan-out
/// finding that misses the change, and the tool-finding filter, which drops a
/// deterministic tool's finding that misses it (tool findings never pass
/// through verify).
///
/// Answers `true` whenever the question cannot be decided — under
/// [`ReviewSubject::Files`], where the whole file is the subject; for a file
/// with no annotations, whose `(validator, file)` never resolved to work-list
/// context; for line `0`, which names no line; and for a line past the
/// annotated content, whose refutation belongs to the bounds check instead.
/// Every caller refutes only on a definite "not the subject".
pub fn line_is_reviewed(subject: ReviewSubject, annotations: &[LineAnnotation], line: u32) -> bool {
    if matches!(subject, ReviewSubject::Files) || annotations.is_empty() {
        return true;
    }
    let Some(index) = (line as usize).checked_sub(1) else {
        return true;
    };
    match annotations.get(index) {
        Some(annotation) => annotation.touched(),
        None => true,
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
    /// whose rendered block would exceed the review per-file cap is never trimmed
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
    // The op is the only thing that decides what this run REVIEWS. Read once,
    // here, and carried on the work-list from this point on.
    let subject = scope.subject();
    let ScopeFiles {
        resolved,
        ignored: excluded,
    } = resolve_scope_files(&scope, repo_path)?;

    // How many files this scope REACHED, read before the fixture split narrows
    // it any further: the reviewable set plus everything the ignore filter
    // already took out. It is the denominator that lets the report state a
    // FULL exclusion as a fact — a `.reviewignore` that covers the whole scope
    // is a deliberate clean review, not an empty one.
    let resolved_files = resolved.files.len() + excluded.len();

    // A validator set's own fixture data is not source: a fail fixture holds
    // the very defect its rule reports, so reviewing it makes every matching
    // rule fire on the file built to make it fire. The exclusion comes from the
    // STORE — every loaded set's `fixtures/` directory — so it leaves the
    // work-list here, before any progress event, any validator pairing, and any
    // tool-rule argument list.
    let (resolved, fixtures) = split_validator_fixtures(resolved, repo_path, loader);

    // The two deliberate exclusions share one list, ignore-excluded first, in
    // the order each stage dropped them. The report tells them apart by their
    // kind, never by their order.
    let excluded: Vec<ExcludedFile> = excluded.into_iter().chain(fixtures).collect();

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
        &before_by_path,
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

    Ok(WorkList::new(resolved.change_purpose, validator_work)
        .with_excluded(excluded)
        .with_resolved_files(resolved_files)
        .with_subject(subject))
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
pub fn detected_project_type_keys(repo_path: &Path) -> Vec<String> {
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

/// Borrow every string in `items` as a `&str`.
///
/// [`detected_project_type_keys`] answers with owned keys, and every stage
/// that consumes those keys — the tool-rule planner, the tool installer, the
/// doctor check — takes a borrowed slice. This is the one conversion between
/// the two, so no caller writes it again.
pub fn as_borrowed_strings<S: AsRef<str>>(items: &[S]) -> Vec<&str> {
    items.iter().map(S::as_ref).collect()
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
    let raw_contents = sources_under_review(matched_files, after_content);

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
        // exceed the per-file cap is never trimmed here either — [`batch_work_list`]
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

/// The subset of `content` covering the files under review, keyed by path.
///
/// The one narrowing every stage applies before it reads file text, so the
/// blame pass and the probe runner see exactly the same file set.
fn sources_under_review(
    matched_files: &BTreeSet<String>,
    content: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    matched_files
        .iter()
        .filter_map(|file| content.get(file).map(|text| (file.clone(), text.clone())))
        .collect()
}

/// Build the shared probe-result cache from a single [`run_probes`] call over the
/// whole change set with the union of every validator's declared probes.
///
/// Entity-bound probes read `change_entities`; file-bound probes read
/// `sources`, so they measure the whole review boundary rather than only the
/// entities the diff touched. The file-bound set holds the tree-sitter family,
/// which reads the current source of every matched file, and `clone-siblings`,
/// which takes the matched file set — unioned with the files the semantic diff
/// names — as the clone sites the change already reached. Diff-aware
/// tree-sitter probes also read each file's base revision, which is why
/// `before_by_path` — already computed for the blame pass — comes through here
/// rather than being read from git a second time.
async fn run_probe_cache(
    validators: &BTreeMap<String, MatchedValidator>,
    change_entities: &[ChangeEntry],
    matched_files: &BTreeSet<String>,
    after_content: &BTreeMap<String, String>,
    before_by_path: &BTreeMap<String, Option<String>>,
    conn: &Connection,
    embedder: &dyn TextEmbedder,
) -> Result<Vec<ProbeResult>, AvpError> {
    let union: BTreeSet<String> = validators
        .values()
        .flat_map(|mv| mv.probes.as_slice().iter().cloned())
        .collect();
    let sources = sources_under_review(matched_files, after_content);
    // A file-bound probe still has work when the diff produced no entities, so
    // an empty entity list alone must not short-circuit the whole cache.
    if union.is_empty() || (change_entities.is_empty() && sources.is_empty()) {
        return Ok(Vec::new());
    }
    // A file absent at the base revision (one the change added) has no before
    // content, and drops out here rather than reaching a probe as an empty file.
    let present_before: BTreeMap<String, String> = before_by_path
        .iter()
        .filter_map(|(file, content)| content.clone().map(|text| (file.clone(), text)))
        .collect();
    let before_sources = sources_under_review(matched_files, &present_before);

    let names: Vec<String> = union.into_iter().collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let change = ProbeChange::new(change_entities.to_vec())
        .with_sources(sources)
        .with_before_sources(before_sources);
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

/// Select the batch-scoped shared probe results — those bound to
/// [`CHANGED_SET_TARGET`] — a validator's declared `probes` pulls from the
/// shared single-run cache.
///
/// Two probes write that target: the `duplicates` changed-set comparison, and
/// the `clone-siblings` overlay.
///
/// Computed ONCE per validator — never once per file — because this evidence
/// spans the WHOLE change under review rather than any one file
/// (see [`probe_result_for_file`] for why the target is excluded there).
fn select_shared_probe_results(cache: &[ProbeResult], probes: &ProbeNames) -> Vec<ProbeResult> {
    select_probe_results_by(cache, probes, |r| r.target == CHANGED_SET_TARGET)
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

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_matching;
