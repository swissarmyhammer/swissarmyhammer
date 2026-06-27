//! `expect expectation create` — agent-authored specs via the doctor green-loop.
//!
//! The authoring op a coding agent drives to capture intent *at the moment it is
//! expressed* (`ideas/expect.md` §"expect expectation create" and §"Errors that
//! teach"). [`create`] reads whatever intent-bearing artifact it is pointed at (a
//! [`CreateSource`]), asks an authoring agent to draft a `*.expect.md`, and loops
//! that draft through [`diagnose`](crate::doctor::diagnose) until no field is in a
//! doctor-`Error` state — the agent patches exactly the red fields using the
//! structured per-field diagnostics, so it **cannot emit an invalid spec**. It
//! then writes the spec, records a *candidate observation*, and leaves the result
//! **unapproved** (ledger state [`New`](LedgerState::New)) for a human to confirm.
//!
//! The engine stays agent-construction-free: authoring is a [`SpecAuthor`] seam,
//! exactly like the [`GoalDriver`](crate::drive::GoalDriver) seam the driver uses.
//! Tests inject a deterministic stub; the tool layer injects a seam backed by the
//! real ACP agent (one fresh scoped session per draft, since
//! [`AcpGoalDriver`](crate::drive::AcpGoalDriver) is single-use). The agent is
//! handed the **resolved** frontmatter schema — the closed enums *plus* the live
//! `model:` set — and the authoring rules, via [`render_schema`].
//!
//! No system is driven: like `doctor`, the whole loop runs without touching the
//! SUT. The candidate observation records the authoring trajectory (never a
//! verdict source) with no checkpoints; with no golden written, the spec reads as
//! [`New`](LedgerState::New) — the human edits for intent and then approves.

use std::future::Future;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::doctor::{
    diagnose, isolation_values, surface_values, tier_values, DiagnosticStatus, DoctorFacts,
    FieldDiagnostic, KNOWN_KEYS, REQUIRED_KEYS,
};
use crate::error::ExpectError;
use crate::observe::write_received;
use crate::spec::{derive_path, EXPECT_EXTENSION};
use crate::types::{LedgerState, Observation, Trajectory};

/// The bound on the draft → doctor → repair loop.
///
/// An honest authoring agent reaches a green spec in one or two passes; this caps
/// a non-converging one so [`create`] errors (writing nothing) rather than looping
/// forever. The guarantee is terminal: either every field is green or [`create`]
/// returns an error — a draft is never left in a doctor-`Error` state on disk.
const MAX_REPAIR_ITERATIONS: usize = 5;

/// The provenance `source` label for a bare intent string.
const SOURCE_INTENT: &str = "intent";
/// The provenance `source` label for the (default, interactive) chat source.
const SOURCE_CHAT: &str = "chat";
/// The provenance `source` label for a kanban task source (`--from-task`).
const SOURCE_TASK: &str = "task";
/// The provenance `source` label for a design-doc source (`--from-spec`).
const SOURCE_SPEC: &str = "spec";
/// The provenance `source` label for a hand-verified session (`--from-session`).
const SOURCE_SESSION: &str = "session";

/// The trajectory step prefix recording the intent the candidate was authored from.
const AUTHORED_STEP_PREFIX: &str = "authored from intent: ";
/// The trajectory step prefix recording the candidate's provenance.
const PROVENANCE_STEP_PREFIX: &str = "provenance: ";

/// The JSON key the authoring agent returns the drafted file's repo-relative path
/// under, parsed by [`parse_draft`].
const DRAFT_PATH_KEY: &str = "path";
/// The JSON key the authoring agent returns the drafted markdown under.
const DRAFT_CONTENT_KEY: &str = "content";

/// The preamble framing the resolved schema handed to the authoring agent.
const SCHEMA_PREAMBLE: &str = "You are authoring a single `*.expect.md` behavioral expectation. \
It is a YAML frontmatter block followed by a markdown body. The frontmatter is a CLOSED set of keys \
(a typo fails the parser loudly), and the dynamic fields are validated against the live values below.";

/// The authoring rules handed to the agent alongside the schema (`ideas/expect.md`
/// §"expect expectation create").
const AUTHORING_RULES: &str = "Authoring rules:\n\
- Intent is mandatory: the body must state the behavior in prose, not just list mechanics.\n\
- Keep criteria bounded: 3-5 acceptance criteria, each a single observable `- [ ]` checklist item.\n\
- State the right reason: pin the behavior, not a coincidence (the 401-vs-200 defense) — a criterion \
must fail when the behavior is wrong, not merely when an unrelated value changes.\n\
- Prefer invariants over literals: assert relationships that survive incidental change \
(e.g. \"the item count equals the number of items\") over brittle exact values where you can.";

/// The forced-output instruction closing every authoring goal: the agent must
/// reply with a single JSON object [`parse_draft`] recovers.
const DRAFT_OUTPUT_INSTRUCTION: &str = "Reply with a single JSON object and nothing else, of the form \
{\"path\": \"<repo-relative path ending in .expect.md>\", \"content\": \"<the full *.expect.md markdown>\"}.";

/// The preamble framing a repair turn: the prior draft plus the red fields to fix.
const REPAIR_PREAMBLE: &str = "Your previous draft was checked by doctor and is NOT yet valid. \
Fix EXACTLY the fields flagged below — apply each suggested fix verbatim — and leave every other \
field unchanged.";

/// A source of intent the authoring pipeline drafts from.
///
/// Every variant feeds one draft → doctor → confirm pipeline; they differ only in
/// where the [`brief`](CreateSource::brief) (the text handed to the agent) comes
/// from and what [`provenance`](CreateSource::provenance) is recorded. The tool
/// layer mines each source (reading the task, the design doc, the session) and
/// hands [`create`] the resolved text, so the engine stays free of any kanban or
/// filesystem-mining dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateSource {
    /// A bare intent string.
    Intent(String),
    /// Intent mined from the interactive conversation (the default source).
    Chat(String),
    /// A kanban task's acceptance criteria; the id is recorded as provenance only,
    /// never coupling the spec to the task lifecycle.
    Task {
        /// The kanban task id, recorded as the candidate's provenance reference.
        id: String,
        /// The task's acceptance-criteria text, handed to the agent as the brief.
        criteria: String,
    },
    /// `should`/`must`/`example` text mined from a design doc / PRD; the doc path
    /// is recorded as provenance.
    Spec {
        /// The mined design-doc path, recorded as the provenance reference.
        path: String,
        /// The mined `should`/`must`/`example` text handed to the agent.
        content: String,
    },
    /// A hand-verified session transcript to capture as an expectation.
    Session(String),
}

impl CreateSource {
    /// The intent text handed to the authoring agent.
    pub fn brief(&self) -> &str {
        match self {
            CreateSource::Intent(text) | CreateSource::Chat(text) | CreateSource::Session(text) => {
                text
            }
            CreateSource::Task { criteria, .. } => criteria,
            CreateSource::Spec { content, .. } => content,
        }
    }

    /// The provenance recorded for a candidate authored from this source.
    pub fn provenance(&self) -> Provenance {
        match self {
            CreateSource::Intent(_) => Provenance::label(SOURCE_INTENT),
            CreateSource::Chat(_) => Provenance::label(SOURCE_CHAT),
            CreateSource::Task { id, .. } => Provenance::referenced(SOURCE_TASK, id.clone()),
            CreateSource::Spec { path, .. } => Provenance::referenced(SOURCE_SPEC, path.clone()),
            CreateSource::Session(_) => Provenance::label(SOURCE_SESSION),
        }
    }
}

/// Where a created expectation came from, recorded as provenance only.
///
/// The candidate's lineage (a `--from-task` id, a `--from-spec` path), kept so the
/// tool layer can link back (a kanban tag/comment) without coupling the spec to
/// the source's lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The source kind: `intent` / `chat` / `task` / `spec` / `session`.
    pub source: String,
    /// The source's identifying reference (a task id, a doc path), when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl Provenance {
    /// A provenance with only a source kind (no identifying reference).
    fn label(source: &str) -> Self {
        Provenance {
            source: source.to_string(),
            reference: None,
        }
    }

    /// A provenance with a source kind and an identifying `reference`.
    fn referenced(source: &str, reference: String) -> Self {
        Provenance {
            source: source.to_string(),
            reference: Some(reference),
        }
    }

    /// Render as `source reference` (`task 01ABC`) or just `source` when there is
    /// no reference — the form recorded in the candidate observation trajectory.
    pub fn render(&self) -> String {
        match &self.reference {
            Some(reference) => format!("{} {reference}", self.source),
            None => self.source.clone(),
        }
    }
}

/// One drafted `*.expect.md` the authoring agent produced: where it goes and what
/// it contains.
///
/// The agent picks the repo-relative path (e.g. `src/checkout/coupon.expect.md`)
/// since identity is the file location; [`create`] safe-joins it under the repo
/// root, refusing an absolute or `..`-bearing path before any write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftSpec {
    /// The repo-relative path for the file, which must end in `.expect.md`.
    pub relative_path: String,
    /// The full `*.expect.md` markdown: frontmatter + intent body + criteria.
    pub content: String,
}

/// What the authoring agent is asked to produce on one turn.
///
/// `schema` (the resolved frontmatter schema + authoring rules) is constant across
/// the loop; `brief` is the mined intent; `repair`, when present, carries the prior
/// draft and the doctor errors to fix. [`render_authoring_goal`] assembles these
/// into the prompt the production seam drives the agent with.
#[derive(Debug, Clone)]
pub struct AuthoringRequest {
    /// The resolved schema + authoring rules (from [`render_schema`]).
    pub schema: String,
    /// The mined intent text to capture.
    pub brief: String,
    /// On a repair turn, the prior draft and its red diagnostics.
    pub repair: Option<RepairContext>,
}

/// The prior draft and the doctor errors a repair turn must fix.
#[derive(Debug, Clone)]
pub struct RepairContext {
    /// The previous (rejected) draft.
    pub prior: DraftSpec,
    /// The doctor findings whose status is [`Error`](DiagnosticStatus::Error).
    pub red: Vec<FieldDiagnostic>,
}

/// The authoring agent seam: draft (or repair) a `*.expect.md` from a request.
///
/// `expect`'s authoring engine depends on this trait, not on the ACP wiring, so
/// [`create`] is testable with a deterministic stub while production drives a live
/// agent (mirroring the [`GoalDriver`](crate::drive::GoalDriver) seam). Expressed
/// as `-> impl Future` rather than `async fn` so the trait carries no implicit
/// `Send` bound — the production future is `!Send` (it runs on a current-thread
/// runtime, like the driver).
pub trait SpecAuthor {
    /// Produce a [`DraftSpec`] for `request` — an initial draft, or a repair of the
    /// prior draft when [`request.repair`](AuthoringRequest::repair) is set.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when the agent cannot be driven or its reply is not a
    /// recoverable draft.
    fn author(
        &self,
        request: &AuthoringRequest,
    ) -> impl Future<Output = Result<DraftSpec, ExpectError>>;
}

/// The result of a successful [`create`]: what was written and where it stands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateOutcome {
    /// The spec's repo-relative identity (`.expect.md` stripped).
    pub path: String,
    /// The written `*.expect.md` file.
    pub spec_file: PathBuf,
    /// The written candidate observation (the gitignored received slot).
    pub received: PathBuf,
    /// Always [`New`](LedgerState::New): authoring targets a fresh identity and
    /// writes no golden, so the candidate is left unapproved for a human to
    /// confirm. [`create`] does not consult the ledger, so this is reported, not
    /// re-derived — the caller owns not drafting over an already-approved spec.
    pub state: LedgerState,
    /// Where the candidate came from, recorded as provenance only.
    pub provenance: Provenance,
    /// How many repair turns the draft needed before it was green (0 if the first
    /// draft was already green).
    pub repair_iterations: usize,
    /// The final (green) per-field diagnostics — every finding `Ok` or `Warning`,
    /// never `Error`.
    pub diagnostics: Vec<FieldDiagnostic>,
}

/// Author one `*.expect.md` from `source`, loop it through `doctor` until no field
/// is in a doctor-`Error` state, write it plus a candidate observation, and leave
/// it unapproved (ledger state [`New`](LedgerState::New)).
///
/// The loop is the load-bearing guarantee (`ideas/expect.md` §"Errors that
/// teach"): each draft is checked by [`diagnose`] against the resolved schema and
/// the injected live [`DoctorFacts`]; any `Error` finding is fed back to the agent
/// as a [`RepairContext`] and the draft is re-authored. Only a green draft is
/// written — a non-converging one (past [`MAX_REPAIR_ITERATIONS`]) returns an
/// error having written nothing, so a spec can never be left in a doctor-`Error`
/// state on disk. No system is driven; the candidate observation records the
/// authoring trajectory (never a verdict) and, with no golden, the spec reads as
/// `new` for a human to edit and approve.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when the agent's drafted path is unsafe
/// (absolute, `..`-bearing, or not `*.expect.md`) or the draft cannot be made
/// green within the repair budget; [`ExpectError`] when the agent fails, the spec
/// file cannot be written, or the candidate observation cannot be persisted.
pub async fn create<A: SpecAuthor>(
    source: &CreateSource,
    repo_root: &Path,
    facts: &DoctorFacts,
    author: &A,
) -> Result<CreateOutcome, ExpectError> {
    let schema = render_schema(facts);
    let brief = source.brief().to_string();

    let mut repair: Option<RepairContext> = None;
    let mut repair_iterations = 0usize;

    // Draft → doctor → repair until no field is red, bounded so a non-converging
    // agent errors (writing nothing) rather than looping forever.
    let (draft, diagnostics) = loop {
        let request = AuthoringRequest {
            schema: schema.clone(),
            brief: brief.clone(),
            repair: repair.take(),
        };
        let draft = author.author(&request).await?;
        let diagnostics = diagnose(&draft.content, facts);
        let red = red_findings(&diagnostics);
        if red.is_empty() {
            break (draft, diagnostics);
        }
        if repair_iterations >= MAX_REPAIR_ITERATIONS {
            return Err(ExpectError::Expectation {
                path: draft.relative_path,
                message: format!(
                    "doctor still reports {} error field(s) after {MAX_REPAIR_ITERATIONS} repair \
                     attempt(s); refusing to write an invalid spec",
                    red.len()
                ),
            });
        }
        repair_iterations += 1;
        repair = Some(RepairContext { prior: draft, red });
    };

    // The drafted path is agent-chosen and therefore untrusted: refuse an absolute
    // or `..`-bearing path before any write (mirrors `observe`'s safe-join).
    let spec_file = safe_spec_path(repo_root, &draft.relative_path)?;
    let identity = derive_path(&spec_file, repo_root)?;
    if let Some(parent) = spec_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&spec_file, &draft.content)?;

    let provenance = source.provenance();
    let observation = candidate_observation(&identity, source, &provenance);
    let received = write_received(repo_root, &observation)?;

    Ok(CreateOutcome {
        path: identity,
        spec_file,
        received,
        state: LedgerState::New,
        provenance,
        repair_iterations,
        diagnostics,
    })
}

/// The doctor findings whose status is [`Error`](DiagnosticStatus::Error) — the
/// fields a repair turn must fix. Warnings (e.g. a pinned model that has gone
/// missing) are not red: they do not block a green draft.
fn red_findings(diagnostics: &[FieldDiagnostic]) -> Vec<FieldDiagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.status == DiagnosticStatus::Error)
        .cloned()
        .collect()
}

/// Render the resolved frontmatter schema handed to the authoring agent: the
/// closed-enum value sets (derived from `doctor`, the single source of truth) plus
/// the live `model:` registry from `facts`, framed with the authoring rules.
pub fn render_schema(facts: &DoctorFacts) -> String {
    let models = if facts.available_models.is_empty() {
        "(none registered — omit `model:` to use the default)".to_string()
    } else {
        facts.available_models.join(", ")
    };
    format!(
        "{SCHEMA_PREAMBLE}\n\n\
         Frontmatter keys (closed set): {keys}\n\
         Required keys: {required}\n\
         surface (closed set): {surfaces}\n\
         tiers (closed subset): {tiers}\n\
         isolation (closed set): {isolations}\n\
         model (validated against the live registry): {models}\n\n\
         {AUTHORING_RULES}",
        keys = KNOWN_KEYS.join(", "),
        required = REQUIRED_KEYS.join(", "),
        surfaces = surface_values().join(" | "),
        tiers = tier_values().join(" | "),
        isolations = isolation_values().join(" | "),
    )
}

/// Assemble the prompt the production seam drives the authoring agent with: the
/// resolved schema, the brief, the repair context (when present), and the forced
/// JSON-output instruction [`parse_draft`] recovers.
///
/// Shared by the tool-layer [`GoalDriver`](crate::drive::GoalDriver)-backed
/// [`SpecAuthor`] so the prompt assembly is unit-testable in the engine.
pub fn render_authoring_goal(request: &AuthoringRequest) -> String {
    let mut goal = String::new();
    goal.push_str(&request.schema);
    goal.push_str("\n\nIntent to capture:\n");
    goal.push_str(&request.brief);
    if let Some(repair) = &request.repair {
        goal.push_str("\n\n");
        goal.push_str(REPAIR_PREAMBLE);
        goal.push_str("\n\nYour previous draft:\n");
        goal.push_str(&repair.prior.content);
        goal.push_str("\n\nDoctor flagged these fields as errors — fix exactly these:\n");
        goal.push_str(&crate::doctor::render(
            &repair.prior.relative_path,
            &repair.red,
        ));
    }
    goal.push_str("\n\n");
    goal.push_str(DRAFT_OUTPUT_INSTRUCTION);
    goal
}

/// Recover a [`DraftSpec`] from the authoring agent's structured reply (a JSON
/// object carrying `path` and `content`).
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when the reply is not an object with both
/// string `path` and `content` fields.
pub fn parse_draft(claim: &serde_json::Value) -> Result<DraftSpec, ExpectError> {
    let unexpected = || ExpectError::Expectation {
        path: "<draft>".to_string(),
        message: format!(
            "authoring agent reply must be a JSON object with string `{DRAFT_PATH_KEY}` and \
             `{DRAFT_CONTENT_KEY}` fields"
        ),
    };
    let relative_path = claim
        .get(DRAFT_PATH_KEY)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(unexpected)?;
    let content = claim
        .get(DRAFT_CONTENT_KEY)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(unexpected)?;
    Ok(DraftSpec {
        relative_path: relative_path.to_string(),
        content: content.to_string(),
    })
}

/// Build the candidate observation for a freshly authored spec: an authoring
/// trajectory (the intent and its provenance) with no checkpoints — the system was
/// not driven, so there is no authoritative state, only the lineage.
fn candidate_observation(
    identity: &str,
    source: &CreateSource,
    provenance: &Provenance,
) -> Observation {
    let steps = vec![
        format!("{AUTHORED_STEP_PREFIX}{}", source.brief()),
        format!("{PROVENANCE_STEP_PREFIX}{}", provenance.render()),
    ];
    Observation {
        path: identity.to_string(),
        checkpoints: Vec::new(),
        trajectory: Trajectory { steps },
    }
}

/// Safe-join the agent-chosen `relative` spec path under `repo_root`, refusing any
/// path that could escape the repo or is not a `*.expect.md` file.
///
/// The drafted path is untrusted (the agent named it), so an absolute path or a
/// `..` component would let a write land outside the repository. Following the
/// safe-join approach in [`crate::observe`] and [`crate::surface::cli`], the path
/// is accepted only when it is relative, free of parent-directory components, and
/// carries the `.expect.md` extension.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when `relative` is absolute, contains a
/// `..` component, or does not end in `.expect.md`.
fn safe_spec_path(repo_root: &Path, relative: &str) -> Result<PathBuf, ExpectError> {
    let candidate = Path::new(relative);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ExpectError::Expectation {
            path: relative.to_string(),
            message: "drafted spec path must be a relative path without `..` components"
                .to_string(),
        });
    }
    if !relative.ends_with(EXPECT_EXTENSION) {
        return Err(ExpectError::Expectation {
            path: relative.to_string(),
            message: format!("drafted spec path must end with `{EXPECT_EXTENSION}`"),
        });
    }
    Ok(repo_root.join(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::Mutex;

    use tempfile::TempDir;

    /// The live model registry the authoring tests validate the schema against.
    fn facts() -> DoctorFacts {
        DoctorFacts {
            available_models: vec![
                "claude-sonnet-4-6".to_string(),
                "qwen-coder-flash".to_string(),
            ],
            known_setup_commands: None,
        }
    }

    /// The repo-relative path the stub drafts target.
    const DRAFT_PATH: &str = "src/checkout/coupon.expect.md";

    /// A doctor-green draft: required fields present, valid surface, stated intent,
    /// bounded checkable criteria, no pinned model — every finding is `Ok`.
    const GOOD_CONTENT: &str = "---\n\
        description: a valid coupon reduces the order total by its discount, once\n\
        surface: cli\n\
        ---\n\
        \n\
        When a shopper applies a valid coupon the displayed total drops by the discount, and \
        applying the same coupon again does not stack.\n\
        \n\
        ## Then\n\
        - [ ] after the first apply, the total is $40\n\
        - [ ] after a second apply, the total is still $40\n";

    /// A deliberately-bad draft: an unknown frontmatter key the doctor rejects (an
    /// `Error`), so the green-loop must repair it.
    const BAD_CONTENT: &str = "---\n\
        description: a typo in the surface key\n\
        surfce: cli\n\
        ---\n\
        \n\
        When a shopper applies a coupon the displayed total drops.\n\
        \n\
        ## Then\n\
        - [ ] after the first apply, the total is $40\n";

    /// Build a [`DraftSpec`] at [`DRAFT_PATH`] carrying `content`.
    fn draft(content: &str) -> DraftSpec {
        DraftSpec {
            relative_path: DRAFT_PATH.to_string(),
            content: content.to_string(),
        }
    }

    /// A deterministic authoring stub: returns its scripted drafts in order, then
    /// repeats the last one, recording every [`AuthoringRequest`] it was handed so
    /// tests can assert what the agent saw (the schema, the repair context).
    struct ScriptedAuthor {
        drafts: Mutex<VecDeque<DraftSpec>>,
        last: Mutex<Option<DraftSpec>>,
        requests: Mutex<Vec<AuthoringRequest>>,
    }

    impl ScriptedAuthor {
        fn new(drafts: Vec<DraftSpec>) -> Self {
            ScriptedAuthor {
                drafts: Mutex::new(drafts.into()),
                last: Mutex::new(None),
                requests: Mutex::new(Vec::new()),
            }
        }

        /// The requests the stub was handed, in call order.
        fn requests(&self) -> Vec<AuthoringRequest> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl SpecAuthor for ScriptedAuthor {
        fn author(
            &self,
            request: &AuthoringRequest,
        ) -> impl Future<Output = Result<DraftSpec, ExpectError>> {
            self.requests.lock().unwrap().push(request.clone());
            let next = self.drafts.lock().unwrap().pop_front();
            let draft = match next {
                Some(draft) => {
                    *self.last.lock().unwrap() = Some(draft.clone());
                    draft
                }
                None => self
                    .last
                    .lock()
                    .unwrap()
                    .clone()
                    .expect("scripted author called before any draft was scripted"),
            };
            async move { Ok(draft) }
        }
    }

    /// Load the candidate observation [`create`] wrote at `path`.
    fn load_observation(path: &Path) -> Observation {
        serde_json::from_str(&std::fs::read_to_string(path).expect("received file")).expect("json")
    }

    /// Whether a set of diagnostics is doctor-green (no `Error` finding).
    fn is_green(diagnostics: &[FieldDiagnostic]) -> bool {
        diagnostics
            .iter()
            .all(|d| d.status != DiagnosticStatus::Error)
    }

    #[tokio::test]
    async fn create_from_intent_produces_a_green_spec_in_new_state_with_a_candidate_observation() {
        let repo = TempDir::new().unwrap();
        let author = ScriptedAuthor::new(vec![draft(GOOD_CONTENT)]);
        let source = CreateSource::Intent(
            "a valid coupon reduces the order total by its discount, once".to_string(),
        );

        let outcome = create(&source, repo.path(), &facts(), &author)
            .await
            .expect("create succeeds from an intent string");

        // Left unapproved: ledger state `new`, no repair needed.
        assert_eq!(outcome.state, LedgerState::New);
        assert_eq!(outcome.repair_iterations, 0);
        assert_eq!(outcome.path, "src/checkout/coupon");

        // The spec file was written and is doctor-green.
        let written = std::fs::read_to_string(&outcome.spec_file).expect("spec written");
        assert!(
            is_green(&diagnose(&written, &facts())),
            "the written spec must be doctor-green"
        );
        assert!(is_green(&outcome.diagnostics));

        // A candidate observation was recorded under the received slot.
        assert!(
            outcome.received.is_file(),
            "candidate observation persisted"
        );
        let observation = load_observation(&outcome.received);
        assert_eq!(observation.path, "src/checkout/coupon");
        assert!(
            observation.checkpoints.is_empty(),
            "the candidate observation drives nothing"
        );
        assert!(
            observation
                .trajectory
                .steps
                .iter()
                .any(|step| step.starts_with(AUTHORED_STEP_PREFIX)),
            "the candidate records the authoring trajectory: {:?}",
            observation.trajectory.steps
        );
        assert_eq!(outcome.provenance.source, SOURCE_INTENT);
    }

    #[tokio::test]
    async fn create_repairs_a_bad_first_draft_to_green() {
        let repo = TempDir::new().unwrap();
        // First draft is malformed (unknown key); the second is green.
        let author = ScriptedAuthor::new(vec![draft(BAD_CONTENT), draft(GOOD_CONTENT)]);
        let source = CreateSource::Intent("a coupon reduces the total".to_string());

        let outcome = create(&source, repo.path(), &facts(), &author)
            .await
            .expect("the green-loop repairs the bad draft");

        assert_eq!(outcome.repair_iterations, 1, "exactly one repair turn");
        assert!(
            is_green(&outcome.diagnostics),
            "the final draft is doctor-green"
        );
        let written = std::fs::read_to_string(&outcome.spec_file).expect("spec written");
        assert!(is_green(&diagnose(&written, &facts())));

        // The repair turn handed the agent the prior draft and the red `surfce`
        // diagnostic — it patched exactly the flagged field.
        let requests = author.requests();
        assert_eq!(requests.len(), 2, "an initial draft plus one repair");
        let repair = requests[1]
            .repair
            .as_ref()
            .expect("the second turn is a repair turn");
        assert!(
            repair.red.iter().any(|d| d.field == "surfce"),
            "the repair context names the red field: {:?}",
            repair.red
        );
    }

    #[tokio::test]
    async fn create_from_task_drafts_from_criteria_and_records_provenance() {
        let repo = TempDir::new().unwrap();
        let author = ScriptedAuthor::new(vec![draft(GOOD_CONTENT)]);
        const TASK_ID: &str = "01KW26ART916Q6N6JX037Q4QSX";
        const CRITERIA: &str = "the coupon should only apply once and reduce the total";
        let source = CreateSource::Task {
            id: TASK_ID.to_string(),
            criteria: CRITERIA.to_string(),
        };

        let outcome = create(&source, repo.path(), &facts(), &author)
            .await
            .expect("create drafts from a task's criteria");

        // Provenance links back to the task without coupling to its lifecycle.
        assert_eq!(outcome.provenance.source, SOURCE_TASK);
        assert_eq!(outcome.provenance.reference.as_deref(), Some(TASK_ID));
        assert_eq!(outcome.state, LedgerState::New);

        // The agent was handed the task's criteria as the brief, and the candidate
        // records the task provenance in its trajectory.
        assert_eq!(author.requests()[0].brief, CRITERIA);
        let observation = load_observation(&outcome.received);
        assert!(
            observation
                .trajectory
                .steps
                .iter()
                .any(|step| step == &format!("{PROVENANCE_STEP_PREFIX}{SOURCE_TASK} {TASK_ID}")),
            "the candidate records the task provenance: {:?}",
            observation.trajectory.steps
        );
    }

    #[tokio::test]
    async fn create_refuses_to_write_a_spec_it_cannot_make_green() {
        let repo = TempDir::new().unwrap();
        // The stub never fixes the draft: the green-loop must exhaust its budget and
        // error, having written nothing — a spec is never left in an Error state.
        let author = ScriptedAuthor::new(vec![draft(BAD_CONTENT)]);
        let source = CreateSource::Intent("a coupon reduces the total".to_string());

        let err = create(&source, repo.path(), &facts(), &author)
            .await
            .expect_err("a never-green draft must error");
        assert!(
            matches!(err, ExpectError::Expectation { .. }),
            "got {err:?}"
        );

        // Nothing was written: no spec file, no candidate observation.
        assert!(
            !repo.path().join(DRAFT_PATH).exists(),
            "no invalid spec is written"
        );
        assert!(
            !repo.path().join(".expect/received").exists(),
            "no candidate observation is written for a failed create"
        );
    }

    #[tokio::test]
    async fn create_refuses_a_drafted_path_that_escapes_the_repo() {
        let repo = TempDir::new().unwrap();
        let escaping = DraftSpec {
            relative_path: "../escape.expect.md".to_string(),
            content: GOOD_CONTENT.to_string(),
        };
        let author = ScriptedAuthor::new(vec![escaping]);
        let source = CreateSource::Intent("a coupon reduces the total".to_string());

        let err = create(&source, repo.path(), &facts(), &author)
            .await
            .expect_err("a `..`-escaping drafted path must be refused");
        assert!(
            matches!(err, ExpectError::Expectation { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn render_schema_hands_the_agent_the_live_models_and_closed_enums() {
        let facts = facts();
        let schema = render_schema(&facts);

        // The live model set is present (the resolved dynamic field).
        for model in &facts.available_models {
            assert!(schema.contains(model), "schema must list model `{model}`");
        }
        // The closed enum sets are present, asserted against doctor's own values
        // (the single source of truth) rather than re-typed literals.
        for surface in surface_values() {
            assert!(
                schema.contains(&surface),
                "schema must list surface `{surface}`"
            );
        }
        for key in REQUIRED_KEYS {
            assert!(
                schema.contains(key),
                "schema must name required key `{key}`"
            );
        }
        // The authoring rules travel with the schema.
        assert!(schema.contains("Intent is mandatory"));
    }

    #[test]
    fn render_authoring_goal_carries_schema_brief_and_repair() {
        let schema = render_schema(&facts());
        let red = red_findings(&diagnose(BAD_CONTENT, &facts()));
        let request = AuthoringRequest {
            schema: schema.clone(),
            brief: "capture the coupon behavior".to_string(),
            repair: Some(RepairContext {
                prior: draft(BAD_CONTENT),
                red,
            }),
        };

        let goal = render_authoring_goal(&request);
        assert!(
            goal.contains(&schema),
            "the goal carries the resolved schema"
        );
        assert!(
            goal.contains("capture the coupon behavior"),
            "and the brief"
        );
        assert!(goal.contains("surfce"), "and the red field to repair");
        assert!(
            goal.contains(DRAFT_OUTPUT_INSTRUCTION),
            "and the forced JSON-output instruction"
        );
    }

    #[test]
    fn parse_draft_recovers_path_and_content() {
        let claim = serde_json::json!({
            "path": DRAFT_PATH,
            "content": GOOD_CONTENT,
        });
        let parsed = parse_draft(&claim).expect("a well-formed reply parses");
        assert_eq!(parsed.relative_path, DRAFT_PATH);
        assert_eq!(parsed.content, GOOD_CONTENT);
    }

    #[test]
    fn parse_draft_rejects_a_reply_missing_fields() {
        let claim = serde_json::json!({ "path": DRAFT_PATH });
        assert!(
            parse_draft(&claim).is_err(),
            "a reply without `content` is rejected"
        );
    }

    #[test]
    fn safe_spec_path_rejects_traversal_absolute_and_non_expect_paths() {
        let repo = Path::new("/repo");
        for bad in [
            "../escape.expect.md",
            "/etc/passwd.expect.md",
            "src/nested/../../escape.expect.md",
            "src/notes.md",
        ] {
            assert!(
                safe_spec_path(repo, bad).is_err(),
                "`{bad}` must be refused"
            );
        }
        let ok = safe_spec_path(repo, DRAFT_PATH).expect("a safe relative path is accepted");
        assert_eq!(ok, repo.join(DRAFT_PATH));
    }
}
