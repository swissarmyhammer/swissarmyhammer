//! Operation-based `expect` MCP tool.
//!
//! The single op-dispatched surface for the expectation feature, modeled on the
//! `diagnostics` and `review` tools: one tool named `expect` that maps `op` →
//! action. Each op is one cell of the domain grid in `ideas/expect.md`, read
//! left-to-right as `<verb> <noun>`; the CLI renders the grammar noun-first
//! (`expect expectation check`, `expect expectations list`) via
//! [`swissarmyhammer_operations::cli_gen`].
//!
//! ## Ops
//!
//! | noun | verbs |
//! |------|-------|
//! | `expectation` | `create`, `get`, `delete`, `observe`, `check` |
//! | `expectations` | `list`, `observe`, `check` |
//! | `observation` | `get`, `delete`, `evaluate`, `approve` |
//! | `observations` | `list`, `evaluate`, `approve` |
//! | `golden` | `get`, `delete`, `evaluate` |
//! | `goldens` | `list`, `evaluate` |
//! | `surface` | `get` |
//! | `surfaces` | `list` |
//!
//! The read-only `surface` / `surfaces` ops serve the static surface adapter
//! catalog ([`swissarmyhammer_expect::surfaces`]), and `observe expectation` /
//! `observe expectations` resolve a scope and run the engine's
//! [`observe`](swissarmyhammer_expect::observe) loop, persisting each received
//! observation under `.expect/received/`. Every other op is still a stub that
//! dispatches to a structured "not implemented yet" payload. The remaining real
//! implementations (and their parameters, doctor pass, evaluate/compare
//! machinery) land in later tasks, which replace these stubs and the placeholder
//! [`Doctorable`](swissarmyhammer_common::health::Doctorable) /
//! [`Initializable`](crate::mcp::tool_registry::Initializable) impls.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, operation, Operation, ParamMeta, ParamType,
    SchemaConfig,
};

use swissarmyhammer_expect::{
    approval_diff, approval_status, approve, check, decide_approval, evaluate_spec,
    find_expect_dir, golden_path, ledger_entry, ledger_queue, observe, observe_repeated,
    read_golden, received_path, surfaces, write_golden, write_received, ApprovalDecision,
    ApprovalStatus, ApproveMode, CheckOptions, CliAdapter, CreateSource, ExpectConfig, ExpectError,
    Expectation, ExpectationLoader, Golden, GradingPins, Observation, ObserveConfig, ScrubberSet,
    Surface,
};
use swissarmyhammer_kanban::{comment::AddComment, task::GetTask, Execute, KanbanContext};
use swissarmyhammer_validators::PoolConfig;

use crate::mcp::op_tool_helpers::{bool_arg, json_result, string_arg};
use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};

/// The tool's registered name and its `cli_category` (the top-level `sah`
/// subcommand the noun-first command tree hangs under).
const EXPECT_TOOL_NAME: &str = "expect";

/// The `status` field every stub op returns until its real implementation lands.
/// Tests assert against this constant rather than re-typing the literal.
const NOT_IMPLEMENTED_STATUS: &str = "not_implemented";

// ---------------------------------------------------------------------------
// Operations (one zero-sized struct per `<verb> <noun>` grid cell). Parameters
// are added when each op gains its real implementation; the still-skeleton ops
// are parameterless and dispatch to the not-implemented placeholder, while the
// implemented `get surface` declares its `name` parameter.
// ---------------------------------------------------------------------------

/// `create expectation` — draft a new expectation spec via the doctor green-loop.
///
/// A manual [`Operation`] impl (rather than the `#[operation]` macro) so it can
/// declare the [`CREATE_PARAMS`] source inputs the authoring pipeline resolves.
#[derive(Debug, Default)]
pub struct ExpectationCreate;

/// The source inputs of `create expectation`: a bare `intent` string, or one of
/// the `--from-*` sources. All feed one draft → doctor → confirm pipeline; they
/// differ only in where the intent is mined from and what provenance is recorded.
static CREATE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("intent")
        .description("A bare intent string to capture as an expectation.")
        .param_type(ParamType::String),
    ParamMeta::new("from_task")
        .description(
            "Draft from a kanban task's acceptance criteria; recorded as provenance only, not \
             coupled to the task lifecycle.",
        )
        .param_type(ParamType::String),
    ParamMeta::new("from_spec")
        .description(
            "Draft from a design doc / PRD (a repo-relative path), mining should/must/example.",
        )
        .param_type(ParamType::String),
    ParamMeta::new("from_session")
        .description("Capture a hand-verified run described by this text.")
        .param_type(ParamType::String),
    ParamMeta::new("from_chat")
        .description("Draft from conversation-mined intent text (the default interactive source).")
        .param_type(ParamType::String),
];

impl Operation for ExpectationCreate {
    fn verb(&self) -> &'static str {
        "create"
    }
    fn noun(&self) -> &'static str {
        "expectation"
    }
    fn description(&self) -> &'static str {
        "Draft a new expectation spec from intent and loop it through doctor until valid"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        CREATE_PARAMS
    }
}

/// `get expectation` — read one expectation spec.
///
/// A manual [`Operation`] impl (rather than the `#[operation]` macro) so it can
/// declare the [`SCOPE_PARAMS`] scope/tag inputs it resolves through
/// [`ExpectationLoader::resolve_scope`].
#[derive(Debug, Default)]
pub struct ExpectationGet;

impl Operation for ExpectationGet {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "expectation"
    }
    fn description(&self) -> &'static str {
        "Get one expectation spec (frontmatter, intent, criteria, and Given/When/Then)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `delete expectation` — remove a spec and its observation and golden.
#[operation(
    verb = "delete",
    noun = "expectation",
    description = "Delete an expectation spec and its observation and golden"
)]
#[derive(Debug, Default)]
pub struct ExpectationDelete;

/// The `scope` / `tag` parameters shared by every op that resolves a `<scope>`
/// through [`ExpectationLoader::resolve_scope`] — `observe`/`observations` and
/// the `evaluate` ops alike. Declared once so the scope grammar cannot drift
/// across ops; each op carries an identical parameter set from this source.
static SCOPE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("scope")
        .description(
            "The expectation scope: a spec path, a folder, or a glob. Omit to select every spec.",
        )
        .param_type(ParamType::String),
    ParamMeta::new("tag")
        .description("Narrow the scope to specs carrying this tag.")
        .param_type(ParamType::String),
];

/// `observe expectation` — drive the system and capture an observation.
///
/// A manual [`Operation`] impl (rather than the `#[operation]` macro) so it can
/// declare the [`SCOPE_PARAMS`] scope/tag inputs, mirroring [`SurfaceGet`].
#[derive(Debug, Default)]
pub struct ExpectationObserve;

impl Operation for ExpectationObserve {
    fn verb(&self) -> &'static str {
        "observe"
    }
    fn noun(&self) -> &'static str {
        "expectation"
    }
    fn description(&self) -> &'static str {
        "Drive the system and capture an observation for one expectation"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `check expectation` — doctor, observe, evaluate, and compare one expectation.
///
/// A manual [`Operation`] impl (rather than the `#[operation]` macro) so it can
/// declare the [`SCOPE_PARAMS`] scope/tag inputs the check resolves through
/// [`ExpectationLoader::discover_raw`].
#[derive(Debug, Default)]
pub struct ExpectationCheck;

impl Operation for ExpectationCheck {
    fn verb(&self) -> &'static str {
        "check"
    }
    fn noun(&self) -> &'static str {
        "expectation"
    }
    fn description(&self) -> &'static str {
        "Doctor, observe, evaluate, and compare one expectation"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `list expectations` — survey every expectation with its ledger state.
///
/// A manual [`Operation`] impl (rather than the `#[operation]` macro) so it can
/// declare the [`SCOPE_PARAMS`] scope/tag inputs that narrow the survey through
/// [`ExpectationLoader::resolve_scope`], mirroring [`ObservationsList`] and
/// [`GoldensList`].
#[derive(Debug, Default)]
pub struct ExpectationsList;

impl Operation for ExpectationsList {
    fn verb(&self) -> &'static str {
        "list"
    }
    fn noun(&self) -> &'static str {
        "expectations"
    }
    fn description(&self) -> &'static str {
        "List every expectation with its ledger state"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `observe expectations` — capture observations for a batch of expectations.
///
/// Shares [`SCOPE_PARAMS`] with [`ExpectationObserve`]; the two differ only in
/// how many specs the scope is expected to match, not in their inputs.
#[derive(Debug, Default)]
pub struct ExpectationsObserve;

impl Operation for ExpectationsObserve {
    fn verb(&self) -> &'static str {
        "observe"
    }
    fn noun(&self) -> &'static str {
        "expectations"
    }
    fn description(&self) -> &'static str {
        "Capture observations for a batch of expectations"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `check expectations` — doctor, observe, evaluate, and compare a batch.
///
/// Shares [`SCOPE_PARAMS`] with [`ExpectationCheck`]; the two differ only in how
/// many specs the scope is expected to match.
#[derive(Debug, Default)]
pub struct ExpectationsCheck;

impl Operation for ExpectationsCheck {
    fn verb(&self) -> &'static str {
        "check"
    }
    fn noun(&self) -> &'static str {
        "expectations"
    }
    fn description(&self) -> &'static str {
        "Doctor, observe, evaluate, and compare a batch of expectations"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `get observation` — read one stored observation.
///
/// A manual [`Operation`] impl so it can declare the [`SCOPE_PARAMS`] scope/tag
/// inputs it resolves through [`ExpectationLoader::resolve_scope`].
#[derive(Debug, Default)]
pub struct ObservationGet;

impl Operation for ObservationGet {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "observation"
    }
    fn description(&self) -> &'static str {
        "Get one stored observation (checkpoint timeline + trajectory)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `delete observation` — remove a stored observation.
#[operation(
    verb = "delete",
    noun = "observation",
    description = "Delete a stored observation"
)]
#[derive(Debug, Default)]
pub struct ObservationDelete;

/// `evaluate observation` — re-judge a stored observation (no re-run).
///
/// A manual [`Operation`] impl so it can declare the [`SCOPE_PARAMS`] scope/tag
/// inputs it resolves through [`ExpectationLoader::resolve_scope`].
#[derive(Debug, Default)]
pub struct ObservationEvaluate;

impl Operation for ObservationEvaluate {
    fn verb(&self) -> &'static str {
        "evaluate"
    }
    fn noun(&self) -> &'static str {
        "observation"
    }
    fn description(&self) -> &'static str {
        "Re-judge a stored observation against its criteria without re-running the system"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// The `scope`/`tag` scope inputs plus the `--missing`/`--changed`/`--all` mode
/// flags shared by `approve observation` and `approve observations`.
///
/// Built from [`SCOPE_PARAMS`] (so the scope grammar stays single-sourced) with
/// the three boolean mode flags appended — the granular selection mirroring
/// snapshot testing's `--update-snapshots`.
static APPROVE_PARAMS: Lazy<Vec<ParamMeta>> = Lazy::new(|| {
    let mut params = SCOPE_PARAMS.to_vec();
    params.extend([
        ParamMeta::new("missing")
            .description("Approve only brand-new expectations that have no golden yet.")
            .param_type(ParamType::Boolean),
        ParamMeta::new("changed")
            .description("Approve only expectations whose received run drifted from the golden.")
            .param_type(ParamType::Boolean),
        ParamMeta::new("all")
            .description("Approve every in-scope expectation that needs approval (new or drifted).")
            .param_type(ParamType::Boolean),
    ]);
    params
});

/// `approve observation` — promote a stored observation to its golden baseline.
///
/// A manual [`Operation`] impl (rather than the `#[operation]` macro) so it can
/// declare the [`APPROVE_PARAMS`] scope inputs and mode flags.
#[derive(Debug, Default)]
pub struct ObservationApprove;

impl Operation for ObservationApprove {
    fn verb(&self) -> &'static str {
        "approve"
    }
    fn noun(&self) -> &'static str {
        "observation"
    }
    fn description(&self) -> &'static str {
        "Promote a stored observation to its golden baseline, freezing its compiled assertions"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        APPROVE_PARAMS.as_slice()
    }
}

/// `list observations` — survey stored observations.
///
/// A manual [`Operation`] impl so it can declare the [`SCOPE_PARAMS`] scope/tag
/// inputs that narrow the survey through [`ExpectationLoader::resolve_scope`].
#[derive(Debug, Default)]
pub struct ObservationsList;

impl Operation for ObservationsList {
    fn verb(&self) -> &'static str {
        "list"
    }
    fn noun(&self) -> &'static str {
        "observations"
    }
    fn description(&self) -> &'static str {
        "List the specs that carry a stored received observation"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `evaluate observations` — re-judge a batch of stored observations (no re-run).
///
/// Shares [`SCOPE_PARAMS`] with [`ObservationEvaluate`]; differs only in how many
/// specs the scope is expected to match.
#[derive(Debug, Default)]
pub struct ObservationsEvaluate;

impl Operation for ObservationsEvaluate {
    fn verb(&self) -> &'static str {
        "evaluate"
    }
    fn noun(&self) -> &'static str {
        "observations"
    }
    fn description(&self) -> &'static str {
        "Re-judge a batch of stored observations without re-running the system"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `approve observations` — promote a batch of observations to their goldens.
///
/// Shares [`APPROVE_PARAMS`] with [`ObservationApprove`]; differs only in how many
/// specs the scope is expected to match.
#[derive(Debug, Default)]
pub struct ObservationsApprove;

impl Operation for ObservationsApprove {
    fn verb(&self) -> &'static str {
        "approve"
    }
    fn noun(&self) -> &'static str {
        "observations"
    }
    fn description(&self) -> &'static str {
        "Promote a batch of stored observations to their golden baselines"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        APPROVE_PARAMS.as_slice()
    }
}

/// `get golden` — read one approved golden baseline.
///
/// A manual [`Operation`] impl so it can declare the [`SCOPE_PARAMS`] scope/tag
/// inputs it resolves through [`ExpectationLoader::resolve_scope`].
#[derive(Debug, Default)]
pub struct GoldenGet;

impl Operation for GoldenGet {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "golden"
    }
    fn description(&self) -> &'static str {
        "Get one approved golden baseline (scrubbed observation, frozen assertions, grading pins)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `delete golden` — remove a golden baseline.
#[operation(
    verb = "delete",
    noun = "golden",
    description = "Delete a golden baseline"
)]
#[derive(Debug, Default)]
pub struct GoldenDelete;

/// `evaluate golden` — re-grade a golden baseline (no re-run).
///
/// A manual [`Operation`] impl so it can declare the [`SCOPE_PARAMS`] scope/tag
/// inputs it resolves through [`ExpectationLoader::resolve_scope`].
#[derive(Debug, Default)]
pub struct GoldenEvaluate;

impl Operation for GoldenEvaluate {
    fn verb(&self) -> &'static str {
        "evaluate"
    }
    fn noun(&self) -> &'static str {
        "golden"
    }
    fn description(&self) -> &'static str {
        "Re-grade a golden baseline against edited criteria without re-running the system"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `list goldens` — survey approved golden baselines.
///
/// A manual [`Operation`] impl so it can declare the [`SCOPE_PARAMS`] scope/tag
/// inputs that narrow the survey through [`ExpectationLoader::resolve_scope`].
#[derive(Debug, Default)]
pub struct GoldensList;

impl Operation for GoldensList {
    fn verb(&self) -> &'static str {
        "list"
    }
    fn noun(&self) -> &'static str {
        "goldens"
    }
    fn description(&self) -> &'static str {
        "List the specs that carry an approved golden baseline"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `evaluate goldens` — re-grade a batch of golden baselines (no re-run).
///
/// Shares [`SCOPE_PARAMS`] with [`GoldenEvaluate`]; differs only in how many specs
/// the scope is expected to match.
#[derive(Debug, Default)]
pub struct GoldensEvaluate;

impl Operation for GoldensEvaluate {
    fn verb(&self) -> &'static str {
        "evaluate"
    }
    fn noun(&self) -> &'static str {
        "goldens"
    }
    fn description(&self) -> &'static str {
        "Re-grade a batch of golden baselines without re-running the system"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SCOPE_PARAMS
    }
}

/// `get surface` — read one surface adapter from the catalog.
///
/// Unlike the skeleton stubs, this op has a real implementation and so declares
/// its `name` parameter (mirroring the `diagnostics` tool's manual op impls).
#[derive(Debug, Default)]
pub struct SurfaceGet;

/// The `name` parameter of `get surface`: which adapter to read. Required — the
/// op resolves exactly one named entry from the closed surface set.
static SURFACE_GET_PARAMS: &[ParamMeta] = &[ParamMeta::new("name")
    .description("The surface adapter to read (one of cli/http/browser/gui/file/db).")
    .param_type(ParamType::String)
    .required()];

impl Operation for SurfaceGet {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "surface"
    }
    fn description(&self) -> &'static str {
        "Get one surface adapter from the catalog"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SURFACE_GET_PARAMS
    }
}

/// `list surfaces` — survey the surface adapter catalog.
#[operation(
    verb = "list",
    noun = "surfaces",
    description = "List the surface adapter catalog"
)]
#[derive(Debug, Default)]
pub struct SurfacesList;

/// Every operation the `expect` tool exposes, in dispatch order. The schema
/// (wire + full), the CLI command tree, and `execute`'s dispatch all read from
/// this single list, so adding an op is one entry here plus its struct.
pub static EXPECT_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &ExpectationCreate as &dyn Operation,
        &ExpectationGet as &dyn Operation,
        &ExpectationDelete as &dyn Operation,
        &ExpectationObserve as &dyn Operation,
        &ExpectationCheck as &dyn Operation,
        &ExpectationsList as &dyn Operation,
        &ExpectationsObserve as &dyn Operation,
        &ExpectationsCheck as &dyn Operation,
        &ObservationGet as &dyn Operation,
        &ObservationDelete as &dyn Operation,
        &ObservationEvaluate as &dyn Operation,
        &ObservationApprove as &dyn Operation,
        &ObservationsList as &dyn Operation,
        &ObservationsEvaluate as &dyn Operation,
        &ObservationsApprove as &dyn Operation,
        &GoldenGet as &dyn Operation,
        &GoldenDelete as &dyn Operation,
        &GoldenEvaluate as &dyn Operation,
        &GoldensList as &dyn Operation,
        &GoldensEvaluate as &dyn Operation,
        &SurfaceGet as &dyn Operation,
        &SurfacesList as &dyn Operation,
    ]
});

// ---------------------------------------------------------------------------
// The tool.
// ---------------------------------------------------------------------------

/// The operation-based `expect` MCP tool.
///
/// Holds an optional [`AgentFactory`](expect_op::AgentFactory): every op except
/// `create expectation` works without it, but `create` drives a live authoring
/// agent and so requires one. The production server injects a factory that builds
/// the configured backend; a tool built without one (the default) returns an
/// actionable error for `create` and serves every other op.
#[derive(Default)]
pub struct ExpectTool {
    /// The live-agent factory the `create expectation` op drives, if wired.
    agent_factory: Option<expect_op::AgentFactory>,
}

impl ExpectTool {
    /// Build the tool with no agent factory — every op but `create` is served.
    pub fn new() -> Self {
        Self {
            agent_factory: None,
        }
    }

    /// Attach the live-agent factory the `create expectation` op drives.
    pub fn with_agent_factory(mut self, factory: expect_op::AgentFactory) -> Self {
        self.agent_factory = Some(factory);
        self
    }
}

/// The shared [`SchemaConfig`] for the `expect` tool's wire and full schemas, so
/// both surfaces describe the tool identically from one source.
fn expect_schema_config() -> SchemaConfig {
    SchemaConfig::new(
        "Capture, evaluate, and approve behavioral expectations against the running system, dispatched by `op`.",
    )
}

/// The structured placeholder a stub op returns: a stable `status`, the op id,
/// and a human-readable message. Replaced per-op as the real implementations
/// land.
fn not_implemented(op: &str) -> serde_json::Value {
    serde_json::json!({
        "status": NOT_IMPLEMENTED_STATUS,
        "op": op,
        "message": format!("`{op}` is not implemented yet"),
    })
}

/// The `get surface` op id (verb + noun), matched in `execute`'s dispatch.
const SURFACE_GET_OP: &str = "get surface";

/// The `list surfaces` op id (verb + noun), matched in `execute`'s dispatch.
const SURFACES_LIST_OP: &str = "list surfaces";

/// The `observe expectation` op id (verb + noun), matched in `execute`'s dispatch.
const EXPECTATION_OBSERVE_OP: &str = "observe expectation";

/// The `observe expectations` op id (verb + noun), matched in `execute`'s dispatch.
const EXPECTATIONS_OBSERVE_OP: &str = "observe expectations";

/// The lowercase wire name of a [`Surface`], derived from its serde form (the
/// source of truth) rather than a re-typed literal.
fn surface_wire_name(surface: Surface) -> String {
    serde_json::to_value(surface)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The surface adapter names, comma-separated, for error messages. Derived from
/// the catalog (the source of truth) so it can never drift from the real set.
fn surface_name_list() -> String {
    surfaces::catalog()
        .iter()
        .map(|info| surface_wire_name(info.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `get surface` — serve one surface adapter's catalog entry, resolved from the
/// required `name` argument. An absent or unknown name is a clear
/// `invalid_params` error listing the valid surfaces.
fn surface_get(
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let Some(name) = string_arg(arguments, "name") else {
        return Err(rmcp::ErrorData::invalid_params(
            format!(
                "`{SURFACE_GET_OP}` requires a `name` (one of {})",
                surface_name_list()
            ),
            None,
        ));
    };
    let surface: Surface = serde_json::from_value(serde_json::Value::String(name.clone()))
        .map_err(|_| {
            rmcp::ErrorData::invalid_params(
                format!(
                    "unknown surface `{name}`. Valid surfaces: {}",
                    surface_name_list()
                ),
                None,
            )
        })?;
    // A parsed `Surface` is a closed-enum variant the catalog always covers.
    let info = surfaces::get(surface).expect("every Surface variant has a catalog entry");
    json_result(&info)
}

/// `list surfaces` — serve the full surface adapter catalog.
fn surfaces_list() -> Result<CallToolResult, rmcp::ErrorData> {
    json_result(&surfaces::catalog())
}

/// Resolve the repo root the observe run provisions and stores against.
///
/// Prefers the git repository root enclosing the session working dir (so spec
/// identities and the `.expect/` slot are repo-relative), falling back to the
/// session root itself when the work dir is not inside a git repository.
fn observe_repo_root(context: &ToolContext) -> PathBuf {
    let session_root = context.session_root();
    swissarmyhammer_directory::find_git_repository_root_from(&session_root).unwrap_or(session_root)
}

/// Observe one expectation against its surface, returning the captured
/// [`Observation`].
///
/// Only the cli surface drives deterministically today; any other surface is a
/// clear `invalid_params` error rather than a silent mis-run.
fn observe_one(spec: &Expectation, repo_root: &Path) -> Result<Observation, rmcp::ErrorData> {
    if spec.frontmatter.surface != Surface::Cli {
        return Err(rmcp::ErrorData::invalid_params(
            format!(
                "`observe` currently supports only the cli surface, but `{}` declares `{}`",
                spec.path,
                surface_wire_name(spec.frontmatter.surface)
            ),
            None,
        ));
    }
    let adapter = CliAdapter::new(spec.frontmatter.timeout);
    let config = ObserveConfig::new(repo_root);
    observe(spec, &adapter, &config).map_err(|err| {
        rmcp::ErrorData::internal_error(format!("observing `{}` failed: {err}", spec.path), None)
    })
}

/// Shared handler for `observe expectation` and `observe expectations`: resolve
/// the `<scope>` (and optional `--tag`), observe each matching spec, persist its
/// received observation, and report what was captured.
///
/// Both ops share one body because they differ only in how many specs the scope
/// is expected to match; the singular form is just a scope that resolves to one.
fn observe_op(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (repo_root, specs) = resolve_scope_specs(arguments, context)?;

    let mut observed = Vec::with_capacity(specs.len());
    for spec in &specs {
        let observation = observe_one(spec, &repo_root)?;
        let received = write_received(&repo_root, &observation).map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!(
                    "writing received observation for `{}` failed: {err}",
                    spec.path
                ),
                None,
            )
        })?;
        observed.push(serde_json::json!({
            "path": observation.path,
            "received": received.display().to_string(),
            "checkpoints": observation.checkpoints.len(),
        }));
    }

    json_result(&serde_json::json!({
        "count": observed.len(),
        "observed": observed,
    }))
}

/// The `evaluate observation` op id (verb + noun), matched in `execute`'s dispatch.
const OBSERVATION_EVALUATE_OP: &str = "evaluate observation";

/// The `evaluate observations` op id (verb + noun), matched in `execute`'s dispatch.
const OBSERVATIONS_EVALUATE_OP: &str = "evaluate observations";

/// The `evaluate golden` op id (verb + noun), matched in `execute`'s dispatch.
const GOLDEN_EVALUATE_OP: &str = "evaluate golden";

/// The `evaluate goldens` op id (verb + noun), matched in `execute`'s dispatch.
const GOLDENS_EVALUATE_OP: &str = "evaluate goldens";

/// The `status` an evaluate op reports for a spec whose source observation file is
/// absent (a `new` expectation with no golden, or a never-observed received slot).
const MISSING_SOURCE_STATUS: &str = "missing";

/// Which stored observation an `evaluate` op grades. The source of truth differs
/// (received vs golden); the pure [`evaluate_spec`] applied to it is identical.
#[derive(Clone, Copy)]
enum EvaluateSource {
    /// The last received observation under `.expect/received/`.
    Received,
    /// The approved golden baseline under `.expect/goldens/`.
    Golden,
}

impl EvaluateSource {
    /// Resolve the stored observation path for spec `identity` under `repo_root`.
    fn path(
        self,
        repo_root: &Path,
        identity: &str,
    ) -> Result<PathBuf, swissarmyhammer_expect::ExpectError> {
        match self {
            EvaluateSource::Received => received_path(repo_root, identity),
            EvaluateSource::Golden => golden_path(repo_root, identity),
        }
    }

    /// The lowercase label naming this source in result payloads and messages.
    fn label(self) -> &'static str {
        match self {
            EvaluateSource::Received => "received",
            EvaluateSource::Golden => "golden",
        }
    }
}

/// Load the stored observation at `path`, or `Ok(None)` when the file does not
/// exist yet (handled gracefully — a `new` expectation, not an error).
fn load_observation(path: &Path) -> Result<Option<Observation>, rmcp::ErrorData> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).map_err(|err| {
        rmcp::ErrorData::internal_error(
            format!("reading observation `{}` failed: {err}", path.display()),
            None,
        )
    })?;
    let observation = serde_json::from_str(&text).map_err(|err| {
        rmcp::ErrorData::internal_error(
            format!("parsing observation `{}` failed: {err}", path.display()),
            None,
        )
    })?;
    Ok(Some(observation))
}

/// Shared handler for the four `evaluate` ops: resolve the `<scope>` (and optional
/// `--tag`), load each spec's stored `source` observation, and re-judge it with
/// the pure [`evaluate_spec`] — no system driven, no model consulted.
///
/// `observation`/`observations` grade the received slot; `golden`/`goldens`
/// re-grade the approved baseline against the current (possibly edited) criteria.
/// A spec whose source observation file is absent is reported (not errored) with
/// [`MISSING_SOURCE_STATUS`], so a `new` expectation with no golden — or a spec
/// never observed — surfaces clearly rather than aborting the batch. The golden
/// store's write side lands with the drift ledger (a later task); this op already
/// reads the [`golden_path`] that ledger will populate.
fn evaluate_op(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
    source: EvaluateSource,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (repo_root, specs) = resolve_scope_specs(arguments, context)?;

    let mut evaluated = Vec::with_capacity(specs.len());
    for spec in &specs {
        let path = source
            .path(&repo_root, &spec.path)
            .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?;
        match load_observation(&path)? {
            Some(observation) => {
                let verdict = evaluate_spec(spec, &observation);
                evaluated.push(serde_json::json!({
                    "path": spec.path,
                    "verdict": verdict,
                }));
            }
            None => evaluated.push(serde_json::json!({
                "path": spec.path,
                "status": MISSING_SOURCE_STATUS,
                "message": format!(
                    "no {} observation for `{}` at {}",
                    source.label(),
                    spec.path,
                    path.display()
                ),
            })),
        }
    }

    json_result(&serde_json::json!({
        "count": evaluated.len(),
        "source": source.label(),
        "evaluated": evaluated,
    }))
}

/// The `get expectation` op id (verb + noun), matched in `execute`'s dispatch.
const EXPECTATION_GET_OP: &str = "get expectation";

/// The `get observation` op id (verb + noun), matched in `execute`'s dispatch.
const OBSERVATION_GET_OP: &str = "get observation";

/// The `get golden` op id (verb + noun), matched in `execute`'s dispatch.
const GOLDEN_GET_OP: &str = "get golden";

/// The `list observations` op id (verb + noun), matched in `execute`'s dispatch.
const OBSERVATIONS_LIST_OP: &str = "list observations";

/// The `list goldens` op id (verb + noun), matched in `execute`'s dispatch.
const GOLDENS_LIST_OP: &str = "list goldens";

/// The `list expectations` op id (verb + noun), matched in `execute`'s dispatch.
const EXPECTATIONS_LIST_OP: &str = "list expectations";

/// Resolve the `<scope>` (and optional `--tag`) arguments to the repo root and the
/// matching specs — the shared front half of every scope-driven op (`get`, `list`,
/// `observe`, `evaluate`), declared once so the resolution cannot drift across ops.
fn resolve_scope_specs(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<(PathBuf, Vec<Expectation>), rmcp::ErrorData> {
    let scope = string_arg(arguments, "scope");
    let tag = string_arg(arguments, "tag");
    let repo_root = observe_repo_root(context);
    let loader = ExpectationLoader::new(&repo_root);
    let specs = loader
        .resolve_scope(scope.as_deref(), tag.as_deref())
        .map_err(|err| {
            rmcp::ErrorData::internal_error(format!("scope resolution failed: {err}"), None)
        })?;
    Ok((repo_root, specs))
}

/// The structured "no stored artifact yet" entry a `get` op returns for a spec
/// whose received observation or golden is absent — a clear status, not an error.
fn missing_artifact(identity: &str, label: &str, path: &Path) -> serde_json::Value {
    serde_json::json!({
        "path": identity,
        "status": MISSING_SOURCE_STATUS,
        "message": format!("no {label} artifact for `{identity}` at {}", path.display()),
    })
}

/// `expectation get <scope>` — return the parsed spec(s) the scope resolves to,
/// each carrying its frontmatter, intent, criteria, and Given/When/Then.
fn expectation_get(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (_repo_root, specs) = resolve_scope_specs(arguments, context)?;
    json_result(&serde_json::json!({
        "count": specs.len(),
        "expectations": specs,
    }))
}

/// `observation get <scope>` — return each spec's last received observation (its
/// checkpoint timeline + driver trajectory), or a missing marker when no run has
/// been observed yet.
fn observation_get(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (repo_root, specs) = resolve_scope_specs(arguments, context)?;
    let mut observations = Vec::with_capacity(specs.len());
    for spec in &specs {
        let path = received_path(&repo_root, &spec.path)
            .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?;
        match load_observation(&path)? {
            Some(observation) => observations.push(serde_json::json!({
                "path": spec.path,
                "observation": observation,
            })),
            None => observations.push(missing_artifact(&spec.path, "received", &path)),
        }
    }
    json_result(&serde_json::json!({
        "count": observations.len(),
        "observations": observations,
    }))
}

/// `golden get <scope>` — return each spec's approved golden baseline (the
/// scrubbed observation, frozen assertions, and grading pins), or a missing marker
/// when no golden has been approved yet.
fn golden_get(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (repo_root, specs) = resolve_scope_specs(arguments, context)?;
    let mut goldens = Vec::with_capacity(specs.len());
    for spec in &specs {
        match read_golden(&repo_root, &spec.path)
            .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?
        {
            Some(golden) => goldens.push(serde_json::json!({
                "path": spec.path,
                "golden": golden,
            })),
            None => {
                let path = golden_path(&repo_root, &spec.path)
                    .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?;
                goldens.push(missing_artifact(&spec.path, "golden", &path));
            }
        }
    }
    json_result(&serde_json::json!({
        "count": goldens.len(),
        "goldens": goldens,
    }))
}

/// `observations list` — survey the spec identities that carry a stored received
/// observation under `.expect/received/`.
fn observations_list(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (repo_root, specs) = resolve_scope_specs(arguments, context)?;
    let mut observations = Vec::new();
    for spec in &specs {
        let path = received_path(&repo_root, &spec.path)
            .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?;
        if path.exists() {
            observations.push(spec.path.clone());
        }
    }
    json_result(&serde_json::json!({
        "count": observations.len(),
        "observations": observations,
    }))
}

/// `goldens list` — survey the spec identities that carry an approved golden under
/// `.expect/goldens/`.
fn goldens_list(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (repo_root, specs) = resolve_scope_specs(arguments, context)?;
    let mut goldens = Vec::new();
    for spec in &specs {
        if read_golden(&repo_root, &spec.path)
            .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?
            .is_some()
        {
            goldens.push(spec.path.clone());
        }
    }
    json_result(&serde_json::json!({
        "count": goldens.len(),
        "goldens": goldens,
    }))
}

/// `expectations list` — survey every in-scope spec with its drift-ledger state,
/// ordering the unapproved-drift queue FIRST.
///
/// For each spec it loads the approved golden and last received observation,
/// classifies the row with [`ledger_entry`] (new/approved/drifted/stale), and
/// orders the rows drifted-first with [`ledger_queue`] so the survey doubles as
/// the review queue. A drifted row carries its re-derived old-vs-new comparison
/// as evidence (`ideas/expect.md` §"The Drift Ledger").
fn expectations_list(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (repo_root, specs) = resolve_scope_specs(arguments, context)?;
    let scrubbers = ScrubberSet::default_set();
    let mut entries = Vec::with_capacity(specs.len());
    for spec in &specs {
        let (golden, received) = load_golden_and_received(&repo_root, &spec.path)?;
        entries.push(ledger_entry(
            spec,
            golden.as_ref(),
            received.as_ref(),
            &scrubbers,
        ));
    }
    let entries = ledger_queue(entries);
    json_result(&serde_json::json!({
        "count": entries.len(),
        "expectations": entries,
    }))
}

// ---------------------------------------------------------------------------
// Approve: the human gate that freezes a golden, with CI refusal and granular
// modes mirroring `--update-snapshots`.
// ---------------------------------------------------------------------------

/// The `approve observation` op id (verb + noun), matched in `execute`'s dispatch.
const OBSERVATION_APPROVE_OP: &str = "approve observation";

/// The `approve observations` op id (verb + noun), matched in `execute`'s dispatch.
const OBSERVATIONS_APPROVE_OP: &str = "approve observations";

/// The environment variable that signals a CI run. Approve NEVER writes when it
/// is set to [`CI_ENABLED_VALUE`].
const CI_ENV_KEY: &str = "CI";

/// The value of [`CI_ENV_KEY`] that enables the CI gate (`CI=true`).
const CI_ENABLED_VALUE: &str = "true";

/// The message a bare `approve` (no mode flag) returns: it previews the diff and
/// requires the reviewer to re-run with an explicit mode to actually write.
const APPROVE_PREVIEW_MESSAGE: &str =
    "Preview only — re-run with --missing, --changed, or --all to write goldens.";

/// The boolean mode flags, paired with the [`ApproveMode`] each selects, in
/// precedence-free order (at most one may be set).
const APPROVE_MODE_FLAGS: &[(&str, ApproveMode)] = &[
    ("missing", ApproveMode::Missing),
    ("changed", ApproveMode::Changed),
    ("all", ApproveMode::All),
];

/// Whether this process is running under CI (`CI=true`).
///
/// Read once at the op edge and injected into the pure [`decide_approval`] policy,
/// so the policy itself never touches the ambient environment.
fn ci_enabled() -> bool {
    std::env::var(CI_ENV_KEY)
        .map(|value| value == CI_ENABLED_VALUE)
        .unwrap_or(false)
}

/// Resolve the selected [`ApproveMode`] from the `--missing`/`--changed`/`--all`
/// flags: `None` (a preview) when none is set, the matching mode when exactly one
/// is, and an `invalid_params` error when more than one is.
fn parse_approve_mode(
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<ApproveMode>, rmcp::ErrorData> {
    let selected: Vec<ApproveMode> = APPROVE_MODE_FLAGS
        .iter()
        .filter(|(flag, _)| bool_arg(arguments, flag))
        .map(|(_, mode)| *mode)
        .collect();
    match selected.as_slice() {
        [] => Ok(None),
        [mode] => Ok(Some(*mode)),
        _ => Err(rmcp::ErrorData::invalid_params(
            "choose at most one of --missing, --changed, --all".to_string(),
            None,
        )),
    }
}

/// The grading pins an approve pass freezes into each golden, from the repo's
/// `.expect/config.toml` (the documented defaults when there is none).
fn approve_grading(repo_root: &Path) -> GradingPins {
    let config = find_expect_dir(repo_root)
        .and_then(|dir| ExpectConfig::load(&dir).ok())
        .unwrap_or_default();
    GradingPins::from_config(&config)
}

/// Load both ledger artifacts for spec `identity`: its approved golden (if any)
/// and its last received observation (if any).
fn load_golden_and_received(
    repo_root: &Path,
    identity: &str,
) -> Result<(Option<Golden>, Option<Observation>), rmcp::ErrorData> {
    let golden = read_golden(repo_root, identity)
        .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?;
    let path = received_path(repo_root, identity)
        .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?;
    let received = load_observation(&path)?;
    Ok((golden, received))
}

/// Render a golden's frozen assertions as the per-criterion binding diff the
/// reviewer reads — each row shows the criterion, the rendered `value ← locator`
/// binding, and whether the locator was hand-edited.
fn render_bindings(golden: &Golden) -> Vec<serde_json::Value> {
    approval_diff(golden)
        .into_iter()
        .map(|binding| {
            serde_json::json!({
                "criterion": binding.criterion,
                "binding": binding.render(),
                "locator": binding.locator,
                "value": binding.value,
                "tier": binding.tier,
                "hand_edited": binding.hand_edited,
            })
        })
        .collect()
}

/// Shared handler for `approve observation` and `approve observations`: resolve
/// the `<scope>` (and optional `--tag`), then either preview the would-be diff (no
/// mode flag) or write the selected goldens.
fn approve_op(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let mode = parse_approve_mode(arguments)?;
    let (repo_root, specs) = resolve_scope_specs(arguments, context)?;
    let grading = approve_grading(&repo_root);
    let scrubbers = ScrubberSet::default_set();
    match mode {
        None => approve_preview(&specs, &repo_root, &grading, &scrubbers),
        Some(mode) => approve_write(&specs, &repo_root, mode, &grading, &scrubbers, ci_enabled()),
    }
}

/// Preview the approve pass: for each spec show its [`ApprovalStatus`] and, when
/// it would be approved, the would-be binding diff — but write nothing. A bare
/// `approve` is the explicit-confirmation gate.
fn approve_preview(
    specs: &[Expectation],
    repo_root: &Path,
    grading: &GradingPins,
    scrubbers: &ScrubberSet,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let mut preview = Vec::with_capacity(specs.len());
    for spec in specs {
        let (golden, received) = load_golden_and_received(repo_root, &spec.path)?;
        let status = approval_status(golden.as_ref(), received.as_ref(), scrubbers);
        let entry = match received.as_ref() {
            Some(received) if matches!(status, ApprovalStatus::New | ApprovalStatus::Drifted) => {
                match approve(spec, received, grading.clone(), golden.as_ref(), scrubbers) {
                    Ok(would_be) => serde_json::json!({
                        "path": spec.path,
                        "status": status,
                        "diff": render_bindings(&would_be),
                    }),
                    Err(err) => serde_json::json!({
                        "path": spec.path,
                        "status": status,
                        "error": err.to_string(),
                    }),
                }
            }
            _ => serde_json::json!({ "path": spec.path, "status": status }),
        };
        preview.push(entry);
    }
    json_result(&serde_json::json!({
        "requires_confirmation": true,
        "message": APPROVE_PREVIEW_MESSAGE,
        "count": preview.len(),
        "preview": preview,
    }))
}

/// Write the selected goldens for an approve pass.
///
/// Two-pass: every decision is built first, and the pass writes nothing if any
/// selected spec is refused by the CI gate ([`ApprovalDecision::RefusedInCi`]) or
/// rejected at compile (a hallucinated locator). So a CI refusal or a hallucinated
/// locator — the two reviewable failures — fails the whole pass before a single
/// golden is written; a CI refusal is a hard `invalid_params` failure. (A raw IO
/// error mid-write is not pre-screened and surfaces per spec; the decision-pass
/// guard is about reviewable refusals, not disk faults.)
fn approve_write(
    specs: &[Expectation],
    repo_root: &Path,
    mode: ApproveMode,
    grading: &GradingPins,
    scrubbers: &ScrubberSet,
    ci: bool,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let mut decisions = Vec::with_capacity(specs.len());
    for spec in specs {
        let (golden, received) = load_golden_and_received(repo_root, &spec.path)?;
        let decision = decide_approval(
            spec,
            golden.as_ref(),
            received.as_ref(),
            mode,
            grading.clone(),
            ci,
            scrubbers,
        )
        .map_err(|err| rmcp::ErrorData::invalid_params(err.to_string(), None))?;
        decisions.push((spec, decision));
    }

    let refused: Vec<&str> = decisions
        .iter()
        .filter_map(|(spec, decision)| {
            matches!(decision, ApprovalDecision::RefusedInCi { .. }).then_some(spec.path.as_str())
        })
        .collect();
    if !refused.is_empty() {
        return Err(rmcp::ErrorData::invalid_params(
            format!(
                "{CI_ENV_KEY}={CI_ENABLED_VALUE}: approve refuses to write {} expectation(s) ({}). A golden is minted locally by observe + approve, never in CI.",
                refused.len(),
                refused.join(", ")
            ),
            None,
        ));
    }

    let mut written = Vec::new();
    let mut skipped = Vec::new();
    for (spec, decision) in decisions {
        match decision {
            ApprovalDecision::Write { status, golden } => {
                let path = write_golden(repo_root, &golden).map_err(|err| {
                    rmcp::ErrorData::internal_error(
                        format!("writing golden for `{}` failed: {err}", spec.path),
                        None,
                    )
                })?;
                written.push(serde_json::json!({
                    "path": spec.path,
                    "status": status,
                    "golden": path.display().to_string(),
                    "diff": render_bindings(&golden),
                }));
            }
            ApprovalDecision::Skipped { status } => skipped.push(serde_json::json!({
                "path": spec.path,
                "status": status,
            })),
            // CI refusals failed the pass above, before any write.
            ApprovalDecision::RefusedInCi { .. } => {
                unreachable!("CI refusals fail the pass before any write")
            }
        }
    }

    json_result(&serde_json::json!({
        "count": written.len(),
        "written": written,
        "skipped": skipped,
    }))
}

// ---------------------------------------------------------------------------
// Check: the composed inner-loop / CI verb (doctor + observe + evaluate +
// compare), with the doctor gate refusing a malformed spec before any observe.
// ---------------------------------------------------------------------------

/// The `check expectation` op id (verb + noun), matched in `execute`'s dispatch.
const EXPECTATION_CHECK_OP: &str = "check expectation";

/// The `check expectations` op id (verb + noun), matched in `execute`'s dispatch.
const EXPECTATIONS_CHECK_OP: &str = "check expectations";

/// Observe one spec for `check` against the cli surface and persist its received
/// observation, returning the captured [`Observation`].
///
/// The driver seam the engine's [`check`] composition calls per well-formed spec:
/// only the cli surface drives deterministically today, so any other surface is a
/// clear [`ExpectError::Surface`] (which surfaces as a per-spec `errored` entry
/// rather than aborting the batch). Persisting the received run here is what later
/// lets `approve` promote it to a golden.
fn observe_for_check(
    spec: &Expectation,
    repo_root: &Path,
) -> Result<Vec<Observation>, ExpectError> {
    if spec.frontmatter.surface != Surface::Cli {
        return Err(ExpectError::Surface(format!(
            "`check` currently supports only the cli surface, but `{}` declares `{}`",
            spec.path,
            surface_wire_name(spec.frontmatter.surface)
        )));
    }
    let adapter = CliAdapter::new(spec.frontmatter.timeout);
    // `pass^k`/`repeat` runs observe more than once and re-arranges each run; the
    // last run is the `received` slot `approve` later promotes to a golden.
    let observations = observe_repeated(spec, &adapter, &ObserveConfig::new(repo_root))?;
    if let Some(received) = observations.last() {
        write_received(repo_root, received)?;
    }
    Ok(observations)
}

/// Shared handler for `check expectation` and `check expectations`: resolve the
/// `<scope>` (and optional `--tag`), then run the engine's [`check`] composition —
/// doctor gate → observe → evaluate → compare — over every matching spec.
///
/// The doctor pass runs first per spec and refuses to run a malformed one, so a
/// non-zero exit is never ambiguous between a bad spec and bad code. The report
/// carries a per-expectation status and a rolled-up `exit_code` the CLI maps to a
/// process exit (a malformed spec, an unmet expectation, or an unapproved drift
/// all fail; a `new` expectation fails only in CI). Both ops share one body
/// because they differ only in how many specs the scope is expected to match.
fn check_op(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let scope = string_arg(arguments, "scope");
    let tag = string_arg(arguments, "tag");
    let repo_root = observe_repo_root(context);

    let facts = doctor::production_facts();
    let config = find_expect_dir(&repo_root)
        .and_then(|dir| ExpectConfig::load(&dir).ok())
        .unwrap_or_default();
    let scrubbers = ScrubberSet::default_set();
    let options = CheckOptions {
        facts: &facts,
        config: &config,
        scrubbers: &scrubbers,
        ci: ci_enabled(),
    };

    let report = check(
        &repo_root,
        scope.as_deref(),
        tag.as_deref(),
        &options,
        |spec| observe_for_check(spec, &repo_root),
    )
    .map_err(|err| rmcp::ErrorData::internal_error(format!("check failed: {err}"), None))?;

    json_result(&report)
}

// ---------------------------------------------------------------------------
// Create: the authoring op a coding agent drives — draft a spec from intent,
// loop it through doctor until green, record a candidate observation, leave new.
// ---------------------------------------------------------------------------

/// The `create expectation` op id (verb + noun), matched in `execute`'s dispatch.
const EXPECTATION_CREATE_OP: &str = "create expectation";

/// The pool sizing for an authoring run: a single scoped session per draft.
const CREATE_POOL_WORKERS: usize = 1;

/// `create expectation` — resolve the intent source, drive the authoring agent
/// through the doctor green-loop, and leave a candidate observation (ledger state
/// `new`) for a human to confirm.
///
/// The op requires a live `factory` (authoring drives an agent); the source is one
/// of `intent` / `--from-task` / `--from-spec` / `--from-session` / `--from-chat`,
/// all feeding the one pipeline. A `--from-task` draft links back to the task as
/// provenance only — a best-effort kanban comment, never lifecycle coupling.
async fn expectation_create(
    factory: &expect_op::AgentFactory,
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let repo_root = observe_repo_root(context);
    let source = resolve_create_source(arguments, context, &repo_root).await?;
    // Capture the task id (if any) before the source is moved, so a successful
    // create can record the provenance link-back on the kanban task.
    let task_id = match &source {
        CreateSource::Task { id, .. } => Some(id.clone()),
        _ => None,
    };

    let outcome = expect_op::run_create_request(
        source,
        repo_root,
        doctor::production_facts(),
        PoolConfig::remote(CREATE_POOL_WORKERS),
        factory.clone(),
    )
    .await
    .map_err(|err| rmcp::ErrorData::internal_error(err, None))?;

    if let Some(task_id) = task_id {
        record_task_provenance(context, &task_id, &outcome.path).await;
    }

    json_result(&outcome)
}

/// Resolve the `create` arguments to a [`CreateSource`].
///
/// The sources are checked in precedence order; `--from-spec` reads the design doc
/// from disk (safe-joined under the repo root), `--from-task` reads the task's
/// acceptance criteria from the kanban board. A `create` with no source argument is
/// an `invalid_params` error.
async fn resolve_create_source(
    arguments: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
    repo_root: &Path,
) -> Result<CreateSource, rmcp::ErrorData> {
    if let Some(id) = string_arg(arguments, "from_task") {
        let criteria = read_task_criteria(context, &id).await?;
        return Ok(CreateSource::Task { id, criteria });
    }
    if let Some(path) = string_arg(arguments, "from_spec") {
        let content = read_repo_file(repo_root, &path)?;
        return Ok(CreateSource::Spec { path, content });
    }
    if let Some(text) = string_arg(arguments, "from_session") {
        return Ok(CreateSource::Session(text));
    }
    if let Some(text) = string_arg(arguments, "from_chat") {
        return Ok(CreateSource::Chat(text));
    }
    if let Some(text) = string_arg(arguments, "intent") {
        return Ok(CreateSource::Intent(text));
    }
    Err(rmcp::ErrorData::invalid_params(
        "`create expectation` needs an intent: pass `intent`, or one of `from_task`, `from_spec`, \
         `from_session`, `from_chat`"
            .to_string(),
        None,
    ))
}

/// Read a `--from-spec` design doc, safe-joined under `repo_root`.
///
/// The path comes from the caller, so an absolute or `..`-bearing path would read
/// outside the repository; it is accepted only when relative and free of
/// parent-directory components (the same safe-join the engine applies on the write
/// side).
fn read_repo_file(repo_root: &Path, relative: &str) -> Result<String, rmcp::ErrorData> {
    let candidate = Path::new(relative);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(rmcp::ErrorData::invalid_params(
            format!("`from_spec` path `{relative}` must be relative without `..` components"),
            None,
        ));
    }
    std::fs::read_to_string(repo_root.join(candidate)).map_err(|err| {
        rmcp::ErrorData::invalid_params(
            format!("reading `from_spec` `{relative}` failed: {err}"),
            None,
        )
    })
}

/// The kanban board context rooted at the session working directory's `.kanban`
/// (mirroring the kanban tool's own resolution).
fn kanban_context(context: &ToolContext) -> KanbanContext {
    let working_dir = context
        .working_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    KanbanContext::new(working_dir.join(".kanban"))
}

/// Read a kanban task's acceptance criteria (its description) for `--from-task`.
///
/// # Errors
///
/// Returns `invalid_params` when the task cannot be read.
async fn read_task_criteria(context: &ToolContext, id: &str) -> Result<String, rmcp::ErrorData> {
    let ctx = kanban_context(context);
    let task = GetTask::new(id)
        .execute(&ctx)
        .await
        .into_result()
        .map_err(|err| {
            rmcp::ErrorData::invalid_params(
                format!("reading kanban task `{id}` failed: {err}"),
                None,
            )
        })?;
    let criteria = task["description"]
        .as_str()
        .or_else(|| task["title"].as_str())
        .unwrap_or_default()
        .to_string();
    Ok(criteria)
}

/// Record the spec's lineage back on the kanban task as a comment — provenance
/// only, never coupling the spec to the task lifecycle.
///
/// Best-effort: a failed comment is logged and does not fail the create (the spec
/// and its candidate observation are already written).
async fn record_task_provenance(context: &ToolContext, task_id: &str, identity: &str) {
    let ctx = kanban_context(context);
    let text = format!(
        "expect: drafted expectation `{identity}` from this task's acceptance criteria \
         (provenance only — the expectation stands on its own)"
    );
    if let Err(err) = AddComment::new(task_id, text)
        .execute(&ctx)
        .await
        .into_result()
    {
        tracing::warn!("expect create could not record task provenance on `{task_id}`: {err}");
    }
}

/// Register an `expect` tool configured with a live agent factory, replacing the
/// bare tool already registered under the `expect` name.
///
/// The wiring layer (which may depend on `swissarmyhammer-agent`) builds the
/// production [`AgentFactory`](expect_op::AgentFactory) from the session's
/// `ModelConfig` and calls this so the `create expectation` op can drive a live
/// authoring agent. Registration is by tool name, so it overwrites the bare tool
/// the default [`register_expect_tools`] installed.
pub fn register_expect_tool_with_factory(
    registry: &mut ToolRegistry,
    agent_factory: expect_op::AgentFactory,
) {
    registry.register(ExpectTool::new().with_agent_factory(agent_factory));
}

impl swissarmyhammer_common::health::Doctorable for ExpectTool {
    fn name(&self) -> &str {
        <Self as McpTool>::name(self)
    }

    fn category(&self) -> &str {
        doctor::EXPECT_CATEGORY
    }

    /// Surface every expectation spec's static diagnostics in `sah doctor`.
    ///
    /// Delegates to [`doctor::health_checks`] (the same static [`diagnose`] the
    /// scoped `expect doctor` trait verb runs): it discovers every `*.expect.md`
    /// under the session repo root, validates each against the live model
    /// registry, and maps each per-field finding to one [`HealthCheck`] under the
    /// `expect` category — no system driven, no model consulted. A pinned
    /// `model:` that has gone missing is a warning, not an error.
    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        doctor::health_checks()
    }
}

// The ACP delegation seam — the `AgentHandle`/`AgentFactory`, the pipeline gate,
// the spawn-blocking driver, and the use-case agent resolution — lives in
// `expect_op`, mirroring `review_op`. The op handlers that drive the agent (the
// observe-over-agent / check ops) consume it in later tasks.
pub mod expect_op;

// The real `Initializable` impl (the `expect init` scaffold) lives in `init`.
mod init;

// The static health check (`expect doctor` + the `sah doctor` provider) lives in
// `doctor`; the `Doctorable` impl below delegates to it.
pub mod doctor;

#[async_trait]
impl McpTool for ExpectTool {
    fn name(&self) -> &'static str {
        EXPECT_TOOL_NAME
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        generate_mcp_schema_wire(&EXPECT_OPERATIONS, expect_schema_config())
    }

    fn schema_full(&self) -> serde_json::Value {
        generate_mcp_schema_full(&EXPECT_OPERATIONS, expect_schema_config())
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some(EXPECT_TOOL_NAME)
    }

    fn operations(&self) -> &'static [&'static dyn Operation] {
        let ops: &[&'static dyn Operation] = &EXPECT_OPERATIONS;
        ops
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let op_str = arguments
            .get("op")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let Some(op_str) = op_str else {
            return Err(rmcp::ErrorData::invalid_params(
                "`expect` requires an `op` (a `<verb> <noun>` operation id, e.g. `check expectation`)",
                None,
            ));
        };

        // Dispatch routes the implemented ops to their handlers; every other
        // known op id resolves to its stub result, and anything unknown is an
        // invalid op. The `EXPECT_OPERATIONS` table stays the single source of
        // truth for "known", rather than a parallel match a human must keep in
        // lockstep with the struct list.
        match op_str {
            SURFACE_GET_OP => surface_get(&arguments),
            SURFACES_LIST_OP => surfaces_list(),
            EXPECTATION_GET_OP => expectation_get(&arguments, context),
            OBSERVATION_GET_OP => observation_get(&arguments, context),
            GOLDEN_GET_OP => golden_get(&arguments, context),
            OBSERVATIONS_LIST_OP => observations_list(&arguments, context),
            GOLDENS_LIST_OP => goldens_list(&arguments, context),
            EXPECTATIONS_LIST_OP => expectations_list(&arguments, context),
            EXPECTATION_OBSERVE_OP | EXPECTATIONS_OBSERVE_OP => observe_op(&arguments, context),
            OBSERVATION_APPROVE_OP | OBSERVATIONS_APPROVE_OP => approve_op(&arguments, context),
            OBSERVATION_EVALUATE_OP | OBSERVATIONS_EVALUATE_OP => {
                evaluate_op(&arguments, context, EvaluateSource::Received)
            }
            GOLDEN_EVALUATE_OP | GOLDENS_EVALUATE_OP => {
                evaluate_op(&arguments, context, EvaluateSource::Golden)
            }
            EXPECTATION_CHECK_OP | EXPECTATIONS_CHECK_OP => check_op(&arguments, context),
            EXPECTATION_CREATE_OP => {
                let factory = self.agent_factory.as_ref().ok_or_else(|| {
                    rmcp::ErrorData::internal_error(
                        "the `create expectation` op needs a live agent; this tool was built \
                         without an agent factory",
                        None,
                    )
                })?;
                expectation_create(factory, &arguments, context).await
            }
            known if EXPECT_OPERATIONS.iter().any(|op| op.op_string() == known) => {
                json_result(&not_implemented(known))
            }
            unknown => {
                let valid = EXPECT_OPERATIONS
                    .iter()
                    .map(|op| op.op_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(rmcp::ErrorData::invalid_params(
                    format!("Unknown operation '{unknown}'. Valid operations: {valid}"),
                    None,
                ))
            }
        }
    }
}

/// Register the operation-based `expect` tool with the registry.
pub fn register_expect_tools(registry: &mut ToolRegistry) {
    registry.register(ExpectTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::mcp::tool_handlers::ToolHandlers;

    /// The full domain grid from `ideas/expect.md` §Operations, each cell as its
    /// `<verb> <noun>` op id. This is the canonical source the tests check both
    /// directions against: the op list must cover exactly this grid, and the
    /// generated noun-first CLI tree must too.
    const GRID: &[&str] = &[
        "create expectation",
        "get expectation",
        "delete expectation",
        "observe expectation",
        "check expectation",
        "list expectations",
        "observe expectations",
        "check expectations",
        "get observation",
        "delete observation",
        "evaluate observation",
        "approve observation",
        "list observations",
        "evaluate observations",
        "approve observations",
        "get golden",
        "delete golden",
        "evaluate golden",
        "list goldens",
        "evaluate goldens",
        "get surface",
        "list surfaces",
    ];

    fn tool() -> ExpectTool {
        ExpectTool::new()
    }

    /// A minimal context (the stub ops read nothing from it).
    fn context() -> ToolContext {
        let git_ops = Arc::new(tokio::sync::Mutex::new(None));
        let tool_handlers = Arc::new(ToolHandlers::new());
        let agent_config = Arc::new(swissarmyhammer_config::ModelConfig::default());
        ToolContext::new(tool_handlers, git_ops, agent_config)
    }

    fn args(pairs: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        pairs.as_object().unwrap().clone()
    }

    /// The grid ops that now have a real implementation rather than the stub.
    /// The not-implemented dispatch test skips these.
    const IMPLEMENTED_OPS: &[&str] = &[
        EXPECTATION_CREATE_OP,
        SURFACE_GET_OP,
        SURFACES_LIST_OP,
        EXPECTATION_GET_OP,
        OBSERVATION_GET_OP,
        GOLDEN_GET_OP,
        OBSERVATIONS_LIST_OP,
        GOLDENS_LIST_OP,
        EXPECTATIONS_LIST_OP,
        EXPECTATION_OBSERVE_OP,
        EXPECTATIONS_OBSERVE_OP,
        OBSERVATION_APPROVE_OP,
        OBSERVATIONS_APPROVE_OP,
        OBSERVATION_EVALUATE_OP,
        OBSERVATIONS_EVALUATE_OP,
        GOLDEN_EVALUATE_OP,
        GOLDENS_EVALUATE_OP,
        EXPECTATION_CHECK_OP,
        EXPECTATIONS_CHECK_OP,
    ];

    /// Pull the JSON payload out of a successful tool result.
    fn payload_of(result: &CallToolResult) -> serde_json::Value {
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            other => panic!("expected text content, got {other:?}"),
        };
        serde_json::from_str(text).expect("json payload")
    }

    /// `EXPECT_OPERATIONS` must cover exactly the domain grid — no missing cells,
    /// no extras — so the op list cannot silently drift from the spec.
    #[test]
    fn operations_cover_the_domain_grid() {
        use std::collections::HashSet;
        let ops: HashSet<String> = EXPECT_OPERATIONS.iter().map(|op| op.op_string()).collect();
        let grid: HashSet<String> = GRID.iter().map(|s| s.to_string()).collect();
        assert_eq!(ops, grid, "EXPECT_OPERATIONS and the domain grid diverge");
    }

    /// Every grid cell must surface as a `noun → verb` pair in the command tree
    /// the shared generator builds from the FULL schema, and a noun-first argv
    /// must parse back to the matching `<verb> <noun>` op id.
    #[test]
    fn command_tree_is_noun_first_and_covers_the_grid() {
        use std::collections::HashSet;
        use swissarmyhammer_operations::cli_gen::build_commands_from_schema;
        use swissarmyhammer_operations::cli_gen::test_support::{
            collect_verb_noun_pairs, parse_argv,
        };

        let schema = tool().schema_full();
        let commands = build_commands_from_schema(&schema);
        let generated = collect_verb_noun_pairs(&commands);

        let expected: HashSet<String> = GRID.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            generated, expected,
            "generated noun-first command tree and the domain grid diverge"
        );

        // Noun-first parse: `expect expectation check` resolves to `check
        // expectation` (the internal `<verb> <noun>` op id).
        const SAMPLE_OP: &str = "check expectation";
        let parsed = parse_argv(
            EXPECT_TOOL_NAME,
            &schema,
            &["expect", "expectation", "check"],
        );
        assert_eq!(
            parsed.get("op").and_then(|v| v.as_str()),
            Some(SAMPLE_OP),
            "noun-first `expectation check` must parse to op `{SAMPLE_OP}`"
        );
    }

    /// The slim wire schema carries the `op` enum with every grid cell.
    #[test]
    fn wire_schema_exposes_every_op() {
        let wire = tool().schema();
        let ops = wire["properties"]["op"]["enum"]
            .as_array()
            .expect("op enum");
        for expected in GRID {
            assert!(
                ops.iter().any(|v| v == expected),
                "missing op `{expected}` from wire schema"
            );
        }
    }

    /// `register_expect_tools` registers a tool named `expect` exposing the grid.
    #[test]
    fn register_advertises_the_expect_tool() {
        let mut registry = ToolRegistry::new();
        register_expect_tools(&mut registry);
        let registered = registry
            .get_tool(EXPECT_TOOL_NAME)
            .expect("expect tool registered");
        let ops: Vec<String> = registered
            .operations()
            .iter()
            .map(|o| o.op_string())
            .collect();
        for expected in GRID {
            assert!(
                ops.iter().any(|s| s == expected),
                "registered tool missing op `{expected}`"
            );
        }
    }

    /// Each grid op dispatches to the structured not-implemented placeholder
    /// (no panic, success result carrying the stable status + op id).
    #[tokio::test]
    async fn every_grid_op_dispatches_to_not_implemented() {
        for op in GRID {
            if IMPLEMENTED_OPS.contains(op) {
                continue;
            }
            let result = tool()
                .execute(args(serde_json::json!({ "op": op })), &context())
                .await
                .unwrap_or_else(|e| panic!("op `{op}` should dispatch, got error: {e}"));
            assert!(
                !result.is_error.unwrap_or(false),
                "op `{op}` should be a success result"
            );
            let text = match &result.content[0].raw {
                rmcp::model::RawContent::Text(t) => &t.text,
                other => panic!("op `{op}` should return text content, got {other:?}"),
            };
            let payload: serde_json::Value = serde_json::from_str(text).expect("json payload");
            assert_eq!(
                payload["status"], NOT_IMPLEMENTED_STATUS,
                "op `{op}` payload should carry the not-implemented status"
            );
            assert_eq!(
                payload["op"], *op,
                "op `{op}` payload should echo the op id"
            );
        }
    }

    /// An unknown op id is rejected with `invalid_params`, not dispatched.
    #[tokio::test]
    async fn unknown_op_is_rejected() {
        let err = tool()
            .execute(
                args(serde_json::json!({ "op": "frobnicate widget" })),
                &context(),
            )
            .await
            .expect_err("unknown op must error");
        assert!(err.message.contains("Unknown operation"));
    }

    /// A missing `op` is rejected with `invalid_params`.
    #[tokio::test]
    async fn missing_op_is_rejected() {
        let err = tool()
            .execute(serde_json::Map::new(), &context())
            .await
            .expect_err("missing op must error");
        assert!(err.message.contains("op"));
    }

    /// `surfaces list` returns the full surface adapter catalog, byte-for-byte
    /// the engine's source-of-truth `catalog()`.
    #[tokio::test]
    async fn surfaces_list_returns_the_full_catalog() {
        let result = tool()
            .execute(
                args(serde_json::json!({ "op": SURFACES_LIST_OP })),
                &context(),
            )
            .await
            .expect("surfaces list should dispatch");
        assert!(!result.is_error.unwrap_or(false));
        let returned: Vec<swissarmyhammer_expect::SurfaceInfo> =
            serde_json::from_value(payload_of(&result)).expect("catalog array");
        assert_eq!(returned, surfaces::catalog());
    }

    /// `surface get cli` returns exactly the catalog's cli entry.
    #[tokio::test]
    async fn surface_get_returns_the_named_entry() {
        let result = tool()
            .execute(
                args(serde_json::json!({ "op": SURFACE_GET_OP, "name": "cli" })),
                &context(),
            )
            .await
            .expect("surface get should dispatch");
        assert!(!result.is_error.unwrap_or(false));
        let returned: swissarmyhammer_expect::SurfaceInfo =
            serde_json::from_value(payload_of(&result)).expect("surface info");
        assert_eq!(Some(returned), surfaces::get(Surface::Cli));
    }

    /// An unknown surface name is rejected with a clear error naming the input,
    /// not dispatched as a catalog hit.
    #[tokio::test]
    async fn surface_get_unknown_name_errors() {
        let err = tool()
            .execute(
                args(serde_json::json!({ "op": SURFACE_GET_OP, "name": "telepathy" })),
                &context(),
            )
            .await
            .expect_err("unknown surface must error");
        assert!(err.message.contains("telepathy"));
        assert!(err.message.contains("unknown surface"));
    }

    /// `surface get` without a `name` is rejected.
    #[tokio::test]
    async fn surface_get_missing_name_errors() {
        let err = tool()
            .execute(
                args(serde_json::json!({ "op": SURFACE_GET_OP })),
                &context(),
            )
            .await
            .expect_err("missing name must error");
        assert!(err.message.contains("name"));
    }

    /// A fixture cli spec: an echoing SUT driven through two `When` steps. The
    /// observe op must capture one checkpoint per step plus a final and persist
    /// the received observation under `.expect/received/`.
    #[cfg(unix)]
    const FIXTURE_SPEC: &str = "---\n\
         description: the app echoes each command it is given\n\
         surface: cli\n\
         setup: ./app.sh\n\
         ---\n\
         \n\
         The app echoes the argument it is driven with.\n\
         \n\
         ## When\n\
         - first\n\
         - second\n\
         \n\
         ## Then\n\
         - [ ] it echoes the first command\n\
         - [ ] it echoes the second command\n";

    /// The repo-relative identity of [`FIXTURE_SPEC`].
    #[cfg(unix)]
    const FIXTURE_IDENTITY: &str = "echo";

    /// Stand up a temp repo with the echoing cli SUT and the fixture spec, and
    /// return a [`ToolContext`] rooted there alongside the repo dir.
    #[cfg(unix)]
    fn observe_fixture() -> (tempfile::TempDir, ToolContext) {
        use std::os::unix::fs::PermissionsExt;

        let repo = tempfile::TempDir::new().unwrap();
        let app = repo.path().join("app.sh");
        std::fs::write(&app, "#!/bin/sh\necho \"$@\"\n").unwrap();
        let mut perms = std::fs::metadata(&app).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&app, perms).unwrap();
        std::fs::write(
            repo.path().join(format!("{FIXTURE_IDENTITY}.expect.md")),
            FIXTURE_SPEC,
        )
        .unwrap();

        let ctx = context().with_working_dir(repo.path().to_path_buf());
        (repo, ctx)
    }

    /// `observe expectation <scope>` provisions the cli SUT, drives each `When`
    /// step, and writes the received observation (3 checkpoints) to disk.
    #[cfg(unix)]
    #[tokio::test]
    async fn observe_expectation_writes_received_observation() {
        let (repo, ctx) = observe_fixture();

        let result = tool()
            .execute(
                args(serde_json::json!({
                    "op": EXPECTATION_OBSERVE_OP,
                    "scope": FIXTURE_IDENTITY,
                })),
                &ctx,
            )
            .await
            .expect("observe expectation should dispatch");
        assert!(!result.is_error.unwrap_or(false), "observe should succeed");

        let payload = payload_of(&result);
        assert_eq!(payload["count"], 1, "exactly one spec observed");
        assert_eq!(payload["observed"][0]["path"], FIXTURE_IDENTITY);
        // Two When steps plus a final.
        assert_eq!(payload["observed"][0]["checkpoints"], 3);

        // The received observation is written to the gitignored slot and reloads.
        let received = repo
            .path()
            .join(".expect")
            .join("received")
            .join(format!("{FIXTURE_IDENTITY}.received.json"));
        assert!(received.is_file(), "received observation is persisted");
        let observation: swissarmyhammer_expect::Observation =
            serde_json::from_str(&std::fs::read_to_string(&received).unwrap())
                .expect("received json parses");
        assert_eq!(observation.path, FIXTURE_IDENTITY);
        assert_eq!(observation.checkpoints.len(), 3);
    }

    /// `observe expectations` (plural) runs the same over a multi-spec scope.
    #[cfg(unix)]
    #[tokio::test]
    async fn observe_expectations_runs_over_a_batch() {
        let (repo, ctx) = observe_fixture();

        let result = tool()
            .execute(
                args(serde_json::json!({ "op": EXPECTATIONS_OBSERVE_OP })),
                &ctx,
            )
            .await
            .expect("observe expectations should dispatch");
        assert!(!result.is_error.unwrap_or(false), "observe should succeed");

        let payload = payload_of(&result);
        assert_eq!(payload["count"], 1, "the batch covers the one fixture spec");
        let received = repo
            .path()
            .join(".expect")
            .join("received")
            .join(format!("{FIXTURE_IDENTITY}.received.json"));
        assert!(
            received.is_file(),
            "the batch persists each received observation"
        );
    }

    /// Write a spec at `identity` carrying the given Tier-1 `criteria`.
    fn write_spec(repo: &Path, identity: &str, criteria: &[&str]) {
        let mut body = String::from(
            "---\ndescription: a coupon reduces the total\nsurface: cli\n---\n\n## Then\n",
        );
        for criterion in criteria {
            body.push_str(&format!("- [ ] {criterion}\n"));
        }
        std::fs::write(repo.join(format!("{identity}.expect.md")), body).unwrap();
    }

    /// Write a single-checkpoint JSON observation for `identity` to `path`,
    /// creating parent directories — a hand-written received/golden fixture.
    fn write_observation(path: &Path, identity: &str, body: serde_json::Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let observation = serde_json::json!({
            "path": identity,
            "checkpoints": [{
                "after": "final",
                "state": { "kind": "json", "body": body },
                "duration_ms": 1
            }],
            "trajectory": { "steps": [] }
        });
        std::fs::write(path, serde_json::to_string_pretty(&observation).unwrap()).unwrap();
    }

    /// Dispatch an evaluate `op` over `scope` against the repo rooted at `ctx`,
    /// returning the success payload.
    async fn run_evaluate(op: &str, scope: &str, ctx: &ToolContext) -> serde_json::Value {
        let result = tool()
            .execute(args(serde_json::json!({ "op": op, "scope": scope })), ctx)
            .await
            .unwrap_or_else(|e| panic!("`{op}` should dispatch, got error: {e}"));
        assert!(!result.is_error.unwrap_or(false), "`{op}` should succeed");
        payload_of(&result)
    }

    /// `observation evaluate <scope>` re-judges the stored received observation,
    /// returning the per-criterion verdict — without re-running the system.
    #[tokio::test]
    async fn observation_evaluate_re_judges_a_stored_received_file() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(
            repo.path(),
            "coupon",
            &[
                "the total is $40",
                "the item count equals the number of items",
            ],
        );
        write_observation(
            &repo.path().join(".expect/received/coupon.received.json"),
            "coupon",
            serde_json::json!({ "total": 40, "item_count": 3, "items": [{}, {}, {}] }),
        );
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(OBSERVATION_EVALUATE_OP, "coupon", &ctx).await;

        assert_eq!(payload["count"], 1);
        assert_eq!(payload["source"], "received");
        let verdict = &payload["evaluated"][0]["verdict"];
        assert_eq!(verdict["path"], "coupon");
        let criteria = verdict["criteria"].as_array().expect("criteria array");
        assert_eq!(criteria.len(), 2, "both Tier-1 criteria are graded");
        assert!(
            criteria.iter().all(|c| c["pass"] == true),
            "every criterion holds against the received observation"
        );
    }

    /// `golden evaluate <scope>` re-grades the approved golden against the current
    /// (edited) criterion set — one criterion that still holds, one that no longer
    /// does — without re-running the system.
    #[tokio::test]
    async fn golden_evaluate_re_grades_a_stored_golden_against_an_edited_criterion_set() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(
            repo.path(),
            "coupon",
            &["the total is $40", "the discount is $5"],
        );
        write_observation(
            &repo.path().join(".expect/goldens/coupon.golden.json"),
            "coupon",
            serde_json::json!({ "total": 40 }),
        );
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(GOLDEN_EVALUATE_OP, "coupon", &ctx).await;

        assert_eq!(payload["source"], "golden");
        let criteria = payload["evaluated"][0]["verdict"]["criteria"]
            .as_array()
            .expect("criteria array");
        assert_eq!(criteria.len(), 2, "both edited criteria are graded");
        let passes: Vec<bool> = criteria
            .iter()
            .map(|c| c["pass"].as_bool().expect("pass bool"))
            .collect();
        assert!(passes.contains(&true), "the holding criterion passes");
        assert!(
            passes.contains(&false),
            "the edited criterion that no longer holds is surfaced as a fail"
        );
    }

    /// `golden evaluate` over a spec with no golden yet reports the missing source
    /// gracefully (a clear status, not a hard error) — the golden store lands in a
    /// later task, and the op is already wired to its path.
    #[tokio::test]
    async fn golden_evaluate_reports_a_missing_golden_gracefully() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(GOLDEN_EVALUATE_OP, "coupon", &ctx).await;

        assert_eq!(payload["count"], 1);
        let entry = &payload["evaluated"][0];
        assert_eq!(entry["path"], "coupon");
        assert_eq!(entry["status"], MISSING_SOURCE_STATUS);
    }

    /// Dispatch a no-scope `op` over the repo rooted at `ctx`, returning the
    /// success payload — the list ops survey every spec, so they take no scope.
    async fn run_op(op: &str, ctx: &ToolContext) -> serde_json::Value {
        let result = tool()
            .execute(args(serde_json::json!({ "op": op })), ctx)
            .await
            .unwrap_or_else(|e| panic!("`{op}` should dispatch, got error: {e}"));
        assert!(!result.is_error.unwrap_or(false), "`{op}` should succeed");
        payload_of(&result)
    }

    /// Write an approved golden for `identity` (a single-checkpoint json
    /// observation with a real compiled assertion) under `.expect/goldens/`.
    fn write_golden_fixture(repo: &Path, identity: &str) {
        use swissarmyhammer_expect::{
            compile, spec_hash, write_golden, Criterion, ExpectConfig, Golden, GradingPins,
        };

        let observation: Observation = serde_json::from_value(serde_json::json!({
            "path": identity,
            "checkpoints": [{
                "after": "final",
                "state": { "kind": "json", "body": { "total": 40 } },
                "duration_ms": 1
            }],
            "trajectory": { "steps": [] }
        }))
        .unwrap();
        let assertion = compile(
            &Criterion {
                text: "the total is $40".to_string(),
                checked: false,
            },
            &observation,
        )
        .expect("criterion compiles");
        // Hash the spec on disk so the fixture golden's stale-detection hash
        // matches its `*.expect.md`, exactly as a real `approve` would freeze it.
        let spec = specs_in(repo, Some(identity))
            .into_iter()
            .next()
            .expect("spec on disk");
        let golden = Golden {
            observation,
            assertions: vec![assertion],
            grading: GradingPins::from_config(&ExpectConfig::default()),
            spec_hash: spec_hash(&spec),
        };
        write_golden(repo, &golden).expect("write golden");
    }

    /// `expectation get <scope>` returns the parsed spec: frontmatter, intent, and
    /// the `## Then` criteria.
    #[tokio::test]
    async fn expectation_get_returns_the_parsed_spec() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(
            repo.path(),
            "coupon",
            &["the total is $40", "the discount is $5"],
        );
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(EXPECTATION_GET_OP, "coupon", &ctx).await;

        assert_eq!(payload["count"], 1);
        let spec = &payload["expectations"][0];
        assert_eq!(spec["path"], "coupon");
        assert_eq!(spec["frontmatter"]["surface"], "cli");
        let criteria = spec["criteria"].as_array().expect("criteria array");
        assert_eq!(criteria.len(), 2);
        assert_eq!(criteria[0]["text"], "the total is $40");
        assert!(
            spec["intent"].as_str().is_some(),
            "the parsed spec carries its intent body",
        );
    }

    /// `observation get <scope>` returns the stored received observation's
    /// checkpoint timeline and driver trajectory.
    #[tokio::test]
    async fn observation_get_returns_the_checkpoint_timeline_and_trajectory() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        write_observation(
            &repo.path().join(".expect/received/coupon.received.json"),
            "coupon",
            serde_json::json!({ "total": 40 }),
        );
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(OBSERVATION_GET_OP, "coupon", &ctx).await;

        assert_eq!(payload["count"], 1);
        let observation = &payload["observations"][0]["observation"];
        assert_eq!(observation["path"], "coupon");
        assert_eq!(
            observation["checkpoints"]
                .as_array()
                .expect("checkpoints")
                .len(),
            1,
        );
        assert!(
            observation["trajectory"]["steps"].is_array(),
            "the trajectory travels with the observation",
        );
    }

    /// `observation get <scope>` over a never-observed spec reports the missing
    /// received slot gracefully rather than erroring.
    #[tokio::test]
    async fn observation_get_reports_a_missing_observation_gracefully() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(OBSERVATION_GET_OP, "coupon", &ctx).await;

        assert_eq!(payload["observations"][0]["status"], MISSING_SOURCE_STATUS,);
    }

    /// `golden get <scope>` returns the stored golden: its scrubbed observation,
    /// frozen assertions, and grading pins.
    #[tokio::test]
    async fn golden_get_returns_the_stored_golden() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        write_golden_fixture(repo.path(), "coupon");
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(GOLDEN_GET_OP, "coupon", &ctx).await;

        assert_eq!(payload["count"], 1);
        let golden = &payload["goldens"][0]["golden"];
        assert_eq!(golden["observation"]["path"], "coupon");
        assert_eq!(
            golden["assertions"].as_array().expect("assertions").len(),
            1,
        );
        assert!(
            golden["grading"]["embedder"].as_str().is_some(),
            "the golden pins its grading embedder",
        );
    }

    /// `golden get <scope>` over a spec with no approved golden reports the missing
    /// baseline gracefully.
    #[tokio::test]
    async fn golden_get_reports_a_missing_golden_gracefully() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(GOLDEN_GET_OP, "coupon", &ctx).await;

        assert_eq!(payload["goldens"][0]["status"], MISSING_SOURCE_STATUS);
    }

    /// `goldens list` surveys exactly the spec identities that carry an approved
    /// golden, not every spec.
    #[tokio::test]
    async fn goldens_list_surveys_specs_with_an_approved_golden() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        write_spec(repo.path(), "refund", &["the total is $40"]);
        write_golden_fixture(repo.path(), "coupon");
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_op(GOLDENS_LIST_OP, &ctx).await;

        assert_eq!(payload["count"], 1, "only the approved spec is listed");
        assert_eq!(payload["goldens"][0], "coupon");
    }

    /// `observations list` surveys exactly the spec identities that carry a stored
    /// received observation.
    #[tokio::test]
    async fn observations_list_surveys_specs_with_a_received_observation() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        write_spec(repo.path(), "refund", &["the total is $40"]);
        write_observation(
            &repo.path().join(".expect/received/coupon.received.json"),
            "coupon",
            serde_json::json!({ "total": 40 }),
        );
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_op(OBSERVATIONS_LIST_OP, &ctx).await;

        assert_eq!(payload["count"], 1, "only the observed spec is listed");
        assert_eq!(payload["observations"][0], "coupon");
    }

    /// `goldens list <scope>` narrows the survey to the scoped specs — both specs
    /// have goldens, but a specific-spec scope restricts the listing to one.
    #[tokio::test]
    async fn goldens_list_honors_a_scope() {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        write_spec(repo.path(), "refund", &["the total is $40"]);
        write_golden_fixture(repo.path(), "coupon");
        write_golden_fixture(repo.path(), "refund");
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let payload = run_evaluate(GOLDENS_LIST_OP, "coupon", &ctx).await;

        assert_eq!(payload["count"], 1, "the scope narrows the survey");
        assert_eq!(payload["goldens"][0], "coupon");
    }

    /// `expectations list` surveys every spec with its drift-ledger state, ordering
    /// the unapproved-drift queue FIRST and annotating the drifted row with its
    /// re-derived old-vs-new evidence — the survey doubling as the review queue.
    #[tokio::test]
    async fn expectations_list_returns_ledger_state_and_orders_drifted_first() {
        let repo = tempfile::TempDir::new().unwrap();

        // `new`: a spec with no golden yet.
        write_spec(repo.path(), "fresh", &["the total is $40"]);

        // `approved`: a golden plus a matching received run.
        write_spec(repo.path(), "approved", &["the total is $40"]);
        seed_golden(repo.path(), "approved", serde_json::json!({ "total": 40 }));
        write_observation(
            &repo.path().join(".expect/received/approved.received.json"),
            "approved",
            serde_json::json!({ "total": 40 }),
        );

        // `drifted`: a golden plus a received run whose matched value changed.
        write_spec(repo.path(), "drifted", &["the total is $40"]);
        seed_golden(repo.path(), "drifted", serde_json::json!({ "total": 40 }));
        write_observation(
            &repo.path().join(".expect/received/drifted.received.json"),
            "drifted",
            serde_json::json!({ "total": 50 }),
        );

        // `stale`: a golden was approved, then the `*.expect.md` was edited.
        write_spec(repo.path(), "stale", &["the total is $40"]);
        seed_golden(repo.path(), "stale", serde_json::json!({ "total": 40 }));
        write_spec(
            repo.path(),
            "stale",
            &["the total is $40", "the discount is $5"],
        );

        let ctx = context().with_working_dir(repo.path().to_path_buf());
        let payload = run_op(EXPECTATIONS_LIST_OP, &ctx).await;

        assert_eq!(payload["count"], 4, "every spec is surveyed");
        let entries = payload["expectations"]
            .as_array()
            .expect("expectations array");

        // The unapproved drift leads the queue, carrying its old-vs-new evidence.
        assert_eq!(entries[0]["path"], "drifted");
        assert_eq!(entries[0]["state"], "drifted");
        assert_eq!(
            entries[0]["comparison"]["criteria"][0]["drifted"], true,
            "the drifted row carries re-derived old-vs-new evidence",
        );

        // Every other spec carries its classified ledger state.
        let state_of = |path: &str| {
            entries
                .iter()
                .find(|entry| entry["path"] == path)
                .and_then(|entry| entry["state"].as_str())
                .map(str::to_string)
        };
        assert_eq!(state_of("fresh").as_deref(), Some("new"));
        assert_eq!(state_of("approved").as_deref(), Some("approved"));
        assert_eq!(state_of("stale").as_deref(), Some("stale"));

        // Only the drifted row carries the old-vs-new comparison.
        let approved_entry = entries
            .iter()
            .find(|entry| entry["path"] == "approved")
            .expect("approved entry");
        assert!(
            approved_entry.get("comparison").is_none(),
            "a non-drifted row carries no comparison",
        );

        // The op honors a scope, like the sibling list ops, narrowing the survey.
        let scoped = run_evaluate(EXPECTATIONS_LIST_OP, "drifted", &ctx).await;
        assert_eq!(scoped["count"], 1, "a scope narrows the survey");
        assert_eq!(scoped["expectations"][0]["path"], "drifted");
    }

    /// `list expectations` must declare the `scope`/`tag` inputs its handler reads,
    /// so the generated CLI and MCP schema actually accept them — mirroring the
    /// sibling `list observations` / `list goldens` ops.
    #[test]
    fn expectations_list_declares_the_scope_inputs() {
        let op = EXPECT_OPERATIONS
            .iter()
            .find(|op| op.op_string() == EXPECTATIONS_LIST_OP)
            .expect("list expectations op registered");
        let params: Vec<&str> = op.parameters().iter().map(|param| param.name).collect();
        assert!(
            params.contains(&"scope") && params.contains(&"tag"),
            "list expectations must advertise scope/tag, got {params:?}",
        );
    }

    // -----------------------------------------------------------------------
    // approve observation / observations.
    //
    // The CI gate is exercised by injecting the flag straight into the private
    // handlers (`approve_write`) rather than through the ambient `CI` env var, so
    // these tests are deterministic even when the suite itself runs under CI. One
    // small `#[serial]` test covers the `ci_enabled()` env read in isolation.
    // -----------------------------------------------------------------------

    /// The default grading pins an approve pass freezes when there is no config.
    fn default_grading() -> GradingPins {
        GradingPins::from_config(&ExpectConfig::default())
    }

    /// Resolve the specs in `repo` for `scope` (every spec when `None`).
    fn specs_in(repo: &Path, scope: Option<&str>) -> Vec<Expectation> {
        ExpectationLoader::new(repo)
            .resolve_scope(scope, None)
            .expect("resolve scope")
    }

    /// Stand up a repo with a brand-new spec (`coupon`) that has a received run
    /// but no golden yet.
    fn new_spec_repo() -> tempfile::TempDir {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "coupon", &["the total is $40"]);
        write_observation(
            &repo.path().join(".expect/received/coupon.received.json"),
            "coupon",
            serde_json::json!({ "total": 40 }),
        );
        repo
    }

    /// `approve observation --all` writes the scrubbed golden with its frozen
    /// assertions, and the returned diff shows the criterion→binding (the locator,
    /// not just the value), so a mis-compiled locator is caught at review.
    #[test]
    fn approve_writes_golden_and_the_diff_shows_the_binding() {
        let repo = new_spec_repo();
        let specs = specs_in(repo.path(), Some("coupon"));

        let result = approve_write(
            &specs,
            repo.path(),
            ApproveMode::All,
            &default_grading(),
            &ScrubberSet::default_set(),
            false, // not CI
        )
        .expect("approve writes");
        let payload = payload_of(&result);

        assert_eq!(payload["count"], 1);
        let written = &payload["written"][0];
        assert_eq!(written["path"], "coupon");
        assert_eq!(written["status"], "new");
        let binding = &written["diff"][0];
        assert_eq!(binding["criterion"], "the total is $40");
        assert_eq!(binding["locator"], "$.total");
        assert_eq!(binding["value"], "40");
        // The binding string carries the locator, not just the value.
        assert!(binding["binding"]
            .as_str()
            .expect("binding string")
            .contains("$.total"));

        // The golden is persisted, scrubbed, with one frozen assertion.
        let golden = read_golden(repo.path(), "coupon")
            .expect("read golden")
            .expect("golden present");
        assert_eq!(golden.assertions.len(), 1);
    }

    /// A bare `approve` (no mode flag) previews the would-be diff and writes
    /// nothing — the explicit-confirmation gate — exercised through the full
    /// dispatch (`execute`), which is env-independent for a preview.
    #[tokio::test]
    async fn approve_without_a_mode_previews_and_writes_nothing() {
        let repo = new_spec_repo();
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let result = tool()
            .execute(
                args(serde_json::json!({ "op": OBSERVATION_APPROVE_OP, "scope": "coupon" })),
                &ctx,
            )
            .await
            .expect("approve preview should dispatch");
        assert!(!result.is_error.unwrap_or(false));
        let payload = payload_of(&result);

        assert_eq!(payload["requires_confirmation"], true);
        assert_eq!(payload["count"], 1);
        let entry = &payload["preview"][0];
        assert_eq!(entry["status"], "new");
        assert_eq!(entry["diff"][0]["locator"], "$.total");
        // No golden was written — the preview is purely advisory.
        assert!(
            read_golden(repo.path(), "coupon")
                .expect("read golden")
                .is_none(),
            "a preview writes no golden",
        );
    }

    /// Seed an approved golden for `identity` by freezing its criteria against an
    /// observation carrying `body` — the real approve path, not a hand-built
    /// golden, so the frozen assertions are genuine.
    fn seed_golden(repo: &Path, identity: &str, body: serde_json::Value) {
        let spec = specs_in(repo, Some(identity))
            .into_iter()
            .next()
            .expect("spec to seed");
        let observation: Observation = serde_json::from_value(serde_json::json!({
            "path": identity,
            "checkpoints": [{
                "after": "final",
                "state": { "kind": "json", "body": body },
                "duration_ms": 1
            }],
            "trajectory": { "steps": [] }
        }))
        .unwrap();
        let golden = approve(
            &spec,
            &observation,
            default_grading(),
            None,
            &ScrubberSet::default_set(),
        )
        .expect("seed approve");
        write_golden(repo, &golden).expect("write seed golden");
    }

    /// Stand up a repo with one new spec (`fresh`) and one drifted spec
    /// (`drifted`).
    ///
    /// The drifted spec carries an **invariant** criterion: its golden was frozen
    /// against three items, and the received run grew to five — the relationship
    /// still holds (so it is genuinely approvable), but the evidence moved, so the
    /// compare flags it as drift. A *literal* drift (a value that simply changed)
    /// would instead violate its own criterion and be refused at approve, which is
    /// correct but not what the selection test is exercising.
    fn mixed_status_repo() -> tempfile::TempDir {
        let repo = tempfile::TempDir::new().unwrap();
        write_spec(repo.path(), "fresh", &["the total is $40"]);
        write_observation(
            &repo.path().join(".expect/received/fresh.received.json"),
            "fresh",
            serde_json::json!({ "total": 40 }),
        );
        write_spec(
            repo.path(),
            "drifted",
            &["the item count equals the number of items"],
        );
        seed_golden(
            repo.path(),
            "drifted",
            serde_json::json!({ "item_count": 3, "items": [{}, {}, {}] }),
        );
        write_observation(
            &repo.path().join(".expect/received/drifted.received.json"),
            "drifted",
            serde_json::json!({ "item_count": 5, "items": [{}, {}, {}, {}, {}] }),
        );
        repo
    }

    /// The written/skipped path partition of an approve-write payload.
    fn partition(payload: &serde_json::Value) -> (Vec<String>, Vec<String>) {
        let paths = |key: &str| {
            payload[key]
                .as_array()
                .expect("array")
                .iter()
                .map(|entry| entry["path"].as_str().expect("path").to_string())
                .collect::<Vec<_>>()
        };
        (paths("written"), paths("skipped"))
    }

    /// `--missing` selects only the brand-new spec; the drifted spec is skipped.
    #[test]
    fn approve_missing_selects_only_new_expectations() {
        let repo = mixed_status_repo();
        let specs = specs_in(repo.path(), None);

        let result = approve_write(
            &specs,
            repo.path(),
            ApproveMode::Missing,
            &default_grading(),
            &ScrubberSet::default_set(),
            false,
        )
        .expect("approve writes");
        let (written, skipped) = partition(&payload_of(&result));

        assert_eq!(written, vec!["fresh"], "only the new spec is approved");
        assert!(
            skipped.contains(&"drifted".to_string()),
            "the drifted spec is skipped by --missing",
        );
    }

    /// `--changed` selects only the drifted spec; the brand-new spec is skipped.
    #[test]
    fn approve_changed_selects_only_drifted_expectations() {
        let repo = mixed_status_repo();
        let specs = specs_in(repo.path(), None);

        let result = approve_write(
            &specs,
            repo.path(),
            ApproveMode::Changed,
            &default_grading(),
            &ScrubberSet::default_set(),
            false,
        )
        .expect("approve writes");
        let (written, skipped) = partition(&payload_of(&result));

        assert_eq!(
            written,
            vec!["drifted"],
            "only the drifted spec is approved"
        );
        assert!(
            skipped.contains(&"fresh".to_string()),
            "the new spec is skipped by --changed",
        );
    }

    /// Under CI, approve refuses to write an unapproved drift — a hard failure,
    /// never a silent re-approval.
    #[test]
    fn ci_refuses_to_approve_a_drift() {
        let repo = mixed_status_repo();
        let specs = specs_in(repo.path(), Some("drifted"));

        let err = approve_write(
            &specs,
            repo.path(),
            ApproveMode::Changed,
            &default_grading(),
            &ScrubberSet::default_set(),
            true, // CI
        )
        .expect_err("CI must refuse to write a drift");

        assert!(
            err.message.contains("refuses"),
            "the refusal names the CI gate: {}",
            err.message
        );
    }

    /// Strict first-run: under CI a brand-new expectation cannot be baselined —
    /// approve refuses rather than minting a green baseline.
    #[test]
    fn ci_refuses_to_baseline_a_new_expectation() {
        let repo = new_spec_repo();
        let specs = specs_in(repo.path(), Some("coupon"));

        let err = approve_write(
            &specs,
            repo.path(),
            ApproveMode::All,
            &default_grading(),
            &ScrubberSet::default_set(),
            true, // CI
        )
        .expect_err("CI must refuse to baseline a new expectation");

        assert!(err.message.contains("refuses"), "{}", err.message);
        // Strict first-run: no golden was minted in CI.
        assert!(
            read_golden(repo.path(), "coupon")
                .expect("read golden")
                .is_none(),
            "a new baseline is never minted in CI",
        );
    }

    /// Passing more than one mode flag is rejected up front.
    #[tokio::test]
    async fn approve_rejects_more_than_one_mode_flag() {
        let repo = new_spec_repo();
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let err = tool()
            .execute(
                args(serde_json::json!({
                    "op": OBSERVATION_APPROVE_OP,
                    "scope": "coupon",
                    "missing": true,
                    "all": true,
                })),
                &ctx,
            )
            .await
            .expect_err("two mode flags must be rejected");
        assert!(err.message.contains("at most one"));
    }

    /// `ci_enabled()` reads `CI=true` from the environment (the one env-touching
    /// test, serialized and self-restoring).
    #[test]
    #[serial_test::serial(env)]
    fn ci_enabled_reads_the_environment() {
        let restore = std::env::var(CI_ENV_KEY).ok();

        std::env::set_var(CI_ENV_KEY, CI_ENABLED_VALUE);
        assert!(ci_enabled(), "CI=true enables the gate");
        std::env::set_var(CI_ENV_KEY, "false");
        assert!(!ci_enabled(), "CI=false leaves the gate open");
        std::env::remove_var(CI_ENV_KEY);
        assert!(!ci_enabled(), "an unset CI leaves the gate open");

        match restore {
            Some(value) => std::env::set_var(CI_ENV_KEY, value),
            None => std::env::remove_var(CI_ENV_KEY),
        }
    }

    // -----------------------------------------------------------------------
    // check expectation / expectations.
    // -----------------------------------------------------------------------

    /// `check expectations` runs the doctor gate, then observe → evaluate →
    /// compare: a malformed spec is refused before any observe (status
    /// `malformed`, never observed), a well-formed cli spec with no golden is
    /// `new`, and the rolled-up exit code is the worst per-spec code (the malformed
    /// spec's, distinct from a code-failure exit).
    #[cfg(unix)]
    #[tokio::test]
    async fn check_expectations_gates_malformed_and_runs_wellformed() {
        use std::os::unix::fs::PermissionsExt;

        let repo = tempfile::TempDir::new().unwrap();
        let app = repo.path().join("app.sh");
        std::fs::write(&app, "#!/bin/sh\necho \"$@\"\n").unwrap();
        let mut perms = std::fs::metadata(&app).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&app, perms).unwrap();

        // A well-formed cli spec: drives one When step, asserts a deterministic
        // exit code — passes evaluate, no golden yet, so it is `new`.
        std::fs::write(
            repo.path().join("echo.expect.md"),
            "---\ndescription: the app echoes and exits cleanly\nsurface: cli\nsetup: ./app.sh\n---\n\nThe app echoes its argument and exits zero.\n\n## When\n- hello\n\n## Then\n- [ ] the command exits with code 0\n",
        )
        .unwrap();
        // A malformed spec: an unknown frontmatter key the doctor gate rejects.
        std::fs::write(
            repo.path().join("broken.expect.md"),
            "---\ndescription: a malformed spec\nsurfce: cli\n---\n\nIntent.\n\n## Then\n- [ ] the command exits with code 0\n",
        )
        .unwrap();

        let ctx = context().with_working_dir(repo.path().to_path_buf());
        let result = tool()
            .execute(
                args(serde_json::json!({ "op": EXPECTATIONS_CHECK_OP })),
                &ctx,
            )
            .await
            .expect("check expectations should dispatch");
        assert!(!result.is_error.unwrap_or(false), "check should succeed");
        let payload = payload_of(&result);

        let entries = payload["entries"].as_array().expect("entries array");
        let status_of = |path: &str| {
            entries
                .iter()
                .find(|entry| entry["path"] == path)
                .and_then(|entry| entry["status"].as_str())
                .map(str::to_string)
        };
        assert_eq!(status_of("broken").as_deref(), Some("malformed"));
        assert_eq!(status_of("echo").as_deref(), Some("new"));

        // The malformed spec dominates the aggregate exit code, distinct from a
        // code-failure exit.
        assert_eq!(
            payload["exit_code"],
            swissarmyhammer_expect::CHECK_EXIT_MALFORMED
        );

        // The doctor gate observed the well-formed spec (its received run was
        // persisted) but never observed the malformed one.
        assert!(
            repo.path()
                .join(".expect/received/echo.received.json")
                .is_file(),
            "the well-formed spec was observed and its received run persisted",
        );
        assert!(
            !repo
                .path()
                .join(".expect/received/broken.received.json")
                .exists(),
            "the malformed spec was never observed",
        );
    }

    // -----------------------------------------------------------------------
    // create expectation.
    //
    // Exercised through the full dispatch with a scripted authoring agent
    // injected as the agent factory, so the op drives the agent seam end to end
    // and leaves a doctor-green spec + candidate observation in `new` state.
    // -----------------------------------------------------------------------

    /// A doctor-green spec the scripted authoring agent returns as its draft.
    const CREATE_GREEN_SPEC: &str = "---\n\
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

    /// An [`AgentFactory`](expect_op::AgentFactory) backed by a scripted agent that
    /// returns the `{path, content}` draft JSON for any prompt — the authoring
    /// agent the green-loop drives, deterministically.
    fn create_scripted_factory(spec_path: &str, content: &str) -> expect_op::AgentFactory {
        use agent_client_protocol::DynConnectTo;
        use swissarmyhammer_validators::review::test_support::{
            ScriptedAdapter, ScriptedAgent, ScriptedAgentConfig,
        };
        use tokio::sync::broadcast;

        let draft = serde_json::json!({ "path": spec_path, "content": content }).to_string();
        let agent = ScriptedAgent::with_config(
            vec![],
            ScriptedAgentConfig {
                default_response: draft,
                ..ScriptedAgentConfig::default()
            },
        );
        Arc::new(move || {
            let agent = Arc::clone(&agent);
            Box::pin(async move {
                let (notify_tx, notification_rx) = broadcast::channel(64);
                let agent = ScriptedAgent::rebind_broadcast(&agent, notify_tx, true);
                Ok(expect_op::AgentHandle {
                    agent: DynConnectTo::new(ScriptedAdapter(agent)),
                    notification_rx,
                })
            })
        })
    }

    /// `create expectation "<intent>"` drives the scripted authoring agent through
    /// the doctor green-loop, writes the spec plus a candidate observation, and
    /// leaves the result unapproved (`new`).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn create_expectation_authors_a_green_spec_left_new() {
        const SPEC_PATH: &str = "src/checkout/coupon.expect.md";
        let repo = tempfile::TempDir::new().unwrap();
        let factory = create_scripted_factory(SPEC_PATH, CREATE_GREEN_SPEC);
        let tool = ExpectTool::new().with_agent_factory(factory);
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let result = tool
            .execute(
                args(serde_json::json!({
                    "op": EXPECTATION_CREATE_OP,
                    "intent": "a coupon reduces the order total once",
                })),
                &ctx,
            )
            .await
            .expect("create expectation should dispatch");
        assert!(!result.is_error.unwrap_or(false), "create should succeed");

        let payload = payload_of(&result);
        assert_eq!(payload["state"], "new", "the candidate is left unapproved");
        assert_eq!(payload["path"], "src/checkout/coupon");

        // The spec and its candidate observation were written under the repo.
        assert!(
            repo.path().join(SPEC_PATH).is_file(),
            "the drafted spec is written"
        );
        assert!(
            repo.path()
                .join(".expect/received/src/checkout/coupon.received.json")
                .is_file(),
            "the candidate observation is written",
        );
    }

    /// `create expectation` on a tool built without an agent factory is a clear
    /// error — authoring requires a live agent.
    #[tokio::test]
    async fn create_expectation_requires_an_agent_factory() {
        let err = tool()
            .execute(
                args(serde_json::json!({ "op": EXPECTATION_CREATE_OP, "intent": "x" })),
                &context(),
            )
            .await
            .expect_err("create without a factory must error");
        assert!(
            err.message.contains("agent"),
            "the error names the missing agent: {}",
            err.message
        );
    }

    /// `create expectation --from-task <id>` reads the task's acceptance criteria
    /// from a real kanban board, drafts a green spec, and records the provenance
    /// link-back as a comment on the task — without coupling to its lifecycle.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn create_expectation_from_task_drafts_from_criteria_and_records_provenance() {
        use swissarmyhammer_kanban::{board::InitBoard, comment::ListComments, task::AddTask};

        const SPEC_PATH: &str = "src/checkout/coupon.expect.md";
        const CRITERIA: &str = "the coupon should only apply once and reduce the total";
        let repo = tempfile::TempDir::new().unwrap();

        // Stand up a real kanban board with one task carrying the criteria.
        let kctx = KanbanContext::new(repo.path().join(".kanban"));
        InitBoard::new("test")
            .execute(&kctx)
            .await
            .into_result()
            .expect("init board");
        let added = AddTask::new("coupon")
            .with_description(CRITERIA)
            .execute(&kctx)
            .await
            .into_result()
            .expect("add task");
        let task_id = added["id"].as_str().expect("created task id").to_string();

        let factory = create_scripted_factory(SPEC_PATH, CREATE_GREEN_SPEC);
        let tool = ExpectTool::new().with_agent_factory(factory);
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let result = tool
            .execute(
                args(serde_json::json!({ "op": EXPECTATION_CREATE_OP, "from_task": task_id })),
                &ctx,
            )
            .await
            .expect("create from_task should dispatch");
        assert!(!result.is_error.unwrap_or(false), "create should succeed");

        let payload = payload_of(&result);
        assert_eq!(payload["state"], "new");
        assert_eq!(payload["provenance"]["source"], "task");
        assert_eq!(payload["provenance"]["reference"], task_id);
        assert!(repo.path().join(SPEC_PATH).is_file(), "the spec is written");

        // The provenance link-back landed as a kanban comment naming the spec.
        // Read through a FRESH context: the create op wrote via its own
        // KanbanContext instance, and the test's `kctx` memoizes an entity context
        // that would not reflect that external write.
        let read_ctx = KanbanContext::new(repo.path().join(".kanban"));
        let comments = ListComments::new(task_id.as_str())
            .execute(&read_ctx)
            .await
            .into_result()
            .expect("list comments");
        assert!(
            comments.to_string().contains("src/checkout/coupon"),
            "a provenance comment names the drafted spec: {comments}",
        );
    }

    /// `create expectation --from-spec <path>` reads the design doc from disk and
    /// routes it through the same pipeline, recording the doc path as provenance.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn create_expectation_from_spec_reads_the_doc_and_routes_through() {
        const SPEC_PATH: &str = "src/checkout/coupon.expect.md";
        let repo = tempfile::TempDir::new().unwrap();
        std::fs::write(
            repo.path().join("design.md"),
            "The coupon must only apply once and reduce the total.",
        )
        .unwrap();

        let factory = create_scripted_factory(SPEC_PATH, CREATE_GREEN_SPEC);
        let tool = ExpectTool::new().with_agent_factory(factory);
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let result = tool
            .execute(
                args(serde_json::json!({ "op": EXPECTATION_CREATE_OP, "from_spec": "design.md" })),
                &ctx,
            )
            .await
            .expect("create from_spec should dispatch");
        assert!(!result.is_error.unwrap_or(false), "create should succeed");

        let payload = payload_of(&result);
        assert_eq!(payload["state"], "new");
        assert_eq!(payload["provenance"]["source"], "spec");
        assert_eq!(payload["provenance"]["reference"], "design.md");
        assert!(repo.path().join(SPEC_PATH).is_file(), "the spec is written");
    }

    /// `create expectation --from-spec` refuses a `..`-escaping doc path.
    #[tokio::test]
    async fn create_expectation_from_spec_rejects_path_traversal() {
        let repo = tempfile::TempDir::new().unwrap();
        let factory = create_scripted_factory("src/x.expect.md", CREATE_GREEN_SPEC);
        let tool = ExpectTool::new().with_agent_factory(factory);
        let ctx = context().with_working_dir(repo.path().to_path_buf());

        let err = tool
            .execute(
                args(serde_json::json!({
                    "op": EXPECTATION_CREATE_OP,
                    "from_spec": "../escape.md",
                })),
                &ctx,
            )
            .await
            .expect_err("a `..`-escaping from_spec path must be refused");
        assert!(
            err.message.contains(".."),
            "the error names the boundary: {}",
            err.message
        );
    }
}
