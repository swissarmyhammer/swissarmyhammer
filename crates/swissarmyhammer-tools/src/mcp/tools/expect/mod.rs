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
//! Every op is currently a stub: it dispatches to a structured "not implemented
//! yet" payload. The real implementations (and their parameters, scope
//! resolution, doctor pass, observe/evaluate/compare machinery) land in later
//! tasks, which replace these stubs and the placeholder
//! [`Doctorable`](swissarmyhammer_common::health::Doctorable) /
//! [`Initializable`](crate::mcp::tool_registry::Initializable) impls.

use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, operation, Operation, SchemaConfig,
};

use crate::mcp::op_tool_helpers::json_result;
use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};

/// The tool's registered name and its `cli_category` (the top-level `sah`
/// subcommand the noun-first command tree hangs under).
const EXPECT_TOOL_NAME: &str = "expect";

/// The `status` field every stub op returns until its real implementation lands.
/// Tests assert against this constant rather than re-typing the literal.
const NOT_IMPLEMENTED_STATUS: &str = "not_implemented";

// ---------------------------------------------------------------------------
// Operations (one zero-sized struct per `<verb> <noun>` grid cell). Parameters
// are added when each op gains its real implementation; for the skeleton every
// op is parameterless and dispatches to the not-implemented placeholder.
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

/// `observe expectation` — drive the system and capture an observation.
#[operation(
    verb = "observe",
    noun = "expectation",
    description = "Drive the system and capture an observation for one expectation"
)]
#[derive(Debug, Default)]
pub struct ExpectationObserve;

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
#[operation(
    verb = "observe",
    noun = "expectations",
    description = "Capture observations for a batch of expectations"
)]
#[derive(Debug, Default)]
pub struct ExpectationsObserve;

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
#[operation(
    verb = "get",
    noun = "surface",
    description = "Get one surface adapter from the catalog"
)]
#[derive(Debug, Default)]
pub struct SurfaceGet;

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

crate::impl_default_doctorable!(ExpectTool);
crate::impl_empty_initializable!(ExpectTool);

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
        _context: &ToolContext,
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

        // Dispatch is data-driven over EXPECT_OPERATIONS: a known op id resolves
        // to its (currently stub) result; anything else is an invalid op. This
        // keeps the op table the single source of truth rather than a parallel
        // 22-arm match a human must keep in lockstep with the struct list.
        if EXPECT_OPERATIONS.iter().any(|op| op.op_string() == op_str) {
            json_result(&not_implemented(op_str))
        } else {
            let valid = EXPECT_OPERATIONS
                .iter()
                .map(|op| op.op_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(rmcp::ErrorData::invalid_params(
                format!("Unknown operation '{op_str}'. Valid operations: {valid}"),
                None,
            ))
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
}
