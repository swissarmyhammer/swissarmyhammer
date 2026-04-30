//! Validator execution via ACP Agent.
//!
//! This module provides the `ValidatorRunner` which executes validators against
//! hook events by calling an Agent via the Agent Client Protocol (ACP).
//!
//! The agent is obtained from `AvpContext`, which armistices the connection
//! lifecycle and hands the runner a [`ConnectionTo<Agent>`] handle. Production
//! production runs against `ClaudeAgent` (or `LlamaAgent`); tests wire a
//! [`PlaybackAgent`] (or one of the inline mocks) to the same connection
//! shape.
//!
//! Validator partials can come from any of the standard validator directories:
//! - builtin/validators/_partials/
//! - $XDG_DATA_HOME/avp/validators/_partials/
//! - <AVP_DIR>/validators/_partials/
//!
//! This follows the same unified pattern as prompts and rules, using
//! [`ValidatorPartialAdapter`] which is a type alias for
//! `LibraryPartialAdapter<ValidatorLoader>`.
//!
//! ## ACP 0.11
//!
//! In ACP 0.10 the runner held an `Arc<dyn Agent + Send + Sync>` and dispatched
//! through the `Agent` trait. ACP 0.11 removed the trait — `Agent` is now a
//! unit [`Role`] marker — and replaced the dispatch surface with the typed
//! builder/handler runtime exposed via [`ConnectionTo<Agent>`]. The runner now
//! stores a clonable [`ConnectionTo<Agent>`] and issues each ACP request via
//! `connection.send_request(req).block_task().await`. Test mocks
//! ([`SessionRecordingAgent`], [`MaxTokensAgent`], [`SlowAgent`]) carry their
//! per-method behaviour as inherent `async fn`s implementing a project-local
//! [`MockAgent`] trait, and a [`MockAgentAdapter`] wraps each mock as a
//! [`ConnectTo<Client>`] middleware so it can be driven through a
//! `Client.builder().connect_with(...)` topology — mirroring the 0.11 SDK
//! pattern used by `claude-agent` (B8/B9), `llama-agent` (C9/C10), and
//! [`agent_client_protocol_extras::PlaybackAgent`].
//!
//! [`Role`]: agent_client_protocol::Role
//! [`PlaybackAgent`]: agent_client_protocol_extras::PlaybackAgent
//!
//! ## Parallel Execution
//!
//! Validators are executed in parallel with adaptive concurrency control:
//! - Initial concurrency is based on CPU count
//! - Concurrency is reduced when rate limits or timeouts are detected
//! - Concurrency gradually recovers after successful executions

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_client_protocol::schema::SessionNotification;
use agent_client_protocol::{Agent, ConnectionTo};
use futures::stream::{FuturesUnordered, StreamExt};
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
use swissarmyhammer_templating::HashMapPartialLoader;
use tokio::sync::{broadcast, Semaphore};

use crate::error::AvpError;
use crate::types::HookType;
use crate::validator::types::{compile_glob_patterns, matches_any_pattern};
use crate::validator::{
    create_executed_ruleset, create_executed_validator, emit_validator_result_log,
    emit_validator_result_log_with_reason, is_rate_limit_error, log_ruleset_result,
    log_validator_result, parse_validator_response,
    render_validator_prompt_with_partials_and_changed_files, ExecutedRuleSet, ExecutedValidator,
    RulePromptContext, RuleResult, RuleSet, Validator, ValidatorLoader, ValidatorResult,
    VALIDATOR_PROMPT_NAME,
};

/// Minimum concurrency level for parallel validator execution.
///
/// The adaptive throttling algorithm will never reduce concurrency below this
/// value, ensuring that at least one validator can always execute. This provides
/// a floor that guarantees forward progress even under severe rate limiting.
const MIN_CONCURRENCY: usize = 1;

/// Maximum concurrency level for parallel validator execution.
///
/// This caps the initial and recovered concurrency to prevent overwhelming the
/// API even on machines with many CPU cores. The value of 8 is chosen to balance
/// throughput with API friendliness.
const MAX_CONCURRENCY: usize = 8;

/// Factor by which to reduce concurrency when rate limits are detected.
///
/// This implements the "multiplicative decrease" phase of the AIMD-like algorithm.
/// When a rate limit is detected, concurrency is divided by this factor (halved).
/// A value of 2 provides aggressive backoff while avoiding complete stalls.
const CONCURRENCY_REDUCTION_FACTOR: usize = 2;

/// Number of consecutive successful executions before attempting to recover concurrency.
///
/// This implements the "additive increase" phase of the AIMD-like algorithm.
/// After this many successes without rate limiting, concurrency is increased by 1.
/// A value of 10 provides stability by ensuring the reduced concurrency is working
/// well before attempting to increase it.
const RECOVERY_THRESHOLD: usize = 10;

/// Per-rule cap on generation tokens for a single `agent.prompt()` call.
///
/// This is a defense-in-depth limit against runaway generation: a misbehaving
/// model, a confused prompt, or a parser bug shouldn't be able to generate
/// millions of tokens and lock the entire hook indefinitely.
///
/// ## Why 16384 (16k)
///
/// The original task recommendation was 4096 — comfortable headroom for a
/// non-reasoning model's typical output (a handful of tool calls + final JSON
/// verdict). We chose 16k instead for two reasons:
///
/// 1. **Reasoning models need room for `<think>` blocks.** Qwen3, the model
///    that motivated this cap, often produces several thousand tokens of
///    interior `<think>` reasoning before tool calls or a verdict. 4k caps
///    legitimate reasoning runs; 16k preserves them.
/// 2. **Alignment with `llama-agent`'s existing per-call cap.** llama-agent's
///    ACP server already hardcodes `MAX_GENERATION_TOKENS = 16384` at the
///    generation layer (see `llama-agent/src/acp/server.rs`). Picking the same
///    value means a runaway generation hits the same wall whether the cap is
///    enforced runner-side or agent-side, and we never have a confusing
///    asymmetry where one layer caps tighter than the other.
///
/// If a rule legitimately needs more than 16k generation tokens, the right
/// fix is to simplify the rule body or pick a more capable model — not to
/// raise this cap.
///
/// ## How it's communicated
///
/// The cap is sent to the agent via the `PromptRequest.meta` map under the key
/// `"max_tokens"`. When the agent honors the cap and stops because it was hit,
/// it returns `stop_reason: MaxTokens`, and [`build_rule_outcome_from_response`]
/// converts that into a loud rule failure (severity follows the rule's
/// effective severity).
///
/// ## Agent-side support status
///
/// - **`llama-agent`**: honors the cap. Reads `request.meta.max_tokens` and
///   uses it as an upper bound on the per-turn `max_tokens` it passes to the
///   generation request. A runaway generation hits this cap and surfaces as
///   `StopReason::MaxTokens` to the runner.
/// - **`claude-agent`**: honors the cap as of kanban task
///   `01KQ7VB868YZ7AWHNT16YB4XZR`. The Claude CLI does not accept a per-turn
///   `--max-tokens` flag, so claude-agent counts streamed output tokens at
///   the agent layer and aborts the subprocess + returns
///   `StopReason::MaxTokens` once the count exceeds the requested cap. The
///   cap is treated as tighter-only — it narrows but never widens
///   claude-agent's existing `max_tokens_per_turn` config.
///
/// The ACP spec explicitly says "Implementations MUST NOT make assumptions
/// about values at these keys" — agents are free to ignore `_meta`. Both
/// in-tree agents now opt in for symmetry with the runner's defense-in-depth
/// expectation.
pub(crate) const RULE_GENERATION_MAX_TOKENS: u64 = 16 * 1024;

/// Default per-rule wall-clock timeout in seconds.
///
/// Each rule inside a ruleset is wrapped in [`tokio::time::timeout`] with this
/// budget. If the agent's `prompt()` call (including its internal agentic loop
/// and any tool calls) does not return within this window, the rule is treated
/// as a passing-with-warning result whose `validator result` log line carries
/// `reason="timeout"`. The hook then proceeds to the next rule.
///
/// This is a wall-clock cap, not a token cap. The token cap
/// ([`RULE_GENERATION_MAX_TOKENS`]) prevents runaway *generation*; the wall
/// timeout prevents runaway *latency* (e.g. an agent silently waiting on a
/// dead MCP session, a stuck channel, or just an unusably slow model).
///
/// The rule-level frontmatter `timeout` field overrides this default per rule
/// (see [`crate::validator::Rule::effective_timeout`]). The default exists so
/// rules and rulesets that don't set their own timeout still have a sane cap.
pub(crate) const RULE_DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Default cap on how many rules of a ruleset run concurrently inside the
/// in-ruleset [`tokio::task::JoinSet`].
///
/// This is the "max in-flight" knob from kanban task `01KQAFFZDX40GSKXQVS0MTNDWV`:
/// the per-rule sessions each spin up an isolated agent (e.g. a fresh llama
/// session), so unbounded parallelism would saturate memory and turn a
/// performance fix into an OOM. A flat default of 4 in-flight rules gives
/// meaningful parallelism without scaling with CPU count — per-rule agents
/// are memory-bound (each holds a llama session), not CPU-bound, so a hard
/// cap is more appropriate than a CPU-derived heuristic.
///
/// The runtime can override this via the `AVP_RULE_MAX_IN_FLIGHT` environment
/// variable (positive integer). Values that fail to parse fall back to the
/// default.
pub(crate) const RULE_DEFAULT_PARALLELISM: usize = 4;

/// Environment variable name for overriding [`RULE_DEFAULT_PARALLELISM`] at
/// runtime.
pub(crate) const RULE_PARALLELISM_ENV_VAR: &str = "AVP_RULE_MAX_IN_FLIGHT";

/// Maximum size in bytes of the partial agent response included in a
/// `MaxTokens` failure message.
///
/// Larger partial responses are truncated and marked with a `[truncated]`
/// suffix to keep the failure message bounded (~2KB) while still preserving
/// enough context to diagnose what the model was generating when the cap fired.
const MAX_TOKENS_PARTIAL_RESPONSE_BYTES: usize = 4 * 1024;

/// Manages adaptive concurrency for parallel validator execution.
///
/// This struct implements an adaptive throttling strategy that balances throughput
/// with API rate limit compliance. The algorithm dynamically adjusts the number of
/// concurrent validator executions based on real-time feedback from the API.
///
/// ## Throttling Algorithm
///
/// The adaptive throttling works in three phases:
///
/// 1. **Initialization**: Concurrency starts at the CPU count, clamped between
///    [`MIN_CONCURRENCY`] (1) and [`MAX_CONCURRENCY`] (8). This provides a reasonable
///    starting point that scales with available compute resources.
///
/// 2. **Reduction (Backoff)**: When a rate limit, timeout, or capacity error is detected,
///    the concurrency is immediately halved (via [`CONCURRENCY_REDUCTION_FACTOR`]).
///    This exponential backoff quickly reduces load on the API. The reduction never
///    goes below [`MIN_CONCURRENCY`] to ensure forward progress.
///
/// 3. **Recovery**: After [`RECOVERY_THRESHOLD`] (10) consecutive successful executions,
///    concurrency is increased by 1, up to the original maximum. This gradual recovery
///    (additive increase) prevents oscillation and allows the system to find a stable
///    operating point.
///
/// ## Rate Limit Detection
///
/// The limiter detects rate limiting through error message analysis. The following
/// patterns trigger a reduction (see [`is_rate_limit_error`]):
/// - HTTP 429 status codes
/// - "rate limit" or "rate_limit" in error messages
/// - "too many requests" errors
/// - Timeout or "timed out" errors
/// - "overloaded" or "capacity" errors
///
/// ## Thread Safety
///
/// All state is managed with atomic operations, making the limiter safe to share
/// across concurrent tasks. The semaphore provides the actual concurrency enforcement.
///
/// ## Example
///
/// ```ignore
/// let limiter = ConcurrencyLimiter::new();
///
/// // Acquire permit before execution
/// let permit = limiter.acquire().await;
///
/// // Execute validator...
/// let result = execute_validator().await;
///
/// // Report outcome for adaptive throttling
/// if is_rate_limited {
///     limiter.report_rate_limit();
/// } else {
///     limiter.report_success();
/// }
///
/// // Permit is released when dropped
/// drop(permit);
/// ```
pub struct ConcurrencyLimiter {
    /// Current maximum concurrency level.
    ///
    /// This value adapts dynamically: it decreases when rate limits are detected
    /// and gradually increases after successful executions.
    max_concurrency: AtomicUsize,

    /// Original maximum concurrency (target for recovery).
    ///
    /// The limiter will never recover beyond this value, which is set at
    /// initialization based on CPU count.
    original_max: usize,

    /// Semaphore for enforcing the concurrency limit.
    ///
    /// Tasks must acquire a permit before execution. The semaphore permits
    /// are adjusted during recovery (increased) but not during reduction
    /// (existing permits are honored until released).
    semaphore: Arc<Semaphore>,

    /// Counter for consecutive successful executions.
    ///
    /// When this reaches [`RECOVERY_THRESHOLD`], concurrency is increased by 1
    /// and the counter resets. A rate limit event also resets this counter.
    success_count: AtomicUsize,
}

impl ConcurrencyLimiter {
    /// Create a new concurrency limiter with CPU-based initial concurrency.
    ///
    /// The initial concurrency is set to the number of CPU cores, clamped
    /// between [`MIN_CONCURRENCY`] and [`MAX_CONCURRENCY`]. This provides
    /// a reasonable starting point that adapts to the available hardware.
    pub fn new() -> Self {
        let cpu_count = num_cpus::get();
        // Use CPU count but cap at a reasonable maximum
        let max_concurrency = cpu_count.clamp(MIN_CONCURRENCY, MAX_CONCURRENCY);

        tracing::debug!(
            "ConcurrencyLimiter initialized with max_concurrency={} (cpus={})",
            max_concurrency,
            cpu_count
        );

        Self {
            max_concurrency: AtomicUsize::new(max_concurrency),
            original_max: max_concurrency,
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
            success_count: AtomicUsize::new(0),
        }
    }

    /// Get the current maximum concurrency level.
    ///
    /// This value may change dynamically as rate limits are detected
    /// and recovered from. The initial value is based on CPU count.
    pub fn current_max(&self) -> usize {
        self.max_concurrency.load(Ordering::Relaxed)
    }

    /// Acquire a permit to execute a validator.
    ///
    /// This method blocks until a permit is available, enforcing the
    /// current concurrency limit. The returned permit is automatically
    /// released when dropped.
    pub async fn acquire(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed unexpectedly")
    }

    /// Report a successful validator execution.
    ///
    /// After enough consecutive successes (defined by `RECOVERY_THRESHOLD`),
    /// the concurrency limit will be increased back toward the original maximum.
    pub fn report_success(&self) {
        let count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Try to recover concurrency after enough successes
        if count >= RECOVERY_THRESHOLD {
            self.success_count.store(0, Ordering::Relaxed);
            self.try_increase_concurrency();
        }
    }

    /// Report a rate limit or timeout error from the API.
    ///
    /// This resets the success counter and reduces the concurrency limit
    /// to avoid overwhelming the API. The limit will recover after
    /// subsequent successful executions.
    pub fn report_rate_limit(&self) {
        // Reset success counter
        self.success_count.store(0, Ordering::Relaxed);
        self.decrease_concurrency();
    }

    /// Decrease concurrency by the reduction factor.
    ///
    /// Implements the exponential backoff phase of adaptive throttling.
    /// The concurrency is divided by [`CONCURRENCY_REDUCTION_FACTOR`] (2),
    /// with a floor of [`MIN_CONCURRENCY`] (1) to ensure forward progress.
    ///
    /// Note: The semaphore permits are not immediately reduced. Existing
    /// permits are honored until released. The reduced `max_concurrency`
    /// prevents new permits from being issued beyond the new limit.
    fn decrease_concurrency(&self) {
        let current = self.max_concurrency.load(Ordering::Relaxed);
        let new_max = (current / CONCURRENCY_REDUCTION_FACTOR).max(MIN_CONCURRENCY);

        if new_max < current {
            self.max_concurrency.store(new_max, Ordering::Relaxed);
            tracing::warn!(
                "Rate limit detected - reducing concurrency from {} to {}",
                current,
                new_max
            );

            // Reduce semaphore permits
            // Note: We can't actually reduce permits on an existing semaphore,
            // but reducing max_concurrency will prevent new acquisitions from
            // exceeding the new limit over time.
        }
    }

    /// Try to increase concurrency back toward the original maximum.
    ///
    /// Implements the additive increase phase of adaptive throttling.
    /// Concurrency is increased by 1 (up to `original_max`), and a new
    /// semaphore permit is added to allow an additional concurrent execution.
    ///
    /// This gradual recovery (AIMD-like: Additive Increase, Multiplicative
    /// Decrease) prevents oscillation and allows the system to find a stable
    /// operating point just below the rate limit threshold.
    fn try_increase_concurrency(&self) {
        let current = self.max_concurrency.load(Ordering::Relaxed);

        if current < self.original_max {
            let new_max = (current + 1).min(self.original_max);
            self.max_concurrency.store(new_max, Ordering::Relaxed);

            // Add a permit to the semaphore
            self.semaphore.add_permits(1);

            tracing::info!(
                "Recovering concurrency from {} to {} (target: {})",
                current,
                new_max,
                self.original_max
            );
        }
    }
}

impl Default for ConcurrencyLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Drive a single-shot prompt turn against an ACP 0.11 agent connection.
///
/// Mirrors the legacy `claude_agent::execute_prompt_with_agent(&Agent, …)`
/// helper, but routes the three ACP requests (`initialize` → `new_session` →
/// `prompt`) through the typed [`ConnectionTo<Agent>`] handle instead of
/// direct trait dispatch.
///
/// # Lifecycle
/// 1. Send `InitializeRequest` to negotiate protocol version.
/// 2. Send `NewSessionRequest` with the current working directory.
/// 3. Spawn a per-session notification collector so streaming
///    `session/update` content is captured concurrently with the prompt.
/// 4. Send `PromptRequest` and wait for the typed response.
/// 5. Wait briefly for trailing notifications and return the assembled
///    [`claude_agent::CollectedResponse`].
///
/// # Errors
/// Returns [`claude_agent::AgentError::Internal`] (carrying the underlying
/// ACP error message) if any of the three round-trips fail. Notification
/// transport errors are swallowed inside the collector — they do not abort
/// the turn.
async fn execute_prompt_via_connection(
    agent: &ConnectionTo<Agent>,
    notifications: broadcast::Receiver<SessionNotification>,
    prompt: impl Into<String>,
) -> Result<claude_agent::CollectedResponse, claude_agent::AgentError> {
    use agent_client_protocol::schema::{
        ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, TextContent,
    };

    let prompt_text = prompt.into();

    // 1. initialize
    agent
        .send_request(InitializeRequest::new(1.into()))
        .block_task()
        .await
        .map_err(|e| {
            claude_agent::AgentError::Internal(format!("Failed to initialize agent: {}", e))
        })?;

    // 2. new_session
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
    let session_response = agent
        .send_request(NewSessionRequest::new(cwd))
        .block_task()
        .await
        .map_err(|e| {
            claude_agent::AgentError::Internal(format!("Failed to create session: {}", e))
        })?;
    let session_id = session_response.session_id;

    // 3. spawn notification collector before prompt() so streaming content is
    //    captured as it arrives.
    let (collector, collected_text, notification_count, _matched_count) =
        claude_agent::spawn_notification_collector(notifications, session_id.clone());

    // 4. prompt
    let prompt_request = PromptRequest::new(
        session_id,
        vec![ContentBlock::Text(TextContent::new(prompt_text))],
    );
    let prompt_response = agent
        .send_request(prompt_request)
        .block_task()
        .await
        .map_err(|e| {
            claude_agent::AgentError::Internal(format!("Failed to execute prompt: {}", e))
        })?;

    // 5. drain trailing notifications and assemble the collected response.
    let content = claude_agent::collect_response_content(
        collector,
        collected_text,
        notification_count,
        &prompt_response,
    )
    .await;

    Ok(claude_agent::CollectedResponse {
        content,
        stop_reason: prompt_response.stop_reason,
    })
}

/// Handle a validator execution response, creating the appropriate result.
///
/// This is a shared helper used by both parallel (`ValidatorTask`) and
/// single (`ValidatorRunner::execute_validator`) execution paths.
fn handle_execution_response(
    validator: &Validator,
    response: Result<claude_agent::CollectedResponse, claude_agent::AgentError>,
) -> (ExecutedValidator, bool) {
    match response {
        Ok(collected) => {
            // Log detailed information when content is empty to help diagnose issues
            if collected.content.trim().is_empty() {
                tracing::error!(
                    validator = %validator.name(),
                    stop_reason = ?collected.stop_reason,
                    content_length = collected.content.len(),
                    "Validator returned empty response"
                );
            }
            let result = parse_validator_response(
                &collected.content,
                &collected.stop_reason,
                validator.name(),
            );
            log_validator_result(validator.name(), &result);
            (create_executed_validator(validator, result), false)
        }
        Err(e) => handle_execution_error(validator, e),
    }
}

/// Handle a validator execution error, detecting rate limits.
fn handle_execution_error(
    validator: &Validator,
    error: claude_agent::AgentError,
) -> (ExecutedValidator, bool) {
    let error_str = error.to_string();
    let is_rate_limit = is_rate_limit_error(&error_str);

    if is_rate_limit {
        tracing::warn!(
            "Rate limit/timeout for validator '{}': {}",
            validator.name(),
            error
        );
    } else {
        tracing::error!(
            "Agent execution failed for validator '{}': {}",
            validator.name(),
            error
        );
    }

    let result =
        crate::validator::ValidatorResult::fail(format!("Agent execution failed: {}", error));
    (create_executed_validator(validator, result), is_rate_limit)
}

/// Create error result for prompt render failures.
///
/// This is a shared helper for both parallel and sequential execution paths.
fn create_render_error(validator: &Validator, error: &str) -> ExecutedValidator {
    tracing::error!(
        "Failed to render validator '{}' prompt: {}",
        validator.name(),
        error
    );
    create_executed_validator(
        validator,
        crate::validator::ValidatorResult::fail(format!("Failed to render prompt: {}", error)),
    )
}

/// Build the `PromptRequest` used for a single rule evaluation.
///
/// Wires the rule's rendered prompt into a `PromptRequest` for the given
/// session and attaches the per-rule `max_tokens` cap via the request's
/// `meta` map (key: `"max_tokens"`, value:
/// [`RULE_GENERATION_MAX_TOKENS`]).
///
/// The cap is communicated through `meta` because the ACP `PromptRequest`
/// schema does not have a first-class `max_tokens` field. Agents that honor
/// the cap will return `stop_reason: MaxTokens` when it fires; the runner
/// then converts that into a loud rule failure via
/// [`build_rule_outcome_from_response`].
///
/// # Arguments
///
/// * `session_id` - The fresh session to issue the prompt on
/// * `rule_prompt` - The rendered rule prompt body
fn build_rule_prompt_request(
    session_id: agent_client_protocol::schema::SessionId,
    rule_prompt: String,
) -> agent_client_protocol::schema::PromptRequest {
    use agent_client_protocol::schema::{ContentBlock, PromptRequest, TextContent};

    let mut meta = serde_json::Map::new();
    meta.insert(
        "max_tokens".to_string(),
        serde_json::json!(RULE_GENERATION_MAX_TOKENS),
    );

    PromptRequest::new(
        session_id,
        vec![ContentBlock::Text(TextContent::new(rule_prompt))],
    )
    .meta(meta)
}

/// Truncate a partial response string to at most
/// [`MAX_TOKENS_PARTIAL_RESPONSE_BYTES`] bytes.
///
/// If the response is longer than the limit, returns the truncated prefix with
/// a `" [truncated]"` marker appended. Truncation respects UTF-8 character
/// boundaries to avoid producing invalid UTF-8 in the output.
///
/// # Arguments
///
/// * `response` - The partial response text captured before the cap fired
fn truncate_partial_response_for_max_tokens(response: &str) -> String {
    if response.len() <= MAX_TOKENS_PARTIAL_RESPONSE_BYTES {
        return response.to_string();
    }

    let mut end = MAX_TOKENS_PARTIAL_RESPONSE_BYTES;
    while end > 0 && !response.is_char_boundary(end) {
        end -= 1;
    }

    format!("{} [truncated]", &response[..end])
}

/// Build a fail-loud failure message for a rule that hit the per-rule
/// `max_tokens` generation cap before producing a verdict.
///
/// The message references the cap value (so users see what was hit) and embeds
/// a truncated partial response (so users have a debug trail of what the model
/// was generating when it ran away).
fn build_max_tokens_failure_message(rule_name: &str, partial_response: &str) -> String {
    format!(
        "Validator rule '{rule}' exceeded max generation tokens ({cap}) without producing a verdict. \
This usually indicates a prompt/model mismatch — file an issue with the rule body and the partial \
response. partial response: {partial}",
        rule = rule_name,
        cap = RULE_GENERATION_MAX_TOKENS,
        partial = truncate_partial_response_for_max_tokens(partial_response),
    )
}

/// Map an `agent.prompt()` outcome onto a [`RuleOutcome`].
///
/// This is the post-processing half of [`ValidatorRunner::execute_rule_in_fresh_session`]:
/// given the prompt response (or error) and the streamed content collected
/// during the call, produce the appropriate [`RuleOutcome`] variant.
///
/// - `Ok(prompt_response)` with `stop_reason == MaxTokens` → [`RuleOutcome::Failure`]
///   referencing the cap and including a truncated partial response. This is a
///   defense-in-depth path: a runaway generation should fail loudly, not
///   silently pass.
/// - `Ok(prompt_response)` (any other stop reason) → parse the streamed content
///   into a verdict and wrap as [`RuleOutcome::Result`]. An empty content body
///   is logged as a warning but still parsed (the parser produces a fail-loud
///   outcome for empty/unparseable responses).
/// - `Err(...)` containing a rate-limit signature → [`RuleOutcome::RateLimited`]
///   so the caller can stop iterating remaining rules.
/// - Any other `Err(...)` → [`RuleOutcome::Failure`].
///
/// # Arguments
///
/// * `rule` - The rule that was evaluated (for naming/severity on the result)
/// * `ruleset` - The parent RuleSet (for severity inheritance)
/// * `response` - The raw `agent.prompt()` result
/// * `content` - The streamed assistant content collected during the prompt call
fn build_rule_outcome_from_response(
    rule: &crate::validator::Rule,
    ruleset: &RuleSet,
    response: Result<agent_client_protocol::schema::PromptResponse, agent_client_protocol::Error>,
    content: String,
) -> RuleOutcome {
    match response {
        Ok(prompt_response) => {
            // Defense-in-depth: if the agent stopped because it hit the
            // per-rule max_tokens cap, treat it as a loud rule failure rather
            // than trying to parse a truncated, half-finished response.
            if matches!(
                prompt_response.stop_reason,
                agent_client_protocol::schema::StopReason::MaxTokens
            ) {
                tracing::error!(
                    "RuleSet '{}' rule '{}' hit max_tokens cap ({}); failing rule",
                    ruleset.name(),
                    rule.name,
                    RULE_GENERATION_MAX_TOKENS,
                );
                return RuleOutcome::Failure(RuleResult {
                    rule_name: rule.name.clone(),
                    severity: rule.effective_severity(ruleset),
                    result: ValidatorResult::fail(build_max_tokens_failure_message(
                        &rule.name, &content,
                    )),
                });
            }

            if content.is_empty() {
                tracing::warn!(
                    "RuleSet '{}' rule '{}' returned empty content (stop_reason: {:?})",
                    ruleset.name(),
                    rule.name,
                    prompt_response.stop_reason
                );
            }

            let result =
                parse_validator_response(&content, &prompt_response.stop_reason, &rule.name);
            RuleOutcome::Result(RuleResult {
                rule_name: rule.name.clone(),
                severity: rule.effective_severity(ruleset),
                result,
            })
        }
        Err(e) => {
            let error_str = e.to_string();
            let rate_limited = is_rate_limit_error(&error_str);

            if rate_limited {
                tracing::warn!(
                    "Rate limit/timeout for RuleSet '{}' rule '{}': {}",
                    ruleset.name(),
                    rule.name,
                    e
                );
            } else {
                tracing::error!(
                    "Agent execution failed for RuleSet '{}' rule '{}': {}",
                    ruleset.name(),
                    rule.name,
                    e
                );
            }

            let rule_result = RuleResult {
                rule_name: rule.name.clone(),
                severity: rule.effective_severity(ruleset),
                result: ValidatorResult::fail(format!("Agent execution failed: {}", e)),
            };

            if rate_limited {
                RuleOutcome::RateLimited(rule_result)
            } else {
                RuleOutcome::Failure(rule_result)
            }
        }
    }
}

/// Task context for executing a single validator in parallel.
///
/// This struct captures all the data needed to execute a validator
/// asynchronously, enabling parallel execution via `FuturesUnordered`.
///
/// The task handles:
/// - Acquiring a concurrency permit before execution
/// - Rendering the validator prompt with partials
/// - Executing the prompt via the ACP agent
/// - Parsing the response and detecting rate limiting
struct ValidatorTask {
    /// Index of this task for preserving result order.
    idx: usize,
    /// The validator to execute.
    validator: Validator,
    /// The hook type that triggered validation.
    hook_type: HookType,
    /// Hook event context as JSON for template rendering.
    context: serde_json::Value,
    /// Optional list of changed files (for Stop hooks).
    changed_files: Option<Vec<String>>,
    /// Concurrency limiter for rate limit handling.
    concurrency: Arc<ConcurrencyLimiter>,
    /// Prompt library for template rendering.
    prompt_library: Arc<PromptLibrary>,
    /// Partial templates for Liquid includes.
    partials: HashMapPartialLoader,
    /// ACP connection to the agent for prompt execution.
    ///
    /// In ACP 0.10 this was an `Arc<dyn Agent + Send + Sync>` because `Agent`
    /// was a trait. ACP 0.11 replaced the trait with a [`Role`] marker plus the
    /// typed builder/handler runtime: outbound calls to the agent now flow over
    /// a [`ConnectionTo<Agent>`] handle obtained from
    /// `Client.builder().connect_with(...)`. The handle is `Clone`, so per-task
    /// fan-out uses `.clone()` rather than `Arc::clone`.
    ///
    /// [`Role`]: agent_client_protocol::Role
    agent: ConnectionTo<Agent>,
    /// Notification sender for streaming responses.
    notifications_tx: broadcast::Sender<SessionNotification>,
}

impl ValidatorTask {
    /// Execute the validator task asynchronously.
    ///
    /// Returns a tuple of:
    /// - `usize`: The task index for result ordering
    /// - `ExecutedValidator`: The validation result
    /// - `bool`: Whether rate limiting was detected
    async fn execute(self) -> (usize, ExecutedValidator, bool) {
        let _permit = self.concurrency.acquire().await;

        let prompt_text = match self.render_prompt() {
            Ok(text) => text,
            Err(e) => return self.render_error(e),
        };

        let notifications = self.notifications_tx.subscribe();
        let response =
            execute_prompt_via_connection(&self.agent, notifications, prompt_text).await;

        let (result, is_rate_limit) = handle_execution_response(&self.validator, response);

        // Report to concurrency limiter for adaptive throttling
        if is_rate_limit {
            self.concurrency.report_rate_limit();
        } else {
            self.concurrency.report_success();
        }

        (self.idx, result, is_rate_limit)
    }

    /// Render the validator prompt.
    fn render_prompt(&self) -> Result<String, String> {
        render_validator_prompt_with_partials_and_changed_files(
            &self.prompt_library,
            &self.validator,
            self.hook_type,
            &self.context,
            Some(&self.partials),
            self.changed_files.as_deref(),
        )
    }

    /// Create error result for render failures.
    fn render_error(self, error: String) -> (usize, ExecutedValidator, bool) {
        (
            self.idx,
            create_render_error(&self.validator, &error),
            false,
        )
    }
}

/// Executes validators via ACP Agent calls.
///
/// The `ValidatorRunner` handles:
/// 1. Rendering validator prompts using the `.system/validator` template
/// 2. Executing prompts via the provided Agent (in parallel with throttling)
/// 3. Parsing LLM responses into pass/fail results
/// 4. Creating `ExecutedValidator` results with metadata
///
/// Validator bodies support Liquid templating with partials, similar to rules
/// and prompts. The runner loads partials from builtin validators automatically.
///
/// ## Parallel Execution
///
/// Validators are executed in parallel with adaptive concurrency control.
/// The concurrency starts at CPU count and automatically reduces when
/// rate limits or timeouts are detected.
///
/// # Usage
///
/// Get the agent connection from `AvpContext` and create a runner:
/// ```ignore
/// let (connection, notifier) = context.agent().await?;
/// let runner = ValidatorRunner::new(connection, notifier)?;
/// ```
///
/// `connection` is a [`ConnectionTo<Agent>`]; `notifier` is the per-session
/// [`claude_agent::NotificationSender`] that the runner subscribes to for
/// streaming response content.
pub struct ValidatorRunner {
    /// Prompt library containing the .validator prompt
    prompt_library: Arc<PromptLibrary>,
    /// Validator partials for template rendering
    partials: HashMapPartialLoader,
    /// ACP connection to the agent for executing prompts.
    ///
    /// In ACP 0.10 this was an `Arc<dyn Agent + Send + Sync>` because `Agent`
    /// was a trait. ACP 0.11 replaced the trait with a [`Role`] marker plus the
    /// typed builder/handler runtime: outbound calls to the agent flow over a
    /// [`ConnectionTo<Agent>`] handle that the caller obtains by running
    /// `Client.builder().connect_with(transport, |conn| ...)` and handing the
    /// `conn` (or a clone) to the runner. The handle is `Clone`, so per-task
    /// fan-out uses `.clone()` rather than `Arc::clone`.
    ///
    /// [`Role`]: agent_client_protocol::Role
    agent: ConnectionTo<Agent>,
    /// Notification sender with per-session channels
    notifier: Arc<claude_agent::NotificationSender>,
    /// Concurrency limiter for parallel execution
    concurrency: Arc<ConcurrencyLimiter>,
    /// Cap on the number of rules that may run concurrently *inside* a single
    /// ruleset's [`tokio::task::JoinSet`].
    ///
    /// This is shared across all rulesets via `clone_for_task` so two parallel
    /// rulesets compete for the same pool of in-flight slots — preventing the
    /// `(num_rulesets × in_flight)` combinatorial explosion that would otherwise
    /// spawn one isolated agent session per rule per ruleset all at once.
    rule_concurrency: Arc<Semaphore>,
}

/// Internal outcome of evaluating a single rule in a fresh session.
///
/// Used by [`ValidatorRunner::execute_rule_in_fresh_session`] to communicate
/// back to the per-rule loop whether the result should be appended, whether
/// rate-limit handling kicks in, or whether it was a non-rate-limit failure.
enum RuleOutcome {
    /// Rule evaluated successfully (passed or failed verdict from the agent).
    Result(RuleResult),
    /// Rule failed with a rate-limit / capacity / transport-level timeout
    /// error reported by the agent. Adaptive throttling kicks in and the
    /// caller continues evaluating remaining rules (the parallel fan-out
    /// makes "stop the loop" no longer meaningful — the other rules are
    /// already in-flight).
    RateLimited(RuleResult),
    /// Rule failed with a non-rate-limit error (e.g. session creation failure
    /// or agent execution error). The caller should record the result and
    /// continue with the next rule.
    Failure(RuleResult),
    /// Rule did not complete within its wall-clock budget (see
    /// [`RULE_DEFAULT_TIMEOUT_SECS`] / per-rule `timeout` frontmatter).
    /// The wrapped [`RuleResult`] is a passing-with-warning verdict so the
    /// hook is not blocked on a stuck rule. The caller emits the
    /// `validator result ... reason="timeout"` log line via
    /// [`emit_rule_timeout_verdict`].
    Timeout(RuleResult),
}

impl ValidatorRunner {
    /// Create a new ValidatorRunner with the given agent and notification sender.
    ///
    /// Takes the NotificationSender which provides per-session channels.
    /// Each RuleSet session subscribes to its own channel - no cross-session
    /// notification bleed.
    pub fn new(
        agent: ConnectionTo<Agent>,
        notifier: Arc<claude_agent::NotificationSender>,
    ) -> Result<Self, AvpError> {
        let prompt_library = Self::load_prompt_library()?;
        let partials = Self::load_validator_partials();
        let concurrency = Arc::new(ConcurrencyLimiter::new());

        let rule_in_flight = resolve_rule_in_flight_cap();
        tracing::debug!(
            "ValidatorRunner rule-in-flight cap = {} (env override key: {})",
            rule_in_flight,
            RULE_PARALLELISM_ENV_VAR,
        );
        let rule_concurrency = Arc::new(Semaphore::new(rule_in_flight));

        Ok(Self {
            prompt_library: Arc::new(prompt_library),
            partials,
            agent,
            notifier,
            concurrency,
            rule_concurrency,
        })
    }

    /// Load and validate the prompt library.
    fn load_prompt_library() -> Result<PromptLibrary, AvpError> {
        let mut prompt_library = PromptLibrary::new();
        let mut resolver = PromptResolver::new();
        resolver
            .load_all_prompts(&mut prompt_library)
            .map_err(|e| AvpError::Agent(format!("Failed to load prompt library: {}", e)))?;

        // Verify .validator prompt exists
        prompt_library.get(VALIDATOR_PROMPT_NAME).map_err(|e| {
            AvpError::Agent(format!(
                ".system/validator prompt not found in prompt library: {}",
                e
            ))
        })?;

        tracing::debug!(".system/validator prompt loaded successfully");
        Ok(prompt_library)
    }

    /// Load validator partials from all sources (builtin + user + project).
    ///
    /// This follows the same pattern as prompts and rules - partials are loaded
    /// from all validator directories with the standard precedence:
    /// 1. Builtin validators (lowest precedence)
    /// 2. User validators ($XDG_DATA_HOME/avp/validators)
    /// 3. Project validators (<AVP_DIR>/validators) (highest precedence)
    fn load_validator_partials() -> HashMapPartialLoader {
        // Create a loader and load all validators (including partials)
        let mut loader = ValidatorLoader::new();

        // First load builtins
        crate::load_builtins(&mut loader);

        // Then load from filesystem (user + project directories)
        // This uses VirtualFileSystem<AvpConfig> internally
        if let Err(e) = loader.load_all() {
            tracing::warn!("Failed to load some validators for partials: {}", e);
        }

        // Extract partials from all loaded validators
        let partials = Self::extract_partials_from_loader(&loader);
        tracing::debug!(
            "Loaded {} validator partials from {} total validators",
            partials.len(),
            loader.len()
        );
        partials
    }

    /// Extract partials from a ValidatorLoader.
    ///
    /// Uses shared partial detection logic from executor module.
    /// Also loads partials from RuleSet _partials/ directories.
    fn extract_partials_from_loader(loader: &ValidatorLoader) -> HashMapPartialLoader {
        use crate::validator::{add_partial_with_aliases, is_partial};

        let mut partials = HashMapPartialLoader::empty();

        // Load partials from legacy validators (for backward compatibility)
        for validator in loader.list() {
            let name = validator.name();
            let body = &validator.body;

            if is_partial(name, body) {
                add_partial_with_aliases(&mut partials, name, body);
            }
        }

        // Load partials from RuleSet _partials/ directories
        for ruleset in loader.list_rulesets() {
            let partials_dir = ruleset.base_path.join("_partials");
            if partials_dir.exists() && partials_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&partials_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md")
                        {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                                    // Strip .liquid suffix if present (e.g. "test-remediation.liquid" -> "test-remediation")
                                    let base_name = name.strip_suffix(".liquid").unwrap_or(name);

                                    // Register with RuleSet-scoped name
                                    let scoped_name =
                                        format!("{}/_partials/{}", ruleset.name(), base_name);
                                    partials.add(&scoped_name, &content);
                                    // Also register with just the base name for easy reference
                                    partials.add(base_name, &content);
                                    // And the full name with .liquid for explicit references
                                    if base_name != name {
                                        partials.add(name, &content);
                                    }
                                    tracing::debug!(
                                        "Loaded partial '{}' from RuleSet '{}' _partials/",
                                        name,
                                        ruleset.name()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        partials
    }

    /// Execute a single validator against a hook event context.
    ///
    /// Returns an `ExecutedValidator` with the result and validator metadata.
    /// Also returns a boolean indicating if a rate limit was detected.
    ///
    /// The `changed_files` parameter is an optional list of files that changed during
    /// this turn. It is typically provided for Stop hooks to enable validators to
    /// focus on changed files.
    pub async fn execute_validator(
        &self,
        validator: &Validator,
        hook_type: HookType,
        context: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> (ExecutedValidator, bool) {
        // Render the validator prompt using shared utility
        let prompt_result = render_validator_prompt_with_partials_and_changed_files(
            &self.prompt_library,
            validator,
            hook_type,
            context,
            Some(&self.partials),
            changed_files,
        );

        let prompt_text = match prompt_result {
            Ok(text) => text,
            Err(e) => return (create_render_error(validator, &e), false),
        };

        let notifications = self.notifier.sender().subscribe();
        let response =
            execute_prompt_via_connection(&self.agent, notifications, prompt_text).await;

        handle_execution_response(validator, response)
    }

    /// Execute multiple validators against a hook event context.
    ///
    /// Executes validators in parallel with adaptive concurrency control.
    /// Concurrency starts at CPU count and reduces when rate limits are detected.
    /// Returns a list of `ExecutedValidator` results.
    ///
    /// The `changed_files` parameter is an optional list of files that changed during
    /// this turn. It is typically provided for Stop hooks to enable validators to
    /// focus on changed files.
    pub async fn execute_validators(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        context: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Vec<ExecutedValidator> {
        if validators.is_empty() {
            return Vec::new();
        }

        self.log_execution_start(validators.len());
        let futures = self.create_validator_tasks(validators, hook_type, context, changed_files);
        Self::collect_results(futures, validators.len()).await
    }

    /// Log the start of parallel validator execution.
    fn log_execution_start(&self, count: usize) {
        tracing::debug!(
            "Executing {} validators in parallel (max_concurrency={})",
            count,
            self.concurrency.current_max()
        );
    }

    /// Create validator tasks for parallel execution.
    fn create_validator_tasks(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        context: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> FuturesUnordered<impl std::future::Future<Output = (usize, ExecutedValidator, bool)>> {
        let changed_files_owned: Option<Vec<String>> = changed_files.map(|f| f.to_vec());
        let futures = FuturesUnordered::new();

        for (idx, validator) in validators.iter().enumerate() {
            let task = self.build_validator_task(
                idx,
                (*validator).clone(),
                hook_type,
                context.clone(),
                changed_files_owned.clone(),
            );
            futures.push(task.execute());
        }

        futures
    }

    /// Build a single validator task with all required context.
    fn build_validator_task(
        &self,
        idx: usize,
        validator: Validator,
        hook_type: HookType,
        context: serde_json::Value,
        changed_files: Option<Vec<String>>,
    ) -> ValidatorTask {
        ValidatorTask {
            idx,
            validator,
            hook_type,
            context,
            changed_files,
            concurrency: Arc::clone(&self.concurrency),
            prompt_library: Arc::clone(&self.prompt_library),
            partials: self.partials.clone(),
            agent: self.agent.clone(),
            notifications_tx: self.notifier.sender(),
        }
    }

    /// Collect validator results from futures, preserving original order.
    async fn collect_results(
        mut futures: FuturesUnordered<
            impl std::future::Future<Output = (usize, ExecutedValidator, bool)>,
        >,
        count: usize,
    ) -> Vec<ExecutedValidator> {
        let mut results: Vec<Option<ExecutedValidator>> = vec![None; count];
        let mut blocked = false;

        while let Some((idx, result, _is_rate_limit)) = futures.next().await {
            if result.is_blocking() && !blocked {
                blocked = true;
                tracing::info!(
                    "Validator '{}' blocked - in-flight validators will complete but result is blocked",
                    result.name
                );
            }
            results[idx] = Some(result);
        }

        results.into_iter().flatten().collect()
    }

    /// Get the current concurrency level for parallel validator execution.
    ///
    /// This value adapts dynamically based on API rate limits and successful
    /// executions. It starts at the CPU count and reduces when rate limits
    /// are detected, then recovers after consecutive successes.
    pub fn current_concurrency(&self) -> usize {
        self.concurrency.current_max()
    }

    // ========================================================================
    // RuleSet Execution (New Architecture)
    // ========================================================================

    /// Execute a single RuleSet as one agent session with conversational rule evaluation.
    ///
    /// This starts one agent session for the RuleSet and evaluates each rule
    /// sequentially as part of the conversation, maintaining context across rules.
    ///
    /// # Arguments
    ///
    /// * `ruleset` - The RuleSet to execute
    /// * `hook_type` - The hook event type
    /// * `context` - Hook event context as JSON
    /// * `changed_files` - Optional list of changed files (for Stop hooks)
    ///
    /// # Returns
    ///
    /// Returns an `ExecutedRuleSet` with results for all rules, and a boolean
    /// indicating if rate limiting was detected.
    pub async fn execute_ruleset(
        &self,
        ruleset: &RuleSet,
        hook_type: HookType,
        context: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> (ExecutedRuleSet, bool) {
        // `hook_type` is consumed in two places:
        //   1. Threaded into the per-rule `validator result` log line emitted
        //      below as soon as each rule's verdict is known. This is what
        //      production scrapes via `grep validator result .avp/log` to
        //      tell which hook ran which rule (e.g. Stop vs PostToolUse).
        //   2. Otherwise the hook event semantics are encoded in `context`
        //      (which carries `hook_event_name` plus any embedded diff
        //      blocks via `prepare_validator_context`).
        //
        // `changed_files` is threaded down to the per-rule prompt builder so
        // the rendered rule prompt includes a `## Files Changed This Turn`
        // section. The diff blocks alone do not explicitly enumerate the
        // file list (and a per-ruleset diff filter can drop files entirely),
        // so the explicit list is what gives the validator orientation about
        // which paths to focus on.

        // Acquire concurrency permit for this RuleSet
        let _permit = self.concurrency.acquire().await;

        tracing::debug!(
            "Executing RuleSet '{}' with {} rules (fresh session per rule)",
            ruleset.name(),
            ruleset.rules.len()
        );

        // Initialize the agent once for the RuleSet. ACP `initialize` is an
        // agent-level handshake (capabilities, version negotiation) and is
        // independent of session lifecycles, so it does not need to be
        // repeated per rule.
        //
        // ACP 0.11: outbound requests flow over `ConnectionTo<Agent>` rather
        // than direct trait calls. `send_request(req).block_task().await`
        // drives the JSON-RPC round-trip and yields the typed response.
        use agent_client_protocol::schema::InitializeRequest;
        let init_request = InitializeRequest::new(1.into());
        if let Err(e) = self.agent.send_request(init_request).block_task().await {
            tracing::error!(
                "Failed to initialize agent for RuleSet '{}': {}",
                ruleset.name(),
                e
            );
            return (
                create_executed_ruleset(
                    ruleset,
                    vec![RuleResult {
                        rule_name: "session-init".to_string(),
                        severity: ruleset.manifest.severity,
                        result: ValidatorResult::fail(format!("Failed to initialize agent: {}", e)),
                    }],
                ),
                false,
            );
        }

        let hook_type_str = hook_type.to_string();

        // Execute rules concurrently inside the ruleset, capped by the shared
        // `rule_concurrency` semaphore. Each rule still runs in its own fresh
        // session — the JoinSet-style fan-out only affects scheduling, not
        // session isolation. Rules that exceed their wall-clock timeout are
        // recorded as passing-with-warning so a single stuck rule cannot block
        // the entire hook.
        let mut futures = FuturesUnordered::new();
        for (idx, rule) in ruleset.rules.iter().enumerate() {
            let timeout_secs = rule.effective_timeout(ruleset) as u64;
            let timeout_secs = if timeout_secs == 0 {
                RULE_DEFAULT_TIMEOUT_SECS
            } else {
                timeout_secs
            };
            futures.push(async move {
                let outcome = self
                    .execute_rule_with_timeout(rule, ruleset, context, changed_files, timeout_secs)
                    .await;
                (idx, outcome)
            });
        }

        // Collect outcomes in original rule order so downstream consumers (and
        // tests that assert on `rule_results[0]`) see a deterministic ordering
        // regardless of which rule finishes first.
        let mut indexed_outcomes: Vec<Option<RuleOutcome>> =
            (0..ruleset.rules.len()).map(|_| None).collect();
        let mut is_rate_limited = false;
        while let Some((idx, outcome)) = futures.next().await {
            indexed_outcomes[idx] = Some(outcome);
        }

        let mut rule_results = Vec::with_capacity(ruleset.rules.len());
        for outcome in indexed_outcomes.into_iter().flatten() {
            match outcome {
                RuleOutcome::Result(rule_result) => {
                    emit_rule_verdict(ruleset.name(), &rule_result, &hook_type_str);
                    rule_results.push(rule_result);
                    self.concurrency.report_success();
                }
                RuleOutcome::RateLimited(rule_result) => {
                    is_rate_limited = true;
                    self.concurrency.report_rate_limit();
                    emit_rule_verdict(ruleset.name(), &rule_result, &hook_type_str);
                    rule_results.push(rule_result);
                }
                RuleOutcome::Failure(rule_result) => {
                    emit_rule_verdict(ruleset.name(), &rule_result, &hook_type_str);
                    rule_results.push(rule_result);
                }
                RuleOutcome::Timeout(rule_result) => {
                    emit_rule_timeout_verdict(ruleset.name(), &rule_result, &hook_type_str);
                    rule_results.push(rule_result);
                    self.concurrency.report_success();
                }
            }
        }

        let executed = create_executed_ruleset(ruleset, rule_results);
        log_ruleset_result(ruleset.name(), &executed);

        (executed, is_rate_limited)
    }

    /// Execute a single rule with both an in-flight cap and a wall-clock timeout.
    ///
    /// This is the inner per-rule worker driven by `execute_ruleset`'s
    /// `JoinSet`-style fan-out. It performs three things in order:
    ///
    /// 1. Acquires a permit from `rule_concurrency` to enforce the shared
    ///    "max in-flight rules across all rulesets" cap. The permit is
    ///    automatically released when this function returns.
    /// 2. Wraps the underlying `execute_rule_in_fresh_session` call in
    ///    [`tokio::time::timeout`] so a stuck agent / dead MCP session /
    ///    runaway tool loop cannot drag the whole hook past the parent
    ///    process's tolerance window.
    /// 3. On timeout, returns [`RuleOutcome::Timeout`] carrying a
    ///    passing-with-warning [`RuleResult`] so the rest of the ruleset
    ///    continues. The caller logs this as `validator result ... reason="timeout"`.
    ///
    /// # Arguments
    ///
    /// * `rule` - The rule to evaluate
    /// * `ruleset` - The parent RuleSet (for severity inheritance and partials)
    /// * `context` - Hook event context as JSON
    /// * `changed_files` - Optional list of paths changed during the turn
    /// * `timeout_secs` - Per-rule wall-clock budget in seconds
    async fn execute_rule_with_timeout(
        &self,
        rule: &crate::validator::Rule,
        ruleset: &RuleSet,
        context: &serde_json::Value,
        changed_files: Option<&[String]>,
        timeout_secs: u64,
    ) -> RuleOutcome {
        // Acquire the in-flight permit *outside* the timeout so blocking on
        // the semaphore is not counted against the rule's own wall budget.
        // This matches the user-visible contract: a rule's `timeout` is "how
        // long the agent has to think", not "how long it has to wait its
        // turn behind other rules".
        let _permit = self
            .rule_concurrency
            .clone()
            .acquire_owned()
            .await
            .expect("rule_concurrency semaphore closed unexpectedly");

        let timeout_duration = std::time::Duration::from_secs(timeout_secs);
        match tokio::time::timeout(
            timeout_duration,
            self.execute_rule_in_fresh_session(rule, ruleset, context, changed_files),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(_elapsed) => {
                tracing::warn!(
                    "RuleSet '{}' rule '{}' exceeded wall-clock timeout of {}s; treating as pass-with-warning",
                    ruleset.name(),
                    rule.name,
                    timeout_secs,
                );
                RuleOutcome::Timeout(RuleResult {
                    rule_name: rule.name.clone(),
                    severity: rule.effective_severity(ruleset),
                    result: ValidatorResult::pass(format!(
                        "Rule '{}' did not complete within {}s; skipped (timeout)",
                        rule.name, timeout_secs,
                    )),
                })
            }
        }
    }

    /// Execute a single rule inside a freshly-created agent session.
    ///
    /// Each call creates a new ACP session via `new_session` so the rule's
    /// prompt is evaluated with no conversation history from prior rules.
    /// The rule prompt itself is self-contained (hook context + rule body +
    /// response-format instructions), rendered via [`RulePromptContext`].
    ///
    /// The agent's internal agentic loop within `prompt()` is preserved —
    /// tool calls and multi-turn behavior happen inside the single
    /// `agent.prompt()` invocation, exactly as before.
    ///
    /// # Arguments
    ///
    /// * `rule` - The rule to evaluate
    /// * `ruleset` - The parent RuleSet (for severity inheritance and partials)
    /// * `context` - Hook event context as JSON, rendered into the rule prompt
    /// * `changed_files` - Optional list of file paths changed during the
    ///   turn. Threaded into the rule prompt so the validator sees a
    ///   `## Files Changed This Turn` section.
    ///
    /// # Returns
    ///
    /// A [`RuleOutcome`] describing the result of the rule evaluation. The
    /// caller maps this onto `rule_results` / rate-limit state.
    async fn execute_rule_in_fresh_session(
        &self,
        rule: &crate::validator::Rule,
        ruleset: &RuleSet,
        context: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> RuleOutcome {
        // Step 1: create a fresh session, or fail-fast with an outcome.
        let session_id = match self.create_rule_session(rule, ruleset).await {
            Ok(id) => id,
            Err(outcome) => return outcome,
        };

        // Step 2: send the rule prompt while collecting streaming content.
        let (response, content) = self
            .send_rule_prompt_and_collect(rule, ruleset, context, changed_files, session_id)
            .await;

        // Step 3: map the response into a RuleOutcome.
        build_rule_outcome_from_response(rule, ruleset, response, content)
    }

    /// Create a fresh ACP session for a single rule.
    ///
    /// On success returns the agent-assigned `SessionId`. On failure returns
    /// the appropriate [`RuleOutcome::Failure`] in the `Err` branch so the
    /// caller can short-circuit without nested matches.
    ///
    /// # Arguments
    ///
    /// * `rule` - The rule the session is being created for (used for logging
    ///   and for constructing the failure result)
    /// * `ruleset` - The parent RuleSet (for severity inheritance on failure)
    async fn create_rule_session(
        &self,
        rule: &crate::validator::Rule,
        ruleset: &RuleSet,
    ) -> Result<agent_client_protocol::schema::SessionId, RuleOutcome> {
        use agent_client_protocol::schema::NewSessionRequest;

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
        // ACP 0.11: dispatch via the typed `ConnectionTo<Agent>` handle
        // instead of direct trait dispatch.
        match self
            .agent
            .send_request(NewSessionRequest::new(cwd))
            .block_task()
            .await
        {
            Ok(resp) => {
                let session_id = resp.session_id;
                tracing::debug!(
                    "RuleSet '{}' rule '{}' got session_id={}",
                    ruleset.name(),
                    rule.name,
                    session_id
                );
                Ok(session_id)
            }
            Err(e) => {
                tracing::error!(
                    "Failed to create session for RuleSet '{}' rule '{}': {}",
                    ruleset.name(),
                    rule.name,
                    e
                );
                Err(RuleOutcome::Failure(RuleResult {
                    rule_name: rule.name.clone(),
                    severity: rule.effective_severity(ruleset),
                    result: ValidatorResult::fail(format!("Failed to create session: {}", e)),
                }))
            }
        }
    }

    /// Build the rule prompt, send it on `session_id`, and collect the
    /// streamed assistant content.
    ///
    /// Spawns a per-session notification collector before issuing
    /// `agent.prompt()` so streaming text is captured as it arrives, then
    /// aborts the collector once `prompt()` returns. The agent's internal
    /// agentic loop (tool use, multi-turn) is preserved inside the single
    /// `prompt()` call.
    ///
    /// # Arguments
    ///
    /// * `rule` - The rule to evaluate
    /// * `ruleset` - The parent RuleSet (for partial resolution)
    /// * `context` - Hook event context as JSON, rendered into the rule prompt
    /// * `changed_files` - Optional list of changed files for the turn,
    ///   rendered into the prompt as `## Files Changed This Turn`
    /// * `session_id` - The fresh session to issue the prompt on
    ///
    /// # Returns
    ///
    /// A pair of:
    /// - the raw `prompt()` result (the agent's `PromptResponse` or its error)
    /// - the collected streaming content (empty string if nothing streamed)
    async fn send_rule_prompt_and_collect(
        &self,
        rule: &crate::validator::Rule,
        ruleset: &RuleSet,
        context: &serde_json::Value,
        changed_files: Option<&[String]>,
        session_id: agent_client_protocol::schema::SessionId,
    ) -> (
        Result<agent_client_protocol::schema::PromptResponse, agent_client_protocol::Error>,
        String,
    ) {
        // Build the self-contained rule prompt. `hook_context` carries the
        // pre-rendered YAML + diff blocks for the rule to inspect, and
        // `changed_files` enumerates the paths the validator should focus on.
        let mut rule_ctx = RulePromptContext::with_partials(rule, ruleset, Some(&self.partials));
        rule_ctx.hook_context = Some(context);
        rule_ctx.changed_files = changed_files;
        let rule_prompt = rule_ctx.render();

        let rule_request = build_rule_prompt_request(session_id.clone(), rule_prompt);

        // Spawn the per-session collector before sending the prompt so it
        // captures streaming notifications as they arrive.
        let rule_notifications = self.notifier.subscribe_session(&session_id.0);
        let (rule_collector, rule_text, _, _) =
            claude_agent::spawn_notification_collector(rule_notifications, session_id);

        // ACP 0.11: dispatch via the typed `ConnectionTo<Agent>` handle.
        let response = self.agent.send_request(rule_request).block_task().await;

        // prompt() returned - collector has already received all content
        rule_collector.abort();

        let content = rule_text.lock().await.clone();
        (response, content)
    }

    /// Execute multiple RuleSets against a hook event context.
    ///
    /// Executes RuleSets in parallel with adaptive concurrency control.
    /// Each RuleSet runs in its own agent session with rules evaluated sequentially.
    ///
    /// # Arguments
    ///
    /// * `rulesets` - Slice of RuleSets to execute
    /// * `hook_type` - The hook event type
    /// * `context` - Hook event context as JSON
    /// * `changed_files` - Optional list of changed files (for Stop hooks)
    ///
    /// # Returns
    ///
    /// Returns a vector of `ExecutedRuleSet` results, one per RuleSet.
    pub async fn execute_rulesets(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
        context: &serde_json::Value,
        changed_files: Option<&[String]>,
        raw_diffs: Option<&[crate::turn::FileDiff]>,
    ) -> Vec<ExecutedRuleSet> {
        if rulesets.is_empty() {
            return Vec::new();
        }

        tracing::debug!(
            "Executing {} RuleSets in parallel (max_concurrency={})",
            rulesets.len(),
            self.concurrency.current_max()
        );

        // Create futures for parallel execution
        let changed_files_owned: Option<Vec<String>> = changed_files.map(|f| f.to_vec());
        let mut futures = FuturesUnordered::new();

        for (idx, ruleset) in rulesets.iter().enumerate() {
            let ruleset_clone = (*ruleset).clone();
            let hook_type_clone = hook_type;
            // Filter changed_files per-ruleset: if the ruleset has match.files
            // patterns, only pass through files matching those patterns.
            let changed_files_clone =
                filter_changed_files_for_ruleset(changed_files_owned.as_deref(), ruleset);

            // Filter diffs per-ruleset and prepare context with filtered diffs.
            // This ensures each ruleset only sees diffs for files matching its
            // file patterns (e.g., a *.rs ruleset won't see .py diffs).
            let context_clone = if raw_diffs.is_some() {
                let filtered_diffs = filter_diffs_for_ruleset(raw_diffs, ruleset);
                crate::turn::prepare_validator_context(context.clone(), filtered_diffs.as_deref())
            } else {
                context.clone()
            };

            // Create a task that executes this RuleSet
            let runner = self.clone_for_task();

            futures.push(async move {
                let (result, is_rate_limit) = runner
                    .execute_ruleset(
                        &ruleset_clone,
                        hook_type_clone,
                        &context_clone,
                        changed_files_clone.as_deref(),
                    )
                    .await;
                (idx, result, is_rate_limit)
            });
        }

        // Collect results preserving order
        let mut results: Vec<Option<ExecutedRuleSet>> = vec![None; rulesets.len()];

        while let Some((idx, result, _is_rate_limit)) = futures.next().await {
            results[idx] = Some(result);
        }

        results.into_iter().flatten().collect()
    }

    /// Clone the runner for task execution.
    ///
    /// Creates a lightweight clone that shares the underlying resources
    /// (agent, prompt library, partials, concurrency limiter).
    fn clone_for_task(&self) -> Self {
        Self {
            prompt_library: Arc::clone(&self.prompt_library),
            partials: self.partials.clone(),
            agent: self.agent.clone(),
            notifier: Arc::clone(&self.notifier),
            concurrency: Arc::clone(&self.concurrency),
            rule_concurrency: Arc::clone(&self.rule_concurrency),
        }
    }
}

/// Emit the per-rule `validator result` log line as soon as a rule's verdict
/// is known.
///
/// This is the eager counterpart to the deferred batch emit that previously
/// fired only after every RuleSet finished. Emitting per-rule guarantees that:
///
/// - A hook that times out mid-ruleset still has logs for the rules that
///   completed (the failure mode captured by kanban task
///   `01KQAFE5WGYJK3HZ8WE3B8N86K`).
/// - The Stop and PostToolUse paths produce identical `validator result`
///   lines — same field order, same level=INFO — because both flow through
///   the same `execute_ruleset` call site.
///
/// The validator name is qualified as `<ruleset>:<rule>` so production log
/// scrapes (`grep code-quality .avp/log`) match the same shape regardless
/// of which hook fired the rule.
fn emit_rule_verdict(ruleset_name: &str, rule_result: &RuleResult, hook_type_str: &str) {
    let qualified_name = format!("{}:{}", ruleset_name, rule_result.rule_name);
    emit_validator_result_log(
        &qualified_name,
        rule_result.passed(),
        hook_type_str,
        rule_result.message(),
    );
}

/// Emit the per-rule `validator result` log line for a rule that hit its
/// wall-clock timeout.
///
/// This is the timeout-specific counterpart to [`emit_rule_verdict`]. It tags
/// the log line with `reason="timeout"` so production scrapes can distinguish
/// "rule passed because the agent said so" from "rule passed because the
/// hook gave up waiting for the agent". The verdict itself remains a pass —
/// the timeout handler intentionally produces a passing [`RuleResult`] so a
/// stuck rule does not block the hook.
///
/// The validator name is qualified as `<ruleset>:<rule>` to match the shape
/// produced by [`emit_rule_verdict`].
fn emit_rule_timeout_verdict(ruleset_name: &str, rule_result: &RuleResult, hook_type_str: &str) {
    let qualified_name = format!("{}:{}", ruleset_name, rule_result.rule_name);
    emit_validator_result_log_with_reason(
        &qualified_name,
        rule_result.passed(),
        hook_type_str,
        rule_result.message(),
        "timeout",
    );
}

/// Resolve the cap on rules that may run concurrently inside a single
/// ruleset's [`tokio::task::JoinSet`].
///
/// Reads the `AVP_RULE_MAX_IN_FLIGHT` environment variable for an explicit
/// runtime override (positive integer). When unset or unparseable, the cap
/// defaults to [`RULE_DEFAULT_PARALLELISM`].
///
/// Per-rule agents are memory-bound (each holds an isolated llama session),
/// so a flat default cap is more appropriate than a CPU-derived heuristic.
fn resolve_rule_in_flight_cap() -> usize {
    if let Ok(raw) = std::env::var(RULE_PARALLELISM_ENV_VAR) {
        if let Ok(parsed) = raw.parse::<usize>() {
            if parsed >= 1 {
                return parsed;
            }
        }
        tracing::warn!(
            "Invalid {} value '{}'; falling back to default of {}",
            RULE_PARALLELISM_ENV_VAR,
            raw,
            RULE_DEFAULT_PARALLELISM,
        );
    }
    RULE_DEFAULT_PARALLELISM
}

/// Filter changed files for a specific RuleSet based on its match.files patterns.
///
/// When a RuleSet has `match.files` glob patterns, only the changed files matching
/// those patterns are returned. This gives each validator focused context instead
/// of every file changed in the turn.
///
/// If the RuleSet has no file patterns, all changed files are passed through unchanged.
/// If `changed_files` is `None`, returns `None`.
///
/// # Arguments
///
/// * `changed_files` - The full list of changed files for the turn
/// * `ruleset` - The RuleSet whose match.files patterns determine the filter
///
/// # Returns
///
/// Filtered list of changed files, or `None` if input was `None`.
pub fn filter_changed_files_for_ruleset(
    changed_files: Option<&[String]>,
    ruleset: &RuleSet,
) -> Option<Vec<String>> {
    let files = changed_files?;

    let patterns = match &ruleset.manifest.match_criteria {
        Some(mc) if !mc.files.is_empty() => &mc.files,
        _ => return Some(files.to_vec()),
    };

    let compiled = compile_glob_patterns(patterns);
    let filtered: Vec<String> = files
        .iter()
        .filter(|file| matches_any_pattern(file, &compiled))
        .cloned()
        .collect();

    Some(filtered)
}

/// Filter diffs for a specific RuleSet based on its match.files patterns.
///
/// Analogous to [`filter_changed_files_for_ruleset`] but operates on `FileDiff`
/// structs. When a RuleSet has `match.files` glob patterns, only diffs for files
/// matching those patterns are returned. This gives each validator focused diff
/// context instead of every file changed in the turn.
///
/// If the RuleSet has no file patterns, all diffs are passed through unchanged.
/// If `diffs` is `None`, returns `None`.
pub fn filter_diffs_for_ruleset(
    diffs: Option<&[crate::turn::FileDiff]>,
    ruleset: &RuleSet,
) -> Option<Vec<crate::turn::FileDiff>> {
    let diffs = diffs?;

    let patterns = match &ruleset.manifest.match_criteria {
        Some(mc) if !mc.files.is_empty() => &mc.files,
        _ => return Some(diffs.to_vec()),
    };

    let compiled = compile_glob_patterns(patterns);
    let filtered: Vec<crate::turn::FileDiff> = diffs
        .iter()
        .filter(|diff| {
            let path_str = diff.path.display().to_string();
            matches_any_pattern(&path_str, &compiled)
        })
        .cloned()
        .collect();

    Some(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Number of rate limit reports to test minimum bound behavior.
    const TEST_RATE_LIMIT_ITERATIONS: usize = 10;

    /// Number of success reports below recovery threshold for testing.
    const TEST_SUCCESS_ITERATIONS_BELOW_THRESHOLD: usize = 5;

    // =========================================================================
    // ConcurrencyLimiter Tests
    // =========================================================================

    #[test]
    fn test_concurrency_limiter_new() {
        let limiter = ConcurrencyLimiter::new();
        let max = limiter.current_max();
        // Should be between MIN_CONCURRENCY and MAX_CONCURRENCY
        assert!(max >= MIN_CONCURRENCY);
        assert!(max <= MAX_CONCURRENCY);
    }

    #[test]
    fn test_concurrency_limiter_default() {
        let limiter = ConcurrencyLimiter::default();
        let max = limiter.current_max();
        assert!(max >= MIN_CONCURRENCY);
    }

    #[tokio::test]
    async fn test_concurrency_limiter_acquire() {
        let limiter = ConcurrencyLimiter::new();
        let initial_max = limiter.current_max();

        // Should be able to acquire a permit
        let permit = limiter.acquire().await;
        assert!(limiter.current_max() == initial_max);

        // Drop the permit
        drop(permit);
    }

    #[test]
    fn test_concurrency_limiter_report_success() {
        let limiter = ConcurrencyLimiter::new();
        let initial_max = limiter.current_max();

        // Report successes - should not change max until threshold
        for _ in 0..TEST_SUCCESS_ITERATIONS_BELOW_THRESHOLD {
            limiter.report_success();
        }
        assert_eq!(limiter.current_max(), initial_max);
    }

    #[test]
    fn test_concurrency_limiter_report_rate_limit() {
        let limiter = ConcurrencyLimiter::new();
        let initial_max = limiter.current_max();

        // Report rate limit - should decrease concurrency
        limiter.report_rate_limit();

        let new_max = limiter.current_max();
        // Should be halved or at minimum
        assert!(new_max <= initial_max);
        assert!(new_max >= MIN_CONCURRENCY);
    }

    #[test]
    fn test_concurrency_limiter_rate_limit_then_recovery() {
        let limiter = ConcurrencyLimiter::new();
        let initial_max = limiter.current_max();

        // First reduce via rate limit
        limiter.report_rate_limit();
        let reduced_max = limiter.current_max();

        // Then report many successes to trigger recovery
        for _ in 0..RECOVERY_THRESHOLD {
            limiter.report_success();
        }

        let recovered_max = limiter.current_max();
        // Should have increased (if we were below original)
        if reduced_max < initial_max {
            assert!(recovered_max > reduced_max || recovered_max == initial_max);
        }
    }

    #[test]
    fn test_concurrency_limiter_never_below_minimum() {
        let limiter = ConcurrencyLimiter::new();

        // Report many rate limits
        for _ in 0..TEST_RATE_LIMIT_ITERATIONS {
            limiter.report_rate_limit();
        }

        // Should never go below minimum
        assert!(limiter.current_max() >= MIN_CONCURRENCY);
    }

    #[test]
    fn test_concurrency_limiter_multiple_recovery_cycles() {
        let limiter = ConcurrencyLimiter::new();
        let initial_max = limiter.current_max();

        // First cycle: reduce then recover
        limiter.report_rate_limit();
        let after_first_limit = limiter.current_max();
        assert!(after_first_limit <= initial_max);

        for _ in 0..RECOVERY_THRESHOLD {
            limiter.report_success();
        }
        let after_first_recovery = limiter.current_max();

        // Second cycle: reduce again then recover
        limiter.report_rate_limit();
        let after_second_limit = limiter.current_max();
        assert!(after_second_limit <= after_first_recovery);

        for _ in 0..RECOVERY_THRESHOLD {
            limiter.report_success();
        }
        let after_second_recovery = limiter.current_max();

        // Should recover (or stay at max if already there)
        assert!(after_second_recovery >= after_second_limit);
    }

    #[test]
    fn test_concurrency_limiter_reduction_factor() {
        let limiter = ConcurrencyLimiter::new();
        let initial = limiter.current_max();

        // Skip test if already at minimum
        if initial <= MIN_CONCURRENCY {
            return;
        }

        limiter.report_rate_limit();
        let reduced = limiter.current_max();

        // Should be reduced by CONCURRENCY_REDUCTION_FACTOR
        let expected = (initial / CONCURRENCY_REDUCTION_FACTOR).max(MIN_CONCURRENCY);
        assert_eq!(reduced, expected);
    }

    #[test]
    fn test_concurrency_limiter_success_resets_counter() {
        let limiter = ConcurrencyLimiter::new();

        // Report some successes (less than threshold)
        for _ in 0..TEST_SUCCESS_ITERATIONS_BELOW_THRESHOLD {
            limiter.report_success();
        }

        // Rate limit resets the success counter
        limiter.report_rate_limit();

        // Need full RECOVERY_THRESHOLD successes again to recover
        for _ in 0..(RECOVERY_THRESHOLD - 1) {
            limiter.report_success();
        }

        // Should not have recovered yet (one short of threshold)
        let before_last = limiter.current_max();

        limiter.report_success();
        let after_threshold = limiter.current_max();

        // May or may not have increased depending on if we were below original
        assert!(after_threshold >= before_last);
    }

    #[tokio::test]
    async fn test_concurrency_limiter_multiple_permits() {
        let limiter = ConcurrencyLimiter::new();
        let max = limiter.current_max();

        // Should be able to acquire multiple permits up to max
        let mut permits = Vec::new();
        for _ in 0..max.min(3) {
            // Limit to 3 to keep test fast
            permits.push(limiter.acquire().await);
        }

        // All permits acquired
        assert_eq!(permits.len(), max.min(3));

        // Drop all permits
        drop(permits);
    }

    // =========================================================================
    // ValidatorRunner Helper Tests
    // =========================================================================

    #[test]
    fn test_is_rate_limit_error_rate_limit() {
        assert!(super::is_rate_limit_error("rate limit exceeded"));
        assert!(super::is_rate_limit_error("Rate Limit Hit"));
        assert!(super::is_rate_limit_error("rate_limit_error"));
    }

    #[test]
    fn test_is_rate_limit_error_429() {
        assert!(super::is_rate_limit_error("HTTP 429 Too Many Requests"));
        assert!(super::is_rate_limit_error("Error 429"));
    }

    #[test]
    fn test_is_rate_limit_error_timeout() {
        assert!(super::is_rate_limit_error("request timeout"));
        assert!(super::is_rate_limit_error("connection timed out"));
        assert!(super::is_rate_limit_error("Timeout waiting for response"));
    }

    #[test]
    fn test_is_rate_limit_error_capacity() {
        assert!(super::is_rate_limit_error("server overloaded"));
        assert!(super::is_rate_limit_error("at capacity"));
        assert!(super::is_rate_limit_error("too many requests"));
    }

    #[test]
    fn test_is_rate_limit_error_normal_errors() {
        // These should NOT be detected as rate limit errors
        assert!(!super::is_rate_limit_error("invalid json"));
        assert!(!super::is_rate_limit_error("validation failed"));
        assert!(!super::is_rate_limit_error("file not found"));
        assert!(!super::is_rate_limit_error("authentication error"));
    }

    // =========================================================================
    // ValidatorRunner Unit Tests with PlaybackAgent
    // =========================================================================

    use agent_client_protocol::schema::{
        AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification, ExtRequest,
        ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
        LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
        SetSessionModeRequest, SetSessionModeResponse,
    };
    use agent_client_protocol::{Channel, Client, ConnectTo};
    use agent_client_protocol_extras::PlaybackAgent;
    use futures::future::BoxFuture;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Trait that test mock agents implement so a single
    /// [`MockAgentAdapter`] can route incoming `ClientRequest` enum variants
    /// onto the right handler. ACP 0.11 removed the `agent_client_protocol::Agent`
    /// trait, so this is a project-local replacement scoped to the test module.
    ///
    /// Default implementations cover the common "no-op" case: `initialize`
    /// returns a stock `InitializeResponse`, `authenticate` returns a stock
    /// `AuthenticateResponse`, and the rest report
    /// [`agent_client_protocol::Error::method_not_found`]. Mocks override only
    /// the methods they actually exercise (`new_session` + `prompt` for the
    /// existing tests).
    ///
    /// The `BoxFuture` return shape matches the SDK's typed handler signature
    /// in `Agent.builder().on_receive_request(...)`, which lets the adapter
    /// forward without per-mock specialisation.
    trait MockAgent: Send + Sync {
        fn initialize<'a>(
            &'a self,
            _request: InitializeRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<InitializeResponse>> {
            Box::pin(async move { Ok(InitializeResponse::new(1.into())) })
        }

        fn authenticate<'a>(
            &'a self,
            _request: AuthenticateRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<AuthenticateResponse>> {
            Box::pin(async move { Ok(AuthenticateResponse::new()) })
        }

        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn load_session<'a>(
            &'a self,
            _request: LoadSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<LoadSessionResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn set_session_mode<'a>(
            &'a self,
            _request: SetSessionModeRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<SetSessionModeResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn prompt<'a>(
            &'a self,
            _request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn cancel<'a>(
            &'a self,
            _notification: CancelNotification,
        ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
            Box::pin(async move { Ok(()) })
        }

        fn ext_method<'a>(
            &'a self,
            _request: ExtRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<ExtResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn ext_notification<'a>(
            &'a self,
            _notification: ExtNotification,
        ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
            Box::pin(async move { Ok(()) })
        }
    }

    /// `ConnectTo<Client>` adapter that drives a [`MockAgent`] as an ACP
    /// 0.11 server.
    ///
    /// Spins up an `Agent.builder()` whose `on_receive_request` /
    /// `on_receive_notification` handlers demultiplex the incoming
    /// `ClientRequest` / `ClientNotification` enums onto the mock's per-method
    /// hooks. The builder runs in server-only mode (`connect_to`) so its main
    /// loop terminates exactly when the wired client transport closes — no
    /// shutdown signal needed.
    struct MockAgentAdapter<M: MockAgent + 'static>(Arc<M>);

    impl<M: MockAgent + 'static> ConnectTo<Client> for MockAgentAdapter<M> {
        async fn connect_to(
            self,
            client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
        ) -> agent_client_protocol::Result<()> {
            let mock = Arc::clone(&self.0);
            let mock_for_notifications = Arc::clone(&self.0);

            agent_client_protocol::Agent
                .builder()
                .name("mock-agent")
                .on_receive_request(
                    {
                        let mock = Arc::clone(&mock);
                        async move |req: agent_client_protocol::ClientRequest, responder, cx| {
                            dispatch_mock_request(&mock, req, responder, &cx)
                        }
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_notification(
                    async move |notif: agent_client_protocol::ClientNotification, _cx| {
                        dispatch_mock_notification(&mock_for_notifications, notif).await;
                        Ok(())
                    },
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_to(client)
                .await
        }
    }

    /// Demultiplex an incoming `ClientRequest` onto the mock's per-method
    /// handlers. Mirrors `dispatch_client_request` in the production agents
    /// (see `llama-agent/src/acp/server.rs`).
    ///
    /// Each per-method dispatch is offloaded to `cx.spawn` so the SDK's event
    /// loop can keep dispatching new incoming requests while a slow handler
    /// (e.g. `SlowAgent::prompt`) is awaiting. Without the spawn, two
    /// concurrent prompts on the same connection would serialise — which the
    /// `test_execute_ruleset_runs_rules_in_parallel` regression test
    /// explicitly forbids.
    fn dispatch_mock_request<M: MockAgent + 'static>(
        mock: &Arc<M>,
        request: agent_client_protocol::ClientRequest,
        responder: agent_client_protocol::Responder<serde_json::Value>,
        cx: &ConnectionTo<Client>,
    ) -> agent_client_protocol::Result<()> {
        use agent_client_protocol::ClientRequest as Req;

        let mock = Arc::clone(mock);
        cx.spawn(async move {
            match request {
                Req::InitializeRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.initialize(req).await),
                Req::AuthenticateRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.authenticate(req).await),
                Req::NewSessionRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.new_session(req).await),
                Req::LoadSessionRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.load_session(req).await),
                Req::SetSessionModeRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.set_session_mode(req).await),
                Req::PromptRequest(req) => responder.cast().respond_with_result(mock.prompt(req).await),
                Req::ExtMethodRequest(req) => {
                    let result = mock.ext_method(req).await.and_then(|ext_response| {
                        serde_json::from_str::<serde_json::Value>(ext_response.0.get())
                            .map_err(|_| agent_client_protocol::Error::internal_error())
                    });
                    responder.respond_with_result(result)
                }
                // ClientRequest is `#[non_exhaustive]` and may grow new
                // variants; surface anything we don't model as
                // method-not-found rather than silently ignoring it, matching
                // the dispatch in the production agents (see
                // `llama-agent/src/acp/server.rs`).
                _ => responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found()),
            }
        })
    }

    /// Demultiplex an incoming `ClientNotification` onto the mock. Errors are
    /// logged inside the per-variant handler and never propagated.
    async fn dispatch_mock_notification<M: MockAgent + ?Sized>(
        mock: &Arc<M>,
        notification: agent_client_protocol::ClientNotification,
    ) {
        use agent_client_protocol::ClientNotification as Notif;

        match notification {
            Notif::CancelNotification(n) => {
                let _ = mock.cancel(n).await;
            }
            Notif::ExtNotification(n) => {
                let _ = mock.ext_notification(n).await;
            }
            _ => {}
        }
    }

    /// Wire a [`MockAgent`] up to a fresh `Client` and run `body` against the
    /// resulting `ConnectionTo<Agent>` handle. Returns whatever `body` returns.
    ///
    /// This is the ACP 0.11 replacement for the 0.10 pattern of constructing
    /// an `Arc<dyn Agent>` directly. Tests pass an `Arc<M>` where `M:
    /// MockAgent`; the helper:
    ///
    /// 1. Builds a `Channel::duplex()` pair of in-process transports.
    /// 2. Spawns the mock as an Agent server on one end via
    ///    [`MockAgentAdapter`].
    /// 3. Runs `Client.builder().connect_with(...)` on the other end and
    ///    invokes `body` with the resulting [`ConnectionTo<Agent>`].
    /// 4. Forwards every incoming `SessionNotification` to the per-session
    ///    [`claude_agent::NotificationSender`] so the validator runner's
    ///    notification subscribers see streaming content.
    async fn run_with_mock_agent<M, F, Fut, R>(
        mock: Arc<M>,
        notifier: Arc<claude_agent::NotificationSender>,
        body: F,
    ) -> R
    where
        M: MockAgent + 'static,
        F: FnOnce(ConnectionTo<Agent>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        // In-process transport pair: channel_a goes to the agent, channel_b to
        // the client.
        let (channel_a, channel_b) = Channel::duplex();

        // Background task: drive the mock agent on channel_a. The task keeps
        // running until either side of the duplex channel closes; we abort it
        // on completion of the client side as a defensive measure.
        let agent_task = tokio::spawn(async move {
            let _ = MockAgentAdapter(mock).connect_to(channel_a).await;
        });

        let result = run_client_against(channel_b, notifier, "mock-test-client", body).await;

        // The client side has finished and dropped channel_b; the agent's
        // dispatch loop will observe the rx close and wind down. Abort
        // explicitly so we don't keep the task pending on its main_fn even if
        // the SDK's `connect_to(pending())` would wait on the foreground.
        agent_task.abort();
        let _ = agent_task.await;
        result
    }

    /// `PlaybackAgent` already implements `ConnectTo<Client>`; this is just a
    /// thin convenience that wires it up the same way as a [`MockAgent`].
    /// Returns whatever `body` returns.
    async fn run_with_playback_agent<F, Fut, R>(
        agent: PlaybackAgent,
        notifier: Arc<claude_agent::NotificationSender>,
        body: F,
    ) -> R
    where
        F: FnOnce(ConnectionTo<Agent>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (channel_a, channel_b) = Channel::duplex();

        let agent_task = tokio::spawn(async move {
            let _ = agent.connect_to(channel_a).await;
        });

        let result = run_client_against(channel_b, notifier, "playback-test-client", body).await;

        agent_task.abort();
        let _ = agent_task.await;
        result
    }

    /// Shared client-side wiring used by `run_with_mock_agent` and
    /// `run_with_playback_agent`. Stands up `Client.builder().connect_with(...)`
    /// against `transport`, forwards every incoming `SessionNotification` to
    /// `notifier`, and runs `body` inside the closure with the resulting
    /// `ConnectionTo<Agent>`.
    async fn run_client_against<F, Fut, R>(
        transport: Channel,
        notifier: Arc<claude_agent::NotificationSender>,
        name: &'static str,
        body: F,
    ) -> R
    where
        F: FnOnce(ConnectionTo<Agent>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let notifier_for_handler = Arc::clone(&notifier);
        Client
            .builder()
            .name(name)
            .on_receive_notification(
                async move |notif: SessionNotification, _cx| {
                    let _ = notifier_for_handler.send_update(notif).await;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(transport, async move |conn: ConnectionTo<Agent>| {
                Ok(body(conn).await)
            })
            .await
            .expect("client connect_with failed")
    }

    /// Create a test fixture directory with a playback file.
    fn create_playback_fixture(response_json: &str) -> (TempDir, PathBuf) {
        let temp = TempDir::new().unwrap();
        let fixture_path = temp.path().join("playback.json");
        std::fs::write(&fixture_path, response_json).unwrap();
        (temp, fixture_path)
    }

    /// Create a test validator for unit tests.
    fn create_test_validator() -> Validator {
        use crate::validator::{Severity, ValidatorFrontmatter, ValidatorSource};

        Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test-validator".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::PreToolUse,
                match_criteria: None,
                trigger_matcher: None,
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: "Check for issues.".to_string(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        }
    }

    /// Playback fixture for a passing validator response.
    const PLAYBACK_PASS: &str = r#"{
        "events": [
            {
                "type": "response",
                "content": "{\"status\": \"passed\", \"message\": \"All checks passed\"}"
            }
        ]
    }"#;

    /// Playback fixture for a failing validator response.
    const PLAYBACK_FAIL: &str = r#"{
        "events": [
            {
                "type": "response",
                "content": "{\"status\": \"failed\", \"message\": \"Found issues in code\"}"
            }
        ]
    }"#;

    #[tokio::test]
    async fn test_validator_runner_new() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_playback_agent(agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body);
            assert!(runner.is_ok(), "ValidatorRunner::new should succeed");

            let runner = runner.unwrap();
            assert!(
                runner.current_concurrency() >= MIN_CONCURRENCY,
                "Concurrency should be at least MIN_CONCURRENCY"
            );
            assert!(
                runner.current_concurrency() <= MAX_CONCURRENCY,
                "Concurrency should be at most MAX_CONCURRENCY"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn test_validator_runner_current_concurrency() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_playback_agent(agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body).unwrap();

            // current_concurrency should return a valid value
            let concurrency = runner.current_concurrency();
            assert!(concurrency >= MIN_CONCURRENCY);
            assert!(concurrency <= MAX_CONCURRENCY);
        })
        .await;
    }

    #[tokio::test]
    async fn test_validator_runner_execute_validator_pass() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_playback_agent(agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            let validator = create_test_validator();
            let context = serde_json::json!({"tool_name": "Write", "file_path": "test.ts"});

            let (result, is_rate_limited) = runner
                .execute_validator(&validator, HookType::PreToolUse, &context, None)
                .await;

            assert!(!is_rate_limited, "Should not be rate limited");
            assert_eq!(result.name, "test-validator");
        })
        .await;
    }

    #[tokio::test]
    async fn test_validator_runner_execute_validators_empty() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_playback_agent(agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            let context = serde_json::json!({"tool_name": "Write"});

            // Empty validators list should return empty results
            let validators: Vec<&Validator> = vec![];
            let results = runner
                .execute_validators(&validators, HookType::PreToolUse, &context, None)
                .await;

            assert!(
                results.is_empty(),
                "Empty input should produce empty output"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn test_validator_runner_execute_validator_with_changed_files() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_playback_agent(agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            let validator = create_test_validator();
            let context = serde_json::json!({"session_id": "test"});
            let changed_files = vec!["src/lib.rs".to_string(), "src/main.rs".to_string()];

            let (result, is_rate_limited) = runner
                .execute_validator(&validator, HookType::Stop, &context, Some(&changed_files))
                .await;

            assert!(!is_rate_limited);
            assert_eq!(result.name, "test-validator");
        })
        .await;
    }

    #[tokio::test]
    async fn test_validator_runner_execute_validator_fail() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_FAIL);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_playback_agent(agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            let validator = create_test_validator();
            let context = serde_json::json!({"tool_name": "Write", "file_path": "test.ts"});

            let (result, is_rate_limited) = runner
                .execute_validator(&validator, HookType::PreToolUse, &context, None)
                .await;

            assert!(!is_rate_limited, "Should not be rate limited");
            assert_eq!(result.name, "test-validator");
        })
        .await;
    }

    // =========================================================================
    // execute_ruleset per-rule fresh session tests
    // =========================================================================

    /// Test agent that records the `session_id` of every `prompt()` call.
    ///
    /// `new_session` returns a freshly minted `SessionId` derived from a
    /// monotonic counter so the runner can hand out distinct ids to each
    /// rule. `prompt` returns a hard-coded valid `passed` response.
    ///
    /// Used by [`test_execute_ruleset_uses_fresh_session_per_rule`] to verify
    /// that each rule in a RuleSet gets its own session and therefore cannot
    /// see prior rules' conversation history.
    ///
    /// ACP 0.11: this used to `impl Agent` (the trait) directly. The trait was
    /// removed in 0.11; the per-method bodies now live as inherent `async fn`s
    /// and are exposed to the SDK via [`MockAgent`] + [`MockAgentAdapter`]. The
    /// behaviour is unchanged.
    struct SessionRecordingAgent {
        next_session: std::sync::atomic::AtomicUsize,
        prompt_session_ids: std::sync::Mutex<Vec<String>>,
    }

    impl SessionRecordingAgent {
        fn new() -> Self {
            Self {
                next_session: std::sync::atomic::AtomicUsize::new(0),
                prompt_session_ids: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn recorded_session_ids(&self) -> Vec<String> {
            self.prompt_session_ids.lock().unwrap().clone()
        }

        /// Mint a fresh `test-session-N` id, recording the bump.
        async fn new_session(
            &self,
            _request: agent_client_protocol::schema::NewSessionRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::schema::NewSessionResponse>
        {
            let n = self
                .next_session
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let session_id =
                agent_client_protocol::schema::SessionId::new(format!("test-session-{}", n));
            Ok(agent_client_protocol::schema::NewSessionResponse::new(
                session_id,
            ))
        }

        /// Capture the prompt's session id and return a `EndTurn` response.
        async fn prompt(
            &self,
            request: agent_client_protocol::schema::PromptRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::schema::PromptResponse> {
            self.prompt_session_ids
                .lock()
                .unwrap()
                .push(request.session_id.0.to_string());
            Ok(agent_client_protocol::schema::PromptResponse::new(
                agent_client_protocol::schema::StopReason::EndTurn,
            ))
        }
    }

    impl MockAgent for SessionRecordingAgent {
        fn new_session<'a>(
            &'a self,
            request: agent_client_protocol::schema::NewSessionRequest,
        ) -> futures::future::BoxFuture<
            'a,
            agent_client_protocol::Result<agent_client_protocol::schema::NewSessionResponse>,
        > {
            Box::pin(async move { SessionRecordingAgent::new_session(self, request).await })
        }

        fn prompt<'a>(
            &'a self,
            request: agent_client_protocol::schema::PromptRequest,
        ) -> futures::future::BoxFuture<
            'a,
            agent_client_protocol::Result<agent_client_protocol::schema::PromptResponse>,
        > {
            Box::pin(async move { SessionRecordingAgent::prompt(self, request).await })
        }
    }

    /// Build a RuleSet with N test rules for the per-rule session tests.
    fn create_ruleset_with_n_rules(rule_names: &[&str]) -> RuleSet {
        use crate::validator::{Rule, RuleSetManifest, RuleSetMetadata, Severity, ValidatorSource};

        let rules = rule_names
            .iter()
            .map(|name| Rule {
                name: (*name).to_string(),
                description: format!("Test rule {}", name),
                body: format!("Validate {}", name),
                severity: None,
                timeout: None,
            })
            .collect();

        RuleSet {
            manifest: RuleSetManifest {
                name: "test-ruleset".to_string(),
                description: "Test RuleSet".to_string(),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                trigger: HookType::Stop,
                match_criteria: None,
                trigger_matcher: None,
                tags: vec![],
                severity: Severity::Error,
                timeout: 30,
                once: false,
            },
            rules,
            source: ValidatorSource::Project,
            base_path: PathBuf::from("/tmp/test-ruleset"),
        }
    }

    /// Regression test: each rule in a RuleSet must run in its own session.
    ///
    /// Prior behaviour: a single `new_session` was called before the rule
    /// loop and reused for every rule, causing rule N to see rule N-1's
    /// prompt and response in its conversation history (prompt bleed).
    ///
    /// Corrected behaviour (this task): `new_session` is called inside the
    /// loop, so each rule gets a distinct `session_id`. This test asserts
    /// that the agent observes a different `session_id` on every `prompt()`
    /// call.
    #[tokio::test]
    async fn test_execute_ruleset_uses_fresh_session_per_rule() {
        let recording_agent = Arc::new(SessionRecordingAgent::new());
        let recording_agent_for_assert = Arc::clone(&recording_agent);

        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(recording_agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            let ruleset = create_ruleset_with_n_rules(&["rule-a", "rule-b", "rule-c"]);
            let context = serde_json::json!({"tool_name": "Write"});

            let (executed, is_rate_limited) = runner
                .execute_ruleset(&ruleset, HookType::Stop, &context, None)
                .await;

            assert!(!is_rate_limited, "Should not be rate limited");
            assert_eq!(
                executed.rule_results.len(),
                3,
                "Each rule should produce a result"
            );

            let session_ids = recording_agent_for_assert.recorded_session_ids();
            assert_eq!(
                session_ids.len(),
                3,
                "agent.prompt() should be called once per rule (no init prompt)"
            );

            // The critical regression assertion: every rule must see a distinct
            // session_id. If any two rules share a session_id, prompt-bleed is
            // possible because the second rule would see the first rule's
            // conversation history.
            let unique: std::collections::HashSet<&String> = session_ids.iter().collect();
            assert_eq!(
                unique.len(),
                session_ids.len(),
                "Each rule must run in its own session_id, but observed: {:?}",
                session_ids
            );
        })
        .await;
    }

    // =========================================================================
    // RULE_GENERATION_MAX_TOKENS cap tests
    // =========================================================================

    /// `build_rule_prompt_request` must attach the per-rule generation cap to
    /// the request's `_meta` map under the key `"max_tokens"`.
    ///
    /// This is the contract the runner relies on: agents that honor the cap
    /// read it from `_meta` (the ACP `PromptRequest` schema does not have a
    /// first-class `max_tokens` field) and return `stop_reason: MaxTokens`
    /// when the generation hits it. If the field disappears or is renamed
    /// silently, runaway generations would no longer be capped.
    #[test]
    fn test_build_rule_prompt_request_sets_max_tokens_meta() {
        let session_id = agent_client_protocol::schema::SessionId::new("test-session");
        let request = build_rule_prompt_request(session_id.clone(), "rule body".to_string());

        // session_id is propagated unchanged
        assert_eq!(request.session_id.0.as_ref(), "test-session");

        // meta is populated and contains the cap under the documented key
        let meta = request
            .meta
            .expect("meta must be populated with max_tokens");
        let max_tokens = meta
            .get("max_tokens")
            .expect("meta must contain a 'max_tokens' entry");
        assert_eq!(
            max_tokens
                .as_u64()
                .expect("'max_tokens' must serialize as a u64"),
            RULE_GENERATION_MAX_TOKENS,
            "meta.max_tokens must equal the RULE_GENERATION_MAX_TOKENS constant"
        );
    }

    /// `truncate_partial_response_for_max_tokens` returns short responses
    /// untouched and truncates long ones at the configured byte budget with a
    /// `[truncated]` marker, respecting UTF-8 character boundaries.
    #[test]
    fn test_truncate_partial_response_short_unchanged() {
        let short = "this is short";
        assert_eq!(
            truncate_partial_response_for_max_tokens(short),
            short,
            "responses under the byte budget must be returned unchanged"
        );
    }

    #[test]
    fn test_truncate_partial_response_long_marked_truncated() {
        let long = "x".repeat(MAX_TOKENS_PARTIAL_RESPONSE_BYTES + 100);
        let truncated = truncate_partial_response_for_max_tokens(&long);
        assert!(
            truncated.ends_with(" [truncated]"),
            "long responses must be marked as [truncated], got: {:?}",
            &truncated[truncated.len().saturating_sub(20)..]
        );
        // The original payload prefix must be preserved (no character drift)
        assert!(truncated.starts_with(&"x".repeat(100)));
    }

    /// `build_rule_outcome_from_response` must convert a `MaxTokens`
    /// `PromptResponse` into a loud rule failure rather than parsing a
    /// truncated, half-finished response. The failure message must reference
    /// the cap and embed the partial response so users have a debug trail.
    #[test]
    fn test_build_rule_outcome_max_tokens_is_failure() {
        use crate::validator::{Rule, RuleSetManifest, RuleSetMetadata, Severity, ValidatorSource};

        let rule = Rule {
            name: "naming-consistency".to_string(),
            description: "Test rule".to_string(),
            body: "Validate naming.".to_string(),
            severity: None,
            timeout: None,
        };
        let ruleset = RuleSet {
            manifest: RuleSetManifest {
                name: "test-ruleset".to_string(),
                description: "Test RuleSet".to_string(),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                trigger: HookType::Stop,
                match_criteria: None,
                trigger_matcher: None,
                tags: vec![],
                severity: Severity::Error,
                timeout: 30,
                once: false,
            },
            rules: vec![],
            source: ValidatorSource::Project,
            base_path: PathBuf::from("/tmp/test-ruleset"),
        };
        let response = Ok(agent_client_protocol::schema::PromptResponse::new(
            agent_client_protocol::schema::StopReason::MaxTokens,
        ));
        let partial = "<think>I was thinking about validators</think> partial output...";

        let outcome =
            build_rule_outcome_from_response(&rule, &ruleset, response, partial.to_string());

        let result = match outcome {
            RuleOutcome::Failure(r) => r,
            other => panic!(
                "MaxTokens stop_reason must produce RuleOutcome::Failure, got {:?}",
                std::mem::discriminant(&other)
            ),
        };

        assert_eq!(result.rule_name, "naming-consistency");
        assert_eq!(
            result.severity,
            Severity::Error,
            "severity must follow the rule's effective severity (the ruleset's Error in this case)"
        );
        assert!(
            !result.passed(),
            "MaxTokens outcome must be a failed result, not passed"
        );

        let message = result.message();
        assert!(
            message.contains(&RULE_GENERATION_MAX_TOKENS.to_string()),
            "failure message must reference the cap value, got: {}",
            message
        );
        assert!(
            message.contains("naming-consistency"),
            "failure message must reference the rule name, got: {}",
            message
        );
        assert!(
            message.contains("partial output"),
            "failure message must include the partial response for debugging, got: {}",
            message
        );
    }

    /// Test agent that returns `stop_reason: MaxTokens` from `prompt()` so we
    /// can exercise the runner's `MaxTokens → loud failure` path through the
    /// real `execute_ruleset` entry point — not just the helper functions.
    ///
    /// This is the integration-style cousin of
    /// [`test_build_rule_outcome_max_tokens_is_failure`]: that test pokes the
    /// helper directly; this one drives the same path through the public API
    /// to catch wiring regressions (e.g. someone adding a new code path that
    /// bypasses `build_rule_outcome_from_response`).
    /// ACP 0.11: behaviour preserved from the previous `impl Agent` form.
    struct MaxTokensAgent {
        next_session: std::sync::atomic::AtomicUsize,
    }

    impl MaxTokensAgent {
        fn new() -> Self {
            Self {
                next_session: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        async fn new_session(
            &self,
            _request: agent_client_protocol::schema::NewSessionRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::schema::NewSessionResponse>
        {
            let n = self
                .next_session
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let session_id =
                agent_client_protocol::schema::SessionId::new(format!("max-tokens-sess-{}", n));
            Ok(agent_client_protocol::schema::NewSessionResponse::new(
                session_id,
            ))
        }

        async fn prompt(
            &self,
            _request: agent_client_protocol::schema::PromptRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::schema::PromptResponse> {
            // The runaway-generation case: agent stopped because it hit the
            // per-rule max_tokens cap before producing a verdict.
            Ok(agent_client_protocol::schema::PromptResponse::new(
                agent_client_protocol::schema::StopReason::MaxTokens,
            ))
        }
    }

    impl MockAgent for MaxTokensAgent {
        fn new_session<'a>(
            &'a self,
            request: agent_client_protocol::schema::NewSessionRequest,
        ) -> futures::future::BoxFuture<
            'a,
            agent_client_protocol::Result<agent_client_protocol::schema::NewSessionResponse>,
        > {
            Box::pin(async move { MaxTokensAgent::new_session(self, request).await })
        }

        fn prompt<'a>(
            &'a self,
            request: agent_client_protocol::schema::PromptRequest,
        ) -> futures::future::BoxFuture<
            'a,
            agent_client_protocol::Result<agent_client_protocol::schema::PromptResponse>,
        > {
            Box::pin(async move { MaxTokensAgent::prompt(self, request).await })
        }
    }

    /// End-to-end test: when the agent returns `stop_reason: MaxTokens` for a
    /// rule, `execute_ruleset` must surface it as a non-rate-limited failure
    /// whose message references the cap and the rule name.
    ///
    /// This guards against a regression where a future refactor of
    /// `execute_rule_in_fresh_session` or `send_rule_prompt_and_collect`
    /// silently bypasses the `MaxTokens → failure` mapping.
    #[tokio::test]
    async fn test_execute_ruleset_max_tokens_fails_loudly() {
        let agent = Arc::new(MaxTokensAgent::new());
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            let ruleset = create_ruleset_with_n_rules(&["naming-consistency"]);
            let context = serde_json::json!({"tool_name": "Write"});

            let (executed, is_rate_limited) = runner
                .execute_ruleset(&ruleset, HookType::Stop, &context, None)
                .await;

            // MaxTokens is a rule-level failure, not a transport-level rate limit
            assert!(
                !is_rate_limited,
                "MaxTokens must not be reported as a rate limit (it's a runaway generation, not throttling)"
            );
            assert_eq!(
                executed.rule_results.len(),
                1,
                "rule must produce exactly one result"
            );

            let result = &executed.rule_results[0];
            assert_eq!(result.rule_name, "naming-consistency");
            assert!(
                !result.passed(),
                "MaxTokens must produce a failed verdict, not a silent pass"
            );
            let message = result.message();
            assert!(
                message.contains(&RULE_GENERATION_MAX_TOKENS.to_string()),
                "failure message must reference the cap value ({}), got: {}",
                RULE_GENERATION_MAX_TOKENS,
                message
            );
            assert!(
                message.contains("naming-consistency"),
                "failure message must reference the rule name, got: {}",
                message
            );
        })
        .await;
    }

    // =========================================================================
    // filter_changed_files_for_ruleset tests
    // =========================================================================

    /// Helper to create a RuleSet with given file patterns in match criteria.
    fn create_ruleset_with_file_patterns(patterns: Vec<String>) -> RuleSet {
        use crate::validator::{
            RuleSetManifest, RuleSetMetadata, Severity, ValidatorMatch, ValidatorSource,
        };

        let match_criteria = if patterns.is_empty() {
            None
        } else {
            Some(ValidatorMatch {
                tools: vec![],
                files: patterns,
            })
        };

        RuleSet {
            manifest: RuleSetManifest {
                name: "test-ruleset".to_string(),
                description: "Test RuleSet".to_string(),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                trigger: HookType::Stop,
                match_criteria,
                trigger_matcher: None,
                tags: vec![],
                severity: Severity::Error,
                timeout: 30,
                once: false,
            },
            rules: vec![],
            source: ValidatorSource::Project,
            base_path: PathBuf::from("/tmp/test-ruleset"),
        }
    }

    #[test]
    fn test_filter_changed_files_with_rs_pattern() {
        let ruleset = create_ruleset_with_file_patterns(vec!["*.rs".to_string()]);
        let files = vec![
            "a.rs".to_string(),
            "b.py".to_string(),
            "src/lib.rs".to_string(),
        ];

        let result = filter_changed_files_for_ruleset(Some(&files), &ruleset);
        // glob "*.rs" matches any file ending in .rs (case-insensitive, no literal separator requirement)
        assert_eq!(
            result,
            Some(vec!["a.rs".to_string(), "src/lib.rs".to_string()])
        );
    }

    #[test]
    fn test_filter_changed_files_with_recursive_pattern() {
        let ruleset = create_ruleset_with_file_patterns(vec!["**/*.rs".to_string()]);
        let files = vec![
            "a.rs".to_string(),
            "b.py".to_string(),
            "src/lib.rs".to_string(),
        ];

        let result = filter_changed_files_for_ruleset(Some(&files), &ruleset);
        assert_eq!(
            result,
            Some(vec!["a.rs".to_string(), "src/lib.rs".to_string()])
        );
    }

    #[test]
    fn test_filter_changed_files_empty_patterns_returns_all() {
        let ruleset = create_ruleset_with_file_patterns(vec![]);
        let files = vec!["a.rs".to_string(), "b.py".to_string()];

        let result = filter_changed_files_for_ruleset(Some(&files), &ruleset);
        assert_eq!(result, Some(vec!["a.rs".to_string(), "b.py".to_string()]));
    }

    #[test]
    fn test_filter_changed_files_no_match_criteria_returns_all() {
        // RuleSet with no match_criteria at all
        let ruleset = create_ruleset_with_file_patterns(vec![]);
        // Ensure match_criteria is None
        let mut ruleset = ruleset;
        ruleset.manifest.match_criteria = None;

        let files = vec!["a.rs".to_string(), "b.py".to_string()];
        let result = filter_changed_files_for_ruleset(Some(&files), &ruleset);
        assert_eq!(result, Some(vec!["a.rs".to_string(), "b.py".to_string()]));
    }

    #[test]
    fn test_filter_changed_files_none_input_returns_none() {
        let ruleset = create_ruleset_with_file_patterns(vec!["*.rs".to_string()]);
        let result = filter_changed_files_for_ruleset(None, &ruleset);
        assert_eq!(result, None);
    }

    #[test]
    fn test_filter_changed_files_no_matches_returns_empty() {
        let ruleset = create_ruleset_with_file_patterns(vec!["*.rs".to_string()]);
        let files = vec!["a.py".to_string(), "b.ts".to_string()];

        let result = filter_changed_files_for_ruleset(Some(&files), &ruleset);
        assert_eq!(result, Some(vec![]));
    }

    #[test]
    fn test_filter_changed_files_multiple_patterns() {
        let ruleset =
            create_ruleset_with_file_patterns(vec!["*.rs".to_string(), "*.toml".to_string()]);
        let files = vec![
            "main.rs".to_string(),
            "Cargo.toml".to_string(),
            "README.md".to_string(),
        ];

        let result = filter_changed_files_for_ruleset(Some(&files), &ruleset);
        assert_eq!(
            result,
            Some(vec!["main.rs".to_string(), "Cargo.toml".to_string()])
        );
    }

    // =========================================================================
    // filter_diffs_for_ruleset tests
    // =========================================================================

    /// Helper to create a FileDiff with the given path and diff text.
    fn make_file_diff(path: &str, diff_text: &str) -> crate::turn::FileDiff {
        crate::turn::FileDiff {
            path: PathBuf::from(path),
            diff_text: diff_text.to_string(),
            is_new_file: false,
            is_binary: false,
        }
    }

    #[test]
    fn test_filter_diffs_with_rs_pattern() {
        let ruleset = create_ruleset_with_file_patterns(vec!["*.rs".to_string()]);
        let diffs = vec![
            make_file_diff("a.rs", "--- a.rs\n+++ a.rs\n@@ -1 +1 @@\n-old\n+new\n"),
            make_file_diff("b.py", "--- b.py\n+++ b.py\n@@ -1 +1 @@\n-old\n+new\n"),
            make_file_diff("c.rs", "--- c.rs\n+++ c.rs\n@@ -1 +1 @@\n-old\n+new\n"),
        ];

        let result = filter_diffs_for_ruleset(Some(&diffs), &ruleset);
        let paths: Vec<String> = result
            .unwrap()
            .iter()
            .map(|d| d.path.display().to_string())
            .collect();
        assert_eq!(paths, vec!["a.rs", "c.rs"]);
    }

    #[test]
    fn test_filter_diffs_no_patterns_returns_all() {
        let ruleset = create_ruleset_with_file_patterns(vec![]);
        let diffs = vec![
            make_file_diff("a.rs", "diff a"),
            make_file_diff("b.py", "diff b"),
        ];

        let result = filter_diffs_for_ruleset(Some(&diffs), &ruleset);
        assert_eq!(result.as_ref().map(|v| v.len()), Some(2));
    }

    #[test]
    fn test_filter_diffs_none_input_returns_none() {
        let ruleset = create_ruleset_with_file_patterns(vec!["*.rs".to_string()]);
        let result = filter_diffs_for_ruleset(None, &ruleset);
        assert_eq!(result, None);
    }

    #[test]
    fn test_filter_diffs_no_matches_returns_empty() {
        let ruleset = create_ruleset_with_file_patterns(vec!["*.rs".to_string()]);
        let diffs = vec![
            make_file_diff("a.py", "diff a"),
            make_file_diff("b.ts", "diff b"),
        ];

        let result = filter_diffs_for_ruleset(Some(&diffs), &ruleset);
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_filter_diffs_multiple_patterns() {
        let ruleset =
            create_ruleset_with_file_patterns(vec!["*.rs".to_string(), "*.toml".to_string()]);
        let diffs = vec![
            make_file_diff("main.rs", "diff rs"),
            make_file_diff("Cargo.toml", "diff toml"),
            make_file_diff("README.md", "diff md"),
        ];

        let result = filter_diffs_for_ruleset(Some(&diffs), &ruleset);
        let paths: Vec<String> = result
            .unwrap()
            .iter()
            .map(|d| d.path.display().to_string())
            .collect();
        assert_eq!(paths, vec!["main.rs", "Cargo.toml"]);
    }

    // =========================================================================
    // Per-rule timeout + in-ruleset parallelism tests
    // =========================================================================

    /// Test agent that sleeps inside `prompt()` for a configurable duration
    /// before returning a passing response. Used to drive the per-rule
    /// wall-clock timeout path.
    /// ACP 0.11: behaviour preserved from the previous `impl Agent` form.
    struct SlowAgent {
        next_session: std::sync::atomic::AtomicUsize,
        sleep_ms: u64,
    }

    impl SlowAgent {
        fn new(sleep_ms: u64) -> Self {
            Self {
                next_session: std::sync::atomic::AtomicUsize::new(0),
                sleep_ms,
            }
        }

        async fn new_session(
            &self,
            _request: agent_client_protocol::schema::NewSessionRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::schema::NewSessionResponse>
        {
            let n = self
                .next_session
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let session_id =
                agent_client_protocol::schema::SessionId::new(format!("slow-sess-{}", n));
            Ok(agent_client_protocol::schema::NewSessionResponse::new(
                session_id,
            ))
        }

        async fn prompt(
            &self,
            _request: agent_client_protocol::schema::PromptRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::schema::PromptResponse> {
            tokio::time::sleep(std::time::Duration::from_millis(self.sleep_ms)).await;
            Ok(agent_client_protocol::schema::PromptResponse::new(
                agent_client_protocol::schema::StopReason::EndTurn,
            ))
        }
    }

    impl MockAgent for SlowAgent {
        fn new_session<'a>(
            &'a self,
            request: agent_client_protocol::schema::NewSessionRequest,
        ) -> futures::future::BoxFuture<
            'a,
            agent_client_protocol::Result<agent_client_protocol::schema::NewSessionResponse>,
        > {
            Box::pin(async move { SlowAgent::new_session(self, request).await })
        }

        fn prompt<'a>(
            &'a self,
            request: agent_client_protocol::schema::PromptRequest,
        ) -> futures::future::BoxFuture<
            'a,
            agent_client_protocol::Result<agent_client_protocol::schema::PromptResponse>,
        > {
            Box::pin(async move { SlowAgent::prompt(self, request).await })
        }
    }

    /// Helper: build a RuleSet whose default timeout is `timeout_secs` and
    /// which contains `n` rules. Used by the timeout / parallelism tests.
    fn create_ruleset_with_timeout(rule_names: &[&str], timeout_secs: u32) -> RuleSet {
        use crate::validator::{Rule, RuleSetManifest, RuleSetMetadata, Severity, ValidatorSource};

        let rules = rule_names
            .iter()
            .map(|name| Rule {
                name: (*name).to_string(),
                description: format!("Test rule {}", name),
                body: format!("Validate {}", name),
                severity: None,
                timeout: None,
            })
            .collect();

        RuleSet {
            manifest: RuleSetManifest {
                name: "timeout-ruleset".to_string(),
                description: "Test RuleSet".to_string(),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                trigger: HookType::Stop,
                match_criteria: None,
                trigger_matcher: None,
                tags: vec![],
                severity: Severity::Error,
                timeout: timeout_secs,
                once: false,
            },
            rules,
            source: ValidatorSource::Project,
            base_path: PathBuf::from("/tmp/timeout-ruleset"),
        }
    }

    /// Each rule is wrapped in its own wall-clock timeout, and a rule that
    /// does not return within the budget must surface as a passing-with-warning
    /// [`RuleResult`] (not a hard failure that would block the hook). The
    /// caller logs that result with `reason="timeout"`.
    #[tokio::test]
    async fn test_execute_ruleset_rule_timeout_passes_with_warning() {
        // Agent sleeps for 5s; rule timeout is 1s. The timeout path must fire.
        let agent = Arc::new(SlowAgent::new(5_000));
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            let ruleset = create_ruleset_with_timeout(&["slow-rule"], 1);
            let context = serde_json::json!({"tool_name": "Write"});

            let start = std::time::Instant::now();
            let (executed, is_rate_limited) = runner
                .execute_ruleset(&ruleset, HookType::Stop, &context, None)
                .await;
            let elapsed = start.elapsed();

            // The timeout must actually have fired — the call must return well
            // before the agent's 5s sleep would have finished.
            assert!(
                elapsed.as_secs() < 4,
                "execute_ruleset must return when the rule times out, not block on the agent (elapsed: {:?})",
                elapsed,
            );

            assert!(!is_rate_limited, "wall-clock timeout is not a rate limit");
            assert_eq!(executed.rule_results.len(), 1);

            let result = &executed.rule_results[0];
            assert_eq!(result.rule_name, "slow-rule");
            assert!(
                result.passed(),
                "timed-out rule must produce a passing-with-warning result so the hook is not blocked"
            );
            let message = result.message();
            assert!(
                message.contains("timeout") || message.contains("did not complete"),
                "timeout message should mention the timeout, got: {}",
                message,
            );
        })
        .await;
    }

    /// Multiple rules whose individual prompt sleeps would, in series, take
    /// longer than the wall budget must still be evaluated in parallel. With
    /// 3 rules sleeping 200ms each and an in-flight cap of at least 2, the
    /// total wall time should be well under the 600ms serial sum.
    #[tokio::test]
    async fn test_execute_ruleset_runs_rules_in_parallel() {
        let agent = Arc::new(SlowAgent::new(200));
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            // Force a non-trivial in-flight cap so the test does not depend on the
            // host's CPU count: 3 in-flight slots is enough to run all 3 rules
            // concurrently.
            let mut runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            runner.rule_concurrency = Arc::new(Semaphore::new(3));

            let ruleset = create_ruleset_with_timeout(&["a", "b", "c"], 30);
            let context = serde_json::json!({"tool_name": "Write"});

            let start = std::time::Instant::now();
            let (executed, _is_rate_limited) = runner
                .execute_ruleset(&ruleset, HookType::Stop, &context, None)
                .await;
            let elapsed = start.elapsed();

            assert_eq!(executed.rule_results.len(), 3);
            // 3 sequential rules at 200ms each would take ~600ms. With parallel
            // execution we expect well under 600ms — give plenty of headroom for
            // CI noise but still fail loudly if the loop went serial.
            assert!(
                elapsed.as_millis() < 500,
                "rules must run in parallel (3x200ms in series = ~600ms), got {:?}",
                elapsed,
            );
        })
        .await;
    }

    /// The in-flight cap must serialize work when there are more rules than
    /// slots. 4 rules sleeping 200ms each with a cap of 1 should take roughly
    /// 4×200ms = 800ms — proving the semaphore actually throttles, rather
    /// than letting all four run at once.
    #[tokio::test]
    async fn test_execute_ruleset_in_flight_cap_throttles() {
        let agent = Arc::new(SlowAgent::new(200));
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        let notifier = Arc::new(notifier);
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let mut runner = ValidatorRunner::new(conn, notifier_body).unwrap();
            runner.rule_concurrency = Arc::new(Semaphore::new(1));

            let ruleset = create_ruleset_with_timeout(&["a", "b", "c", "d"], 30);
            let context = serde_json::json!({"tool_name": "Write"});

            let start = std::time::Instant::now();
            let (executed, _) = runner
                .execute_ruleset(&ruleset, HookType::Stop, &context, None)
                .await;
            let elapsed = start.elapsed();

            assert_eq!(executed.rule_results.len(), 4);
            // With cap=1 we expect serial execution: ~4×200ms = 800ms minimum.
            assert!(
                elapsed.as_millis() >= 700,
                "in-flight cap=1 must serialize work; expected >=700ms but got {:?}",
                elapsed,
            );
        })
        .await;
    }

    /// `resolve_rule_in_flight_cap` must default to [`RULE_DEFAULT_PARALLELISM`]
    /// when the env var is unset.
    #[test]
    #[serial_test::serial(rule_parallelism_env)]
    fn test_resolve_rule_in_flight_cap_default() {
        // Save and clear the env var
        let saved = std::env::var(RULE_PARALLELISM_ENV_VAR).ok();
        std::env::remove_var(RULE_PARALLELISM_ENV_VAR);

        let cap = resolve_rule_in_flight_cap();
        assert_eq!(
            cap, RULE_DEFAULT_PARALLELISM,
            "default cap must be RULE_DEFAULT_PARALLELISM"
        );

        // Restore
        if let Some(v) = saved {
            std::env::set_var(RULE_PARALLELISM_ENV_VAR, v);
        }
    }

    /// `resolve_rule_in_flight_cap` must honor the env var override when set
    /// to a positive integer, and fall back to the default for invalid or
    /// non-positive values.
    #[test]
    #[serial_test::serial(rule_parallelism_env)]
    fn test_resolve_rule_in_flight_cap_env_override() {
        let saved = std::env::var(RULE_PARALLELISM_ENV_VAR).ok();

        // Valid positive integer
        std::env::set_var(RULE_PARALLELISM_ENV_VAR, "7");
        assert_eq!(resolve_rule_in_flight_cap(), 7);

        // Invalid value falls back to default
        std::env::set_var(RULE_PARALLELISM_ENV_VAR, "not-a-number");
        assert_eq!(resolve_rule_in_flight_cap(), RULE_DEFAULT_PARALLELISM);

        // Zero is rejected (must be >=1) and falls back
        std::env::set_var(RULE_PARALLELISM_ENV_VAR, "0");
        assert_eq!(resolve_rule_in_flight_cap(), RULE_DEFAULT_PARALLELISM);

        // Cleanup
        std::env::remove_var(RULE_PARALLELISM_ENV_VAR);
        if let Some(v) = saved {
            std::env::set_var(RULE_PARALLELISM_ENV_VAR, v);
        }
    }
}
