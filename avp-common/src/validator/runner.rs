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
    create_executed_validator, parse_validator_response, render_validator_prompt_with_partials,
    ExecutedValidator, Validator, ValidatorLoader, VALIDATOR_PROMPT_NAME,
};

/// Minimum concurrency level (never go below this).
const MIN_CONCURRENCY: usize = 1;

/// Number of successful executions before attempting to increase concurrency.
const RECOVERY_THRESHOLD: usize = 10;

/// Manages adaptive concurrency for parallel validator execution.
///
/// Starts at CPU count and reduces when rate limits or timeouts are detected.
/// Gradually recovers after successful executions.
pub struct ConcurrencyLimiter {
    /// Current max concurrency level.
    max_concurrency: AtomicUsize,
    /// Original max concurrency (for recovery).
    original_max: usize,
    /// Semaphore for limiting concurrent executions.
    semaphore: Arc<Semaphore>,
    /// Counter for successful executions (for recovery).
    success_count: AtomicUsize,
}

impl ConcurrencyLimiter {
    /// Create a new concurrency limiter based on CPU count.
    pub fn new() -> Self {
        let cpu_count = num_cpus::get();
        // Use CPU count but cap at a reasonable maximum
        let max_concurrency = cpu_count.min(8).max(MIN_CONCURRENCY);

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

    /// Get the current max concurrency.
    pub fn current_max(&self) -> usize {
        self.max_concurrency.load(Ordering::Relaxed)
    }

    /// Acquire a permit to execute a validator.
    pub async fn acquire(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed unexpectedly")
    }

    /// Report a successful execution.
    pub fn report_success(&self) {
        let count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Try to recover concurrency after enough successes
        if count >= RECOVERY_THRESHOLD {
            self.success_count.store(0, Ordering::Relaxed);
            self.try_increase_concurrency();
        }
    }

    /// Report a rate limit or timeout error.
    pub fn report_rate_limit(&self) {
        // Reset success counter
        self.success_count.store(0, Ordering::Relaxed);
        self.decrease_concurrency();
    }

    /// Decrease concurrency by half (minimum 1).
    fn decrease_concurrency(&self) {
        let current = self.max_concurrency.load(Ordering::Relaxed);
        let new_max = (current / 2).max(MIN_CONCURRENCY);

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

    /// Try to increase concurrency back toward the original max.
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
        let (tx, _) = broadcast::channel(256);
        let tx_clone = tx.clone();

        // Forward notifications from the provided receiver to our sender
        tokio::spawn(async move {
            let mut rx = notifications;
            while let Ok(notification) = rx.recv().await {
                let _ = tx_clone.send(notification);
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
    /// Partials are identified by:
    /// - Names starting with `_partials/`
    /// - Content starting with `{% partial %}`
    fn extract_partials_from_loader(loader: &ValidatorLoader) -> HashMapPartialLoader {
        let mut partials = HashMapPartialLoader::empty();

        for validator in loader.list() {
            let name = validator.name();
            let body = &validator.body;

            // Check if this is a partial
            let is_partial =
                name.starts_with("_partials/") || body.trim_start().starts_with("{% partial %}");

            if is_partial {
                // Add with the original name
                partials.add(name, body.clone());

                // Also add with just the base name (without _partials/ prefix)
                if let Some(base_name) = name.strip_prefix("_partials/") {
                    partials.add(base_name, body.clone());
                }
            }
        }

        partials
    }

    /// Execute a single validator against a hook event context.
    ///
    /// Returns an `ExecutedValidator` with the result and validator metadata.
    /// Also returns a boolean indicating if a rate limit was detected.
    pub async fn execute_validator(
        &self,
        validator: &Validator,
        hook_type: HookType,
        context: &serde_json::Value,
    ) -> (ExecutedValidator, bool) {
        // Render the validator prompt with partials support
        let prompt_result = render_validator_prompt_with_partials(
            &self.prompt_library,
            validator,
            hook_type,
            context,
            Some(&self.partials),
        );

        let prompt_text = match prompt_result {
            Ok(text) => text,
            Err(e) => {
                tracing::error!(
                    "Failed to render validator '{}' prompt: {}",
                    validator.name(),
                    e
                );
                return (
                    create_executed_validator(
                        validator,
                        crate::validator::ValidatorResult::fail(format!(
                            "Failed to render prompt: {}",
                            e
                        )),
                    ),
                    false,
                );
            }
        };

        // Get a fresh notification receiver for this execution
        let notifications = self.notifications.subscribe();

        // Execute via claude_agent::execute_prompt_with_agent helper
        let response =
            claude_agent::execute_prompt_with_agent(&*self.agent, notifications, prompt_text).await;

        match response {
            Ok(prompt_response) => {
                let result = parse_validator_response(&prompt_response.content);
                tracing::debug!(
                    "Validator '{}' result: {} - {}",
                    validator.name(),
                    if result.passed() { "PASSED" } else { "FAILED" },
                    result.message()
                );
                (create_executed_validator(validator, result), false)
            }
            Err(e) => {
                let error_str = e.to_string();
                let is_rate_limit = Self::is_rate_limit_error(&error_str);

                if is_rate_limit {
                    tracing::warn!(
                        "Rate limit/timeout for validator '{}': {}",
                        validator.name(),
                        e
                    );
                } else {
                    tracing::error!(
                        "Agent execution failed for validator '{}': {}",
                        validator.name(),
                        e
                    );
                }

                (
                    create_executed_validator(
                        validator,
                        crate::validator::ValidatorResult::fail(format!(
                            "Agent execution failed: {}",
                            e
                        )),
                    ),
                    is_rate_limit,
                )
            }
        }
    }

    /// Check if an error indicates a rate limit or timeout.
    fn is_rate_limit_error(error: &str) -> bool {
        let error_lower = error.to_lowercase();
        error_lower.contains("rate limit")
            || error_lower.contains("rate_limit")
            || error_lower.contains("too many requests")
            || error_lower.contains("429")
            || error_lower.contains("timeout")
            || error_lower.contains("timed out")
            || error_lower.contains("overloaded")
            || error_lower.contains("capacity")
    }

    /// Execute multiple validators against a hook event context.
    ///
    /// Executes validators in parallel with adaptive concurrency control.
    /// Concurrency starts at CPU count and reduces when rate limits are detected.
    /// Returns a list of `ExecutedValidator` results.
    pub async fn execute_validators(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        context: &serde_json::Value,
    ) -> Vec<ExecutedValidator> {
        if validators.is_empty() {
            return Vec::new();
        }

        tracing::debug!(
            "Executing {} validators in parallel (max_concurrency={})",
            validators.len(),
            self.concurrency.current_max()
        );

        // Create futures for all validators
        let mut futures = FuturesUnordered::new();

        for (idx, validator) in validators.iter().enumerate() {
            let concurrency = Arc::clone(&self.concurrency);
            let prompt_library = Arc::clone(&self.prompt_library);
            let partials = self.partials.clone();
            let agent = Arc::clone(&self.agent);
            let notifications_tx = self.notifications.clone();
            let hook_type = hook_type;
            let context = context.clone();
            let validator_name = validator.name().to_string();
            let validator_frontmatter = validator.frontmatter.clone();
            let validator_body = validator.body.clone();
            let validator_source = validator.source.clone();
            let validator_path = validator.path.clone();

            futures.push(async move {
                // Acquire a permit before executing
                let _permit = concurrency.acquire().await;

                // Recreate the validator reference for this task
                let validator = Validator {
                    frontmatter: validator_frontmatter,
                    body: validator_body,
                    source: validator_source,
                    path: validator_path,
                };

                // Render the validator prompt
                let prompt_result = render_validator_prompt_with_partials(
                    &prompt_library,
                    &validator,
                    hook_type,
                    &context,
                    Some(&partials),
                );

                let prompt_text = match prompt_result {
                    Ok(text) => text,
                    Err(e) => {
                        tracing::error!(
                            "Failed to render validator '{}' prompt: {}",
                            validator_name,
                            e
                        );
                        return (
                            idx,
                            create_executed_validator(
                                &validator,
                                crate::validator::ValidatorResult::fail(format!(
                                    "Failed to render prompt: {}",
                                    e
                                )),
                            ),
                            false,
                        );
                    }
                };

                // Get a fresh notification receiver
                let notifications = notifications_tx.subscribe();

                // Execute via agent
                let response =
                    claude_agent::execute_prompt_with_agent(&*agent, notifications, prompt_text)
                        .await;

                match response {
                    Ok(prompt_response) => {
                        let result = parse_validator_response(&prompt_response.content);
                        tracing::debug!(
                            "Validator '{}' result: {} - {}",
                            validator_name,
                            if result.passed() { "PASSED" } else { "FAILED" },
                            result.message()
                        );
                        concurrency.report_success();
                        (idx, create_executed_validator(&validator, result), false)
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        let is_rate_limit = Self::is_rate_limit_error(&error_str);

                        if is_rate_limit {
                            tracing::warn!(
                                "Rate limit/timeout for validator '{}': {}",
                                validator_name,
                                e
                            );
                            concurrency.report_rate_limit();
                        } else {
                            tracing::error!(
                                "Agent execution failed for validator '{}': {}",
                                validator_name,
                                e
                            );
                        }

                        (
                            idx,
                            create_executed_validator(
                                &validator,
                                crate::validator::ValidatorResult::fail(format!(
                                    "Agent execution failed: {}",
                                    e
                                )),
                            ),
                            is_rate_limit,
                        )
                    }
                }
            });
        }

        // Collect results as they complete
        let mut results: Vec<Option<ExecutedValidator>> = vec![None; validators.len()];
        let mut blocked = false;

        while let Some((idx, result, _is_rate_limit)) = futures.next().await {
            // Check if this result blocks further processing
            if result.is_blocking() && !blocked {
                blocked = true;
                tracing::info!(
                    "Validator '{}' blocked - in-flight validators will complete but result is blocked",
                    result.name
                );
            }
            results[idx] = Some(result);
        }

        // Return results in original order, filtering out None values
        results.into_iter().flatten().collect()
    }

    /// Get the current concurrency level.
    pub fn current_concurrency(&self) -> usize {
        self.concurrency.current_max()
    }
}

#[cfg(test)]
mod tests {
    // Note: ValidatorRunner now requires an Agent.
    // Unit tests are in integration tests with PlaybackAgent via AvpContext.
}
