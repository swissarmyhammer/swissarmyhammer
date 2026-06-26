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
    observe, surfaces, write_received, CliAdapter, Expectation, ExpectationLoader, Observation,
    ObserveConfig, Surface,
};

use crate::mcp::op_tool_helpers::{json_result, string_arg};
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

/// `create expectation` — draft a new expectation spec.
#[operation(
    verb = "create",
    noun = "expectation",
    description = "Draft a new expectation spec"
)]
#[derive(Debug, Default)]
pub struct ExpectationCreate;

/// `get expectation` — read one expectation spec.
#[operation(
    verb = "get",
    noun = "expectation",
    description = "Get one expectation spec"
)]
#[derive(Debug, Default)]
pub struct ExpectationGet;

/// `delete expectation` — remove a spec and its observation and golden.
#[operation(
    verb = "delete",
    noun = "expectation",
    description = "Delete an expectation spec and its observation and golden"
)]
#[derive(Debug, Default)]
pub struct ExpectationDelete;

/// The `scope` / `tag` parameters shared by `observe expectation` and
/// `observe expectations`: both resolve a `<scope>` (optionally narrowed by a
/// `--tag`) through [`ExpectationLoader::resolve_scope`], so they declare an
/// identical parameter set from one source rather than two drifting copies.
static OBSERVE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("scope")
        .description(
            "The expectation scope: a spec path, a folder, or a glob. Omit to observe every spec.",
        )
        .param_type(ParamType::String),
    ParamMeta::new("tag")
        .description("Narrow the scope to specs carrying this tag.")
        .param_type(ParamType::String),
];

/// `observe expectation` — drive the system and capture an observation.
///
/// A manual [`Operation`] impl (rather than the `#[operation]` macro) so it can
/// declare the [`OBSERVE_PARAMS`] scope/tag inputs, mirroring [`SurfaceGet`].
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
        OBSERVE_PARAMS
    }
}

/// `check expectation` — doctor, observe, evaluate, and compare one expectation.
#[operation(
    verb = "check",
    noun = "expectation",
    description = "Doctor, observe, evaluate, and compare one expectation"
)]
#[derive(Debug, Default)]
pub struct ExpectationCheck;

/// `list expectations` — survey every expectation with its ledger state.
#[operation(
    verb = "list",
    noun = "expectations",
    description = "List every expectation with its ledger state"
)]
#[derive(Debug, Default)]
pub struct ExpectationsList;

/// `observe expectations` — capture observations for a batch of expectations.
///
/// Shares [`OBSERVE_PARAMS`] with [`ExpectationObserve`]; the two differ only in
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
        OBSERVE_PARAMS
    }
}

/// `check expectations` — doctor, observe, evaluate, and compare a batch.
#[operation(
    verb = "check",
    noun = "expectations",
    description = "Doctor, observe, evaluate, and compare a batch of expectations"
)]
#[derive(Debug, Default)]
pub struct ExpectationsCheck;

/// `get observation` — read one stored observation.
#[operation(
    verb = "get",
    noun = "observation",
    description = "Get one stored observation (checkpoint timeline + trajectory)"
)]
#[derive(Debug, Default)]
pub struct ObservationGet;

/// `delete observation` — remove a stored observation.
#[operation(
    verb = "delete",
    noun = "observation",
    description = "Delete a stored observation"
)]
#[derive(Debug, Default)]
pub struct ObservationDelete;

/// `evaluate observation` — re-judge a stored observation (no re-run).
#[operation(
    verb = "evaluate",
    noun = "observation",
    description = "Re-judge a stored observation against its criteria without re-running the system"
)]
#[derive(Debug, Default)]
pub struct ObservationEvaluate;

/// `approve observation` — promote a stored observation to its golden baseline.
#[operation(
    verb = "approve",
    noun = "observation",
    description = "Promote a stored observation to its golden baseline"
)]
#[derive(Debug, Default)]
pub struct ObservationApprove;

/// `list observations` — survey stored observations.
#[operation(
    verb = "list",
    noun = "observations",
    description = "List stored observations"
)]
#[derive(Debug, Default)]
pub struct ObservationsList;

/// `evaluate observations` — re-judge a batch of stored observations (no re-run).
#[operation(
    verb = "evaluate",
    noun = "observations",
    description = "Re-judge a batch of stored observations without re-running the system"
)]
#[derive(Debug, Default)]
pub struct ObservationsEvaluate;

/// `approve observations` — promote a batch of observations to their goldens.
#[operation(
    verb = "approve",
    noun = "observations",
    description = "Promote a batch of observations to their golden baselines"
)]
#[derive(Debug, Default)]
pub struct ObservationsApprove;

/// `get golden` — read one approved golden baseline.
#[operation(
    verb = "get",
    noun = "golden",
    description = "Get one approved golden baseline"
)]
#[derive(Debug, Default)]
pub struct GoldenGet;

/// `delete golden` — remove a golden baseline.
#[operation(
    verb = "delete",
    noun = "golden",
    description = "Delete a golden baseline"
)]
#[derive(Debug, Default)]
pub struct GoldenDelete;

/// `evaluate golden` — re-grade a golden baseline (no re-run).
#[operation(
    verb = "evaluate",
    noun = "golden",
    description = "Re-grade a golden baseline against edited criteria without re-running the system"
)]
#[derive(Debug, Default)]
pub struct GoldenEvaluate;

/// `list goldens` — survey approved golden baselines.
#[operation(
    verb = "list",
    noun = "goldens",
    description = "List approved golden baselines"
)]
#[derive(Debug, Default)]
pub struct GoldensList;

/// `evaluate goldens` — re-grade a batch of golden baselines (no re-run).
#[operation(
    verb = "evaluate",
    noun = "goldens",
    description = "Re-grade a batch of golden baselines without re-running the system"
)]
#[derive(Debug, Default)]
pub struct GoldensEvaluate;

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
#[derive(Debug, Default)]
pub struct ExpectTool;

impl ExpectTool {
    /// Build the tool.
    pub fn new() -> Self {
        Self
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
    let scope = string_arg(arguments, "scope");
    let tag = string_arg(arguments, "tag");
    let repo_root = observe_repo_root(context);

    let loader = ExpectationLoader::new(&repo_root);
    let specs = loader
        .resolve_scope(scope.as_deref(), tag.as_deref())
        .map_err(|err| {
            rmcp::ErrorData::internal_error(format!("scope resolution failed: {err}"), None)
        })?;

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

crate::impl_default_doctorable!(ExpectTool);

// The real `Initializable` impl (the `expect init` scaffold) lives in `init`.
mod init;

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
            EXPECTATION_OBSERVE_OP | EXPECTATIONS_OBSERVE_OP => observe_op(&arguments, context),
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
        SURFACE_GET_OP,
        SURFACES_LIST_OP,
        EXPECTATION_OBSERVE_OP,
        EXPECTATIONS_OBSERVE_OP,
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
}
