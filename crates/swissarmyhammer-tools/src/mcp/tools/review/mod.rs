//! Operation-based `review` MCP tool.
//!
//! A single op-dispatched tool — like `git`, `kanban`, and `code_context` — that
//! exposes the local multi-agent review engine plus validator introspection. The
//! tool is a thin dispatch shim: it maps `op` → action, resolves the engine's
//! inputs from the MCP session/work-dir, calls the engine, and serializes the
//! result. No pipeline logic lives here (it lives in
//! [`swissarmyhammer_validators::review`]).
//!
//! ## Ops
//!
//! | Op | Purpose |
//! |----|---------|
//! | `review file` | Review a file path or glob (the scope's noun). |
//! | `review working` | Review uncommitted changes vs HEAD. |
//! | `review sha` | Review the changes in/since a commit or range. |
//! | `list validators` | Summarize the loaded RuleSet stack. |
//! | `get validator` | One validator's frontmatter, probes, and rule bodies. |
//! | `check validators` | Lint every loaded validator. |
//!
//! The three `review` ops drive the engine over a live ACP agent, supplied to the
//! tool through an [`AgentFactory`](review_op::AgentFactory) seam (the production
//! server injects the configured backend; tests inject a scripted agent). The
//! loader-read ops need no agent.

/// The three pipeline ops (`review file/working/sha`): scope resolution, engine
/// invocation over a live ACP agent, and progress/content notification streaming.
pub mod review_op;
/// The validator introspection ops (`list/get/check validators`): pure loader
/// reads over the builtin → user → project RuleSet stack, no agent required.
pub mod validators;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use swissarmyhammer_common::utils::find_git_repository_root_from;
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, Operation, ParamMeta, ParamType,
    SchemaConfig,
};
use swissarmyhammer_validators::review::Scope;

use crate::mcp::op_tool_helpers::{json_result, string_arg, string_array_arg, usize_arg};
use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};
use review_op::{AgentFactory, EmbedderFactory, ReviewRequest, ReviewResponse};

// ---------------------------------------------------------------------------
// Operations (verb + noun + parameter metadata) — schema + CLI generation.
// ---------------------------------------------------------------------------

/// The shared `validators?[]` modifier, declared once and spliced into each
/// `review` op's parameter list (ParamMeta is not `Copy`, so it is re-stated
/// rather than indexed out of a shared static).
const VALIDATORS_PARAM: ParamMeta = ParamMeta::new("validators")
    .description(
        "Optional subset of validator names to run (defaults to every matching validator).",
    )
    .param_type(ParamType::Array);

/// The shared `backend?` modifier.
const BACKEND_PARAM: ParamMeta = ParamMeta::new("backend")
    .description("Agent backend / concurrency policy: `session` (remote default) or `local` (one in-process worker).")
    .param_type(ParamType::String);

/// The shared `batch_size?` modifier (bytes), declared once and spliced into each
/// `review` op's parameter list.
const BATCH_SIZE_PARAM: ParamMeta = ParamMeta::new("batch_size")
    .description(
        "Max inlined file content per review batch, in BYTES (default 262144 = 256 KiB). Changed files are packed whole into batches up to this budget and each batch is reviewed independently; a single file larger than this is an error. Raise it to review larger files in one batch, lower it for smaller batches.",
    )
    .param_type(ParamType::Integer);

/// `review file` — review an explicit file path or glob.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReviewFile;

static REVIEW_FILE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("path")
        .description("A file path or glob to review (reviewed whole when there is no diff).")
        .param_type(ParamType::String)
        .required(),
    VALIDATORS_PARAM,
    BACKEND_PARAM,
    BATCH_SIZE_PARAM,
];

impl Operation for ReviewFile {
    fn verb(&self) -> &'static str {
        "review"
    }
    fn noun(&self) -> &'static str {
        "file"
    }
    fn description(&self) -> &'static str {
        "Review an explicit file path or glob with the multi-agent review engine"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        REVIEW_FILE_PARAMS
    }
}

/// `review working` — review uncommitted changes vs HEAD.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReviewWorking;

impl Operation for ReviewWorking {
    fn verb(&self) -> &'static str {
        "review"
    }
    fn noun(&self) -> &'static str {
        "working"
    }
    fn description(&self) -> &'static str {
        "Review uncommitted changes vs HEAD with the multi-agent review engine"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        REVIEW_WORKING_PARAMS
    }
}

static REVIEW_WORKING_PARAMS: &[ParamMeta] = &[VALIDATORS_PARAM, BACKEND_PARAM, BATCH_SIZE_PARAM];

/// `review sha` — review the changes in/since a commit or range.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReviewSha;

static REVIEW_SHA_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("sha")
        .description("A commit sha or range (e.g. `HEAD~1..HEAD`) whose changes to review.")
        .param_type(ParamType::String)
        .required(),
    VALIDATORS_PARAM,
    BACKEND_PARAM,
    BATCH_SIZE_PARAM,
];

impl Operation for ReviewSha {
    fn verb(&self) -> &'static str {
        "review"
    }
    fn noun(&self) -> &'static str {
        "sha"
    }
    fn description(&self) -> &'static str {
        "Review the changes in/since a commit or range with the multi-agent review engine"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        REVIEW_SHA_PARAMS
    }
}

/// `list validators` — summarize the loaded RuleSet stack.
#[derive(Clone, Copy, Debug, Default)]
pub struct ListValidators;

static LIST_VALIDATORS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("source")
        .description("Filter by precedence layer: `builtin` | `user` | `project` | `all`.")
        .param_type(ParamType::String),
    ParamMeta::new("match")
        .description("Filter to validators whose globs match this path/glob.")
        .param_type(ParamType::String),
];

impl Operation for ListValidators {
    fn verb(&self) -> &'static str {
        "list"
    }
    fn noun(&self) -> &'static str {
        "validators"
    }
    fn description(&self) -> &'static str {
        "List the loaded validators (RuleSets), their source layer, globs, and probes"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        LIST_VALIDATORS_PARAMS
    }
}

/// `get validator` — one validator's full detail.
#[derive(Clone, Copy, Debug, Default)]
pub struct GetValidator;

static GET_VALIDATOR_PARAMS: &[ParamMeta] = &[ParamMeta::new("name")
    .description("The validator (RuleSet) name to read.")
    .param_type(ParamType::String)
    .required()];

impl Operation for GetValidator {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "validator"
    }
    fn description(&self) -> &'static str {
        "Read one validator's frontmatter, probes, and full rule bodies"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_VALIDATOR_PARAMS
    }
}

/// `check validators` — lint every loaded validator.
#[derive(Clone, Copy, Debug, Default)]
pub struct CheckValidators;

impl Operation for CheckValidators {
    fn verb(&self) -> &'static str {
        "check"
    }
    fn noun(&self) -> &'static str {
        "validators"
    }
    fn description(&self) -> &'static str {
        "Lint every loaded validator: globs compile, no stray trigger, probes exist"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        &[]
    }
}

static REVIEW_FILE: Lazy<ReviewFile> = Lazy::new(ReviewFile::default);
static REVIEW_WORKING: Lazy<ReviewWorking> = Lazy::new(ReviewWorking::default);
static REVIEW_SHA: Lazy<ReviewSha> = Lazy::new(ReviewSha::default);
static LIST_VALIDATORS: Lazy<ListValidators> = Lazy::new(ListValidators::default);
static GET_VALIDATOR: Lazy<GetValidator> = Lazy::new(GetValidator::default);
static CHECK_VALIDATORS: Lazy<CheckValidators> = Lazy::new(CheckValidators::default);

/// The full operation set the `review` tool advertises.
pub static REVIEW_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*REVIEW_FILE as &dyn Operation,
        &*REVIEW_WORKING as &dyn Operation,
        &*REVIEW_SHA as &dyn Operation,
        &*LIST_VALIDATORS as &dyn Operation,
        &*GET_VALIDATOR as &dyn Operation,
        &*CHECK_VALIDATORS as &dyn Operation,
    ]
});

/// The op dispatched when a call omits `op` (or sends it empty).
///
/// Named so the default in [`ReviewTool::execute`] and its match arm share one
/// definition and can never diverge; a test pins it to the `review working`
/// operation advertised by [`REVIEW_OPERATIONS`].
const DEFAULT_OP: &str = "review working";

// ---------------------------------------------------------------------------
// The tool.
// ---------------------------------------------------------------------------

/// The operation-based `review` MCP tool.
///
/// Holds an optional [`AgentFactory`]: the loader-read ops (`list`/`get`/`check`
/// validators) never use it, but the three `review` ops require it. The
/// production server injects a factory that builds the configured backend; a
/// tool constructed without one (the default) serves the loader-read ops and
/// returns an actionable error for the `review` ops.
#[derive(Default)]
pub struct ReviewTool {
    /// The live-agent factory the `review` ops drive, if wired.
    agent_factory: Option<AgentFactory>,
    /// The embedder factory the probe runner uses; defaults to the loaded
    /// platform embedder when unset.
    embedder_factory: Option<EmbedderFactory>,
    /// The pinned pool worker count from `review.concurrency`, applied by the
    /// server at the wiring layer. `None` defers to the coarse `backend` policy.
    concurrency: Option<usize>,
}

impl std::fmt::Debug for ReviewTool {
    /// Manual impl: the two factory fields are trait objects (closures) with no
    /// `Debug` of their own, so they render by presence/absence instead.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReviewTool")
            .field(
                "agent_factory",
                &self.agent_factory.as_ref().map(|_| "AgentFactory"),
            )
            .field(
                "embedder_factory",
                &self.embedder_factory.as_ref().map(|_| "EmbedderFactory"),
            )
            .field("concurrency", &self.concurrency)
            .finish()
    }
}

impl ReviewTool {
    /// A tool with no agent factory — loader-read ops only.
    pub fn new() -> Self {
        Self {
            agent_factory: None,
            embedder_factory: None,
            concurrency: None,
        }
    }

    /// Attach the live-agent factory the three `review` ops drive.
    pub fn with_agent_factory(mut self, factory: AgentFactory) -> Self {
        self.agent_factory = Some(factory);
        self
    }

    /// Override the embedder factory (defaults to the loaded platform embedder).
    pub fn with_embedder_factory(mut self, factory: EmbedderFactory) -> Self {
        self.embedder_factory = Some(factory);
        self
    }

    /// Pin the review pool worker count from the `review.concurrency` config.
    ///
    /// `Some(n)` pins every `review` op's pool to `n` workers regardless of the
    /// coarse `backend` choice; `None` (the default) defers to the backend
    /// policy. The server sets this when it wires the configured tool.
    pub fn with_concurrency(mut self, concurrency: Option<usize>) -> Self {
        self.concurrency = concurrency;
        self
    }

    /// Resolve the repository root from the MCP session work-dir (never
    /// `current_dir()`): the explicit `working_dir`, then its git root.
    fn resolve_repo_path(
        &self,
        context: &ToolContext,
    ) -> Result<std::path::PathBuf, rmcp::ErrorData> {
        let working_dir = context.working_dir.clone().ok_or_else(|| {
            rmcp::ErrorData::internal_error(
                "review tool requires a session working directory (working_dir is unset)",
                None,
            )
        })?;
        Ok(find_git_repository_root_from(&working_dir).unwrap_or(working_dir))
    }

    /// Dispatch one of the three `review` ops: build the scope, resolve inputs,
    /// drive the engine, and serialize the report.
    async fn execute_review(
        &self,
        scope: Scope,
        args: &serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let factory = self.agent_factory.as_ref().ok_or_else(|| {
            rmcp::ErrorData::internal_error(
                "the `review` ops need a live agent; this tool was built without an agent factory \
                 (the loader-read ops `list`/`get`/`check validators` work without one)",
                None,
            )
        })?;

        let repo_path = self.resolve_repo_path(context)?;
        let request = ReviewRequest::new(scope)
            .with_backend(string_arg(args, "backend"))
            .with_validators(string_array_arg(args, "validators"))
            .with_concurrency(self.concurrency)
            .with_batch_size(usize_arg(args, "batch_size"));

        let embedder_factory = self
            .embedder_factory
            .clone()
            .unwrap_or_else(review_op::default_embedder_factory);
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

        // Streaming bridge (one per call, whenever any transport can carry
        // something — MCP peer, in-process sink): spawned HERE on the outer
        // runtime, BEFORE `run_review_request` enters its spawn_blocking +
        // nested current-thread runtime — only the sync UnboundedSender
        // crosses in. A peer WITHOUT a progressToken still gets a bridge
        // (content-only: notifications/message + keep-alive; progress ticks
        // are token-gated per the MCP spec). Only no-transport-at-all → None
        // → zero notifications.
        let (progress, drain) = match review_op::spawn_review_progress_bridge(context) {
            Some(bridge) => {
                let (sender, drain) = bridge.into_parts();
                (Some(sender), Some(drain))
            }
            None => (None, None),
        };

        let result = review_op::run_review_request(
            request,
            &repo_path,
            embedder_factory,
            factory.clone(),
            &now,
            progress,
        )
        .await;

        // The pipeline dropped its sender when it finished, so the bridge
        // drains to completion; await it so every buffered notification is
        // flushed to the client before the final result returns. Progress is
        // advisory — a drain that did not join cleanly is logged, never fatal.
        if let Some(drain) = drain {
            if let Err(err) = drain.await {
                tracing::warn!(error = ?err, "review: progress drain task did not join cleanly");
            }
        }

        let report = result.map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        json_result(&ReviewResponse::from(report))
    }
}

/// The `sah doctor` category the review tool's validator checks report under.
const VALIDATORS_CATEGORY: &str = "validators";

impl swissarmyhammer_common::health::Doctorable for ReviewTool {
    fn name(&self) -> &str {
        <Self as McpTool>::name(self)
    }

    fn category(&self) -> &str {
        VALIDATORS_CATEGORY
    }

    /// Lint every loaded validator and surface the result in `sah doctor`.
    ///
    /// Reuses the engine's `check validators` lint (the same loader read + lint
    /// the `check validators` op runs — no agent, no review run, no re-linting
    /// here). All valid → one OK line; each problem → one Error line naming the
    /// offending validator, describing the problem, and carrying a fix.
    ///
    /// CWD resolution: like the `list`/`get`/`check validators` ops, the lint
    /// loads the project layer relative to the session's working directory (the
    /// directory `sah doctor` runs in), never an unrelated `current_dir()`.
    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        use swissarmyhammer_common::health::HealthCheck;

        match validators::check_validators() {
            Ok(response) if response.ok => {
                vec![HealthCheck::ok(
                    "Validators",
                    format!("{} validators loaded, all valid", response.count),
                    VALIDATORS_CATEGORY,
                )]
            }
            Ok(response) => response
                .errors
                .into_iter()
                .map(|problem| {
                    HealthCheck::error(
                        format!("Validator {}", problem.path),
                        problem.problem,
                        Some(
                            "Fix the validator's VALIDATOR.md frontmatter (see `review check \
                             validators` for the full lint)"
                                .to_string(),
                        ),
                        VALIDATORS_CATEGORY,
                    )
                })
                .collect(),
            Err(e) => vec![HealthCheck::error(
                "Validators",
                format!("failed to lint validators: {e}"),
                Some("Ensure the validator directories are readable".to_string()),
                VALIDATORS_CATEGORY,
            )],
        }
    }
}

crate::impl_empty_initializable!(ReviewTool);

/// Shared schema config for the review tool, so the wire and full generators
/// stay in lockstep on the description.
fn review_schema_config() -> SchemaConfig {
    SchemaConfig::new(
        "Local multi-agent code review and validator introspection, dispatched by `op`.",
    )
}

#[async_trait]
impl McpTool for ReviewTool {
    fn name(&self) -> &'static str {
        "review"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    /// The slim WIRE schema advertised over MCP: only the `op` enum, dropping the
    /// heavy CLI-facing keys (`x-operation-schemas` / `x-operation-groups` /
    /// `x-op-signatures` / …). The in-process CLI tree consumes [`Self::schema_full`].
    fn schema(&self) -> serde_json::Value {
        generate_mcp_schema_wire(&REVIEW_OPERATIONS, review_schema_config())
    }

    /// The FULL in-process schema: carries `x-operation-schemas`,
    /// `x-operation-groups`, and `x-op-signatures` for noun/verb CLI generation.
    fn schema_full(&self) -> serde_json::Value {
        generate_mcp_schema_full(&REVIEW_OPERATIONS, review_schema_config())
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("review")
    }

    fn operations(&self) -> &'static [&'static dyn Operation] {
        let ops: &[&'static dyn Operation] = &REVIEW_OPERATIONS;
        ops
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        // A missing or empty `op` falls back to [`DEFAULT_OP`], the same
        // constant its match arm dispatches on, so the two cannot diverge.
        let op_str = arguments
            .get("op")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_OP);

        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            DEFAULT_OP => self.execute_review(Scope::Working, &args, context).await,
            "review file" => {
                let target = string_arg(&args, "path").ok_or_else(|| {
                    rmcp::ErrorData::invalid_params(
                        "`review file` requires a `path` (a file path or glob)",
                        None,
                    )
                })?;
                self.execute_review(scope_for_path(&target), &args, context)
                    .await
            }
            "review sha" => {
                let sha = string_arg(&args, "sha").ok_or_else(|| {
                    rmcp::ErrorData::invalid_params(
                        "`review sha` requires a `sha` (a commit or range)",
                        None,
                    )
                })?;
                self.execute_review(Scope::Sha(sha), &args, context).await
            }
            "list validators" => {
                let summaries = validators::list_validators(
                    string_arg(&args, "source").as_deref(),
                    string_arg(&args, "match").as_deref(),
                )
                .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;
                json_result(&summaries)
            }
            "get validator" => {
                let name = string_arg(&args, "name").ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("`get validator` requires a `name`", None)
                })?;
                let detail = validators::get_validator(&name)
                    .map_err(|e| rmcp::ErrorData::invalid_params(e, None))?;
                json_result(&detail)
            }
            "check validators" => {
                let response = validators::check_validators()
                    .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;
                json_result(&response)
            }
            other => {
                // The valid-op list is derived from `REVIEW_OPERATIONS` (the single
                // source of truth) so a new op can never silently diverge from this
                // message.
                let valid_ops = REVIEW_OPERATIONS
                    .iter()
                    .map(|op| format!("'{}'", op.op_string()))
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(rmcp::ErrorData::invalid_params(
                    format!("Unknown operation '{other}'. Valid operations: {valid_ops}"),
                    None,
                ))
            }
        }
    }
}

/// Build the [`Scope`] for a `review file` target: a glob when it has glob
/// metacharacters, else a single file path.
fn scope_for_path(target: &str) -> Scope {
    if target.contains(['*', '?', '[']) {
        Scope::Glob(target.to_string())
    } else {
        Scope::File(target.to_string())
    }
}

/// Register the operation-based `review` tool with the registry.
///
/// The tool is registered without an agent factory: the loader-read ops
/// (`list`/`get`/`check validators`) work immediately. The server attaches a
/// live-agent factory for the three `review` ops where it wires the backend.
pub fn register_review_tools(registry: &mut ToolRegistry) {
    registry.register(ReviewTool::new());
}

/// Register a `review` tool configured with live factories, replacing any bare
/// tool already registered under the `review` name.
///
/// The wiring layer (a crate that may depend on `swissarmyhammer-agent`) builds
/// the production [`AgentFactory`] from the session's `ModelConfig` and calls
/// this to swap the loader-only default for a tool whose three `review` ops can
/// drive the engine. `embedder_factory` is `None` to keep the loaded platform
/// embedder default; `concurrency` pins the pool worker count
/// (`review.concurrency`) when set.
///
/// Registration is by tool name, so this overwrites the bare `review` tool the
/// default [`register_review_tools`] installed.
pub fn register_review_tool_with_factories(
    registry: &mut ToolRegistry,
    agent_factory: AgentFactory,
    embedder_factory: Option<EmbedderFactory>,
    concurrency: Option<usize>,
) {
    let mut tool = ReviewTool::new()
        .with_agent_factory(agent_factory)
        .with_concurrency(concurrency);
    if let Some(embedder) = embedder_factory {
        tool = tool.with_embedder_factory(embedder);
    }
    registry.register(tool);
}

#[cfg(test)]
mod tests;
