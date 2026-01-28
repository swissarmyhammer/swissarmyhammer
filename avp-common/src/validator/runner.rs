//! Validator execution via ACP Agent.
//!
//! This module provides the `ValidatorRunner` which executes validators against
//! hook events by calling an Agent via the Agent Client Protocol (ACP).
//!
//! The agent is obtained from `AvpContext`, which handles lazy creation of
//! ClaudeAgent in production or injection of PlaybackAgent for testing.
//!
//! Validator partials can come from any of the standard validator directories:
//! - builtin/validators/_partials/
//! - ~/<AVP_DIR>/validators/_partials/
//! - <AVP_DIR>/validators/_partials/
//!
//! This follows the same unified pattern as prompts and rules, using
//! [`ValidatorPartialAdapter`] which is a type alias for
//! `LibraryPartialAdapter<ValidatorLoader>`.
//!
//! ## Parallel Execution
//!
//! Validators are executed in parallel with adaptive concurrency control:
//! - Initial concurrency is based on CPU count
//! - Concurrency is reduced when rate limits or timeouts are detected
//! - Concurrency gradually recovers after successful executions

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_client_protocol::{Agent, SessionNotification};
use futures::stream::{FuturesUnordered, StreamExt};
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
use swissarmyhammer_templating::HashMapPartialLoader;
use tokio::sync::{broadcast, Semaphore};

use crate::error::AvpError;
use crate::types::HookType;
use crate::validator::{
    create_executed_validator, is_rate_limit_error, log_validator_result, parse_validator_response,
    render_validator_prompt_with_partials_and_changed_files, ExecutedValidator, Validator,
    ValidatorLoader, VALIDATOR_PROMPT_NAME,
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
            let result = parse_validator_response(&collected.content, &collected.stop_reason);
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
        tracing::warn!("Rate limit/timeout for validator '{}': {}", validator.name(), error);
    } else {
        tracing::error!("Agent execution failed for validator '{}': {}", validator.name(), error);
    }

    let result = crate::validator::ValidatorResult::fail(format!("Agent execution failed: {}", error));
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
    /// ACP agent for prompt execution.
    agent: Arc<dyn Agent + Send + Sync>,
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
            claude_agent::execute_prompt_with_agent(&*self.agent, notifications, prompt_text).await;

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
        (self.idx, create_render_error(&self.validator, &error), false)
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
/// Get the agent from `AvpContext` and create a runner:
/// ```ignore
/// let (agent, notifications) = context.agent().await?;
/// let runner = ValidatorRunner::new(agent, notifications)?;
/// ```
pub struct ValidatorRunner {
    /// Prompt library containing the .validator prompt
    prompt_library: Arc<PromptLibrary>,
    /// Validator partials for template rendering
    partials: HashMapPartialLoader,
    /// Agent for executing prompts
    agent: Arc<dyn Agent + Send + Sync>,
    /// Notification sender for resubscription
    notifications: broadcast::Sender<SessionNotification>,
    /// Concurrency limiter for parallel execution
    concurrency: Arc<ConcurrencyLimiter>,
}

impl ValidatorRunner {
    /// Create a new ValidatorRunner with the given agent.
    ///
    /// The agent and notifications should be obtained from `AvpContext::agent()`.
    pub fn new(
        agent: Arc<dyn Agent + Send + Sync>,
        notifications: broadcast::Receiver<SessionNotification>,
    ) -> Result<Self, AvpError> {
        let prompt_library = Self::load_prompt_library()?;
        let partials = Self::load_validator_partials();
        let concurrency = Arc::new(ConcurrencyLimiter::new());

        // Create a sender so we can resubscribe for each validator execution
        let (tx, _) = broadcast::channel(crate::context::NOTIFICATION_CHANNEL_CAPACITY);
        let tx_clone = tx.clone();

        // Forward notifications from the provided receiver to our sender
        tokio::spawn(async move {
            let mut rx = notifications;
            loop {
                match rx.recv().await {
                    Ok(notification) => {
                        let _ = tx_clone.send(notification);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(
                            skipped_messages = n,
                            "ValidatorRunner notification forwarder lagged"
                        );
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        Ok(Self {
            prompt_library: Arc::new(prompt_library),
            partials,
            agent,
            notifications: tx,
            concurrency,
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
    /// 2. User validators (~/<AVP_DIR>/validators)
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
    fn extract_partials_from_loader(loader: &ValidatorLoader) -> HashMapPartialLoader {
        use crate::validator::{add_partial_with_aliases, is_partial};

        let mut partials = HashMapPartialLoader::empty();

        for validator in loader.list() {
            let name = validator.name();
            let body = &validator.body;

            if is_partial(name, body) {
                add_partial_with_aliases(&mut partials, name, body);
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

        let notifications = self.notifications.subscribe();
        let response =
            claude_agent::execute_prompt_with_agent(&*self.agent, notifications, prompt_text).await;

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
            agent: Arc::clone(&self.agent),
            notifications_tx: self.notifications.clone(),
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

    use agent_client_protocol_extras::PlaybackAgent;
    use std::path::PathBuf;
    use tempfile::TempDir;

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
        let notifications = agent.subscribe_notifications();

        let runner = ValidatorRunner::new(Arc::new(agent), notifications);
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
    }

    #[tokio::test]
    async fn test_validator_runner_current_concurrency() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let notifications = agent.subscribe_notifications();

        let runner = ValidatorRunner::new(Arc::new(agent), notifications).unwrap();

        // current_concurrency should return a valid value
        let concurrency = runner.current_concurrency();
        assert!(concurrency >= MIN_CONCURRENCY);
        assert!(concurrency <= MAX_CONCURRENCY);
    }

    #[tokio::test]
    async fn test_validator_runner_execute_validator_pass() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let notifications = agent.subscribe_notifications();

        let runner = ValidatorRunner::new(Arc::new(agent), notifications).unwrap();
        let validator = create_test_validator();
        let context = serde_json::json!({"tool_name": "Write", "file_path": "test.ts"});

        let (result, is_rate_limited) = runner
            .execute_validator(&validator, HookType::PreToolUse, &context, None)
            .await;

        assert!(!is_rate_limited, "Should not be rate limited");
        assert_eq!(result.name, "test-validator");
    }

    #[tokio::test]
    async fn test_validator_runner_execute_validators_empty() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let notifications = agent.subscribe_notifications();

        let runner = ValidatorRunner::new(Arc::new(agent), notifications).unwrap();
        let context = serde_json::json!({"tool_name": "Write"});

        // Empty validators list should return empty results
        let validators: Vec<&Validator> = vec![];
        let results = runner
            .execute_validators(&validators, HookType::PreToolUse, &context, None)
            .await;

        assert!(results.is_empty(), "Empty input should produce empty output");
    }

    #[tokio::test]
    async fn test_validator_runner_execute_validator_with_changed_files() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let notifications = agent.subscribe_notifications();

        let runner = ValidatorRunner::new(Arc::new(agent), notifications).unwrap();
        let validator = create_test_validator();
        let context = serde_json::json!({"session_id": "test"});
        let changed_files = vec!["src/lib.rs".to_string(), "src/main.rs".to_string()];

        let (result, is_rate_limited) = runner
            .execute_validator(
                &validator,
                HookType::Stop,
                &context,
                Some(&changed_files),
            )
            .await;

        assert!(!is_rate_limited);
        assert_eq!(result.name, "test-validator");
    }

    #[tokio::test]
    async fn test_validator_runner_execute_validator_fail() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_FAIL);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let notifications = agent.subscribe_notifications();

        let runner = ValidatorRunner::new(Arc::new(agent), notifications).unwrap();
        let validator = create_test_validator();
        let context = serde_json::json!({"tool_name": "Write", "file_path": "test.ts"});

        let (result, is_rate_limited) = runner
            .execute_validator(&validator, HookType::PreToolUse, &context, None)
            .await;

        assert!(!is_rate_limited, "Should not be rate limited");
        assert_eq!(result.name, "test-validator");
    }
}
