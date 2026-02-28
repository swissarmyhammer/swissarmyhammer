//! Core workflow execution logic
//! sah rule ignore test_rule_with_allow

use super::{
    ExecutionEvent, ExecutionEventType, ExecutorError, ExecutorResult, DEFAULT_MAX_HISTORY_SIZE,
    LAST_ACTION_RESULT_KEY, MAX_TRANSITIONS,
};
use crate::{
    metrics::{MemoryMetrics, WorkflowMetrics},
    parse_action_from_description_with_context, ActionError, CompensationKey, ErrorContext,
    StateId, Workflow, WorkflowRun, WorkflowRunStatus,
};

use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use swissarmyhammer_config::model::ModelConfig;

/// Workflow execution engine
pub struct WorkflowExecutor {
    /// Execution history for debugging
    execution_history: Vec<ExecutionEvent>,
    /// Maximum size of execution history to prevent unbounded growth
    max_history_size: usize,
    /// Metrics collector for workflow execution
    metrics: WorkflowMetrics,

    /// Optional workflow storage for test mode
    test_storage: Option<Arc<crate::storage::WorkflowStorage>>,
    /// Working directory for file operations
    _working_dir: std::path::PathBuf,

    /// Optional agent configuration for workflow operations
    _agent: Option<Arc<ModelConfig>>,
}

impl WorkflowExecutor {
    /// Common initialization for all constructors
    fn with_config(
        working_dir: std::path::PathBuf,
        test_storage: Option<Arc<crate::storage::WorkflowStorage>>,
        agent: Option<Arc<ModelConfig>>,
    ) -> Self {
        Self {
            execution_history: Vec::new(),
            max_history_size: DEFAULT_MAX_HISTORY_SIZE,
            metrics: WorkflowMetrics::new(),
            test_storage,
            _working_dir: working_dir,
            _agent: agent,
        }
    }

    /// Create a new workflow executor
    pub fn new() -> Self {
        Self::with_config(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            None,
            None,
        )
    }

    /// Create a new workflow executor with custom working directory
    pub fn with_working_dir<P: AsRef<std::path::Path>>(working_dir: P) -> Self {
        Self::with_config(working_dir.as_ref().to_path_buf(), None, None)
    }

    /// Create a new workflow executor with test storage
    pub fn with_test_storage(storage: Arc<crate::storage::WorkflowStorage>) -> Self {
        Self::with_config(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            Some(storage),
            None,
        )
    }

    /// Create a workflow executor with agent configuration
    pub fn with_agent(agent: Arc<ModelConfig>) -> Self {
        Self::with_config(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            None,
            Some(agent),
        )
    }

    /// Create a workflow executor with working directory and agent
    pub fn with_working_dir_and_agent<P: AsRef<std::path::Path>>(
        working_dir: P,
        agent: Arc<ModelConfig>,
    ) -> Self {
        Self::with_config(working_dir.as_ref().to_path_buf(), None, Some(agent))
    }

    /// Get the workflow storage (test storage if available, otherwise create file system storage)
    pub fn get_storage(
        &self,
    ) -> swissarmyhammer_common::Result<Arc<crate::storage::WorkflowStorage>> {
        if let Some(storage) = &self.test_storage {
            Ok(storage.clone())
        } else {
            Ok(Arc::new(crate::storage::WorkflowStorage::file_system()?))
        }
    }

    /// Log state entry
    fn log_state_entry(&mut self, state_id: &StateId, workflow_name: &crate::WorkflowName) {
        let msg = format!(
            "ENTERING state: {} for workflow {}",
            state_id, workflow_name
        );
        tracing::info!("{}", msg);
        self.log_event(ExecutionEventType::StateExecution, msg);
    }

    /// Log state exit
    fn log_state_exit(
        &mut self,
        state_id: &StateId,
        workflow_name: &crate::WorkflowName,
        success: bool,
    ) {
        let status = if success { "success" } else { "error" };
        let msg = format!(
            "EXITING state: {} for workflow {} ({})",
            state_id, workflow_name, status
        );
        if success {
            tracing::info!("{}", msg);
            self.log_event(ExecutionEventType::StateExecution, msg);
        } else {
            tracing::error!("{}", msg);
            self.log_event(ExecutionEventType::Failed, msg);
        }
    }

    /// Start a new workflow run (initializes but doesn't execute)
    pub fn start_workflow(&mut self, workflow: Workflow) -> ExecutorResult<WorkflowRun> {
        // Validate workflow before starting
        workflow
            .validate_structure()
            .map_err(|errors| ExecutorError::ValidationFailed(errors.join("; ")))?;

        // Create WorkflowRun with agent if available
        let run = if let Some(agent) = &self._agent {
            WorkflowRun::new_with_agent(workflow, agent.clone())
        } else {
            WorkflowRun::new(workflow)
        };

        // Start metrics tracking for this run
        self.metrics.start_run(run.id, run.workflow.name.clone());

        self.log_event(
            ExecutionEventType::Started,
            format!("Started workflow: {}", run.workflow.name),
        );

        Ok(run)
    }

    /// Execute workflow and complete metrics tracking
    async fn execute_and_complete_metrics(
        &mut self,
        run: &mut WorkflowRun,
        transition_limit: usize,
    ) -> ExecutorResult<WorkflowRun> {
        let result = self.execute_state_with_limit(run, transition_limit).await;

        match &result {
            Ok(_) => {
                self.metrics.complete_run(&run.id, run.status, None);
            }
            Err(e) => {
                self.metrics
                    .complete_run(&run.id, WorkflowRunStatus::Failed, Some(e.to_string()));
            }
        }

        result.map(|_| run.clone())
    }

    /// Start and execute a new workflow run
    pub async fn start_and_execute_workflow(
        &mut self,
        workflow: Workflow,
    ) -> ExecutorResult<WorkflowRun> {
        let mut run = self.start_workflow(workflow)?;
        self.execute_and_complete_metrics(&mut run, MAX_TRANSITIONS)
            .await
    }

    /// Start and execute a workflow with a custom transition limit (for testing)
    #[cfg(test)]
    pub async fn start_and_execute_workflow_with_limit(
        &mut self,
        workflow: Workflow,
        transition_limit: usize,
    ) -> ExecutorResult<WorkflowRun> {
        let mut run = self.start_workflow(workflow)?;
        self.execute_and_complete_metrics(&mut run, transition_limit)
            .await
    }

    /// Check if workflow execution should stop
    pub fn is_workflow_finished(&self, run: &WorkflowRun) -> bool {
        run.status == WorkflowRunStatus::Completed || run.status == WorkflowRunStatus::Failed
    }

    /// Check if workflow execution was aborted via context variable
    fn check_for_abort(&self, run: &WorkflowRun) -> ExecutorResult<()> {
        if let Some(abort_reason_value) = run.context.get_workflow_var("__ABORT_REQUESTED__") {
            if let Some(abort_reason) = abort_reason_value.as_str() {
                tracing::error!("***Workflow Aborted***: {}", abort_reason);
                return Err(ExecutorError::Abort(abort_reason.to_string()));
            }
        }
        Ok(())
    }

    /// Check if workflow exit was requested via JS global state.
    ///
    /// When an inline prompt action calls `flow` with `op: "exit"`, it sets
    /// `__workflow_exit__` in the process-global JS state. This method checks
    /// for that flag and, if found, cleanly completes the workflow.
    async fn check_for_exit(&self, run: &mut WorkflowRun) -> bool {
        let js_state = swissarmyhammer_js::JsState::global();
        match js_state.get("__workflow_exit__").await {
            Ok(value) => {
                let should_exit = match &value {
                    serde_json::Value::Bool(true) => true,
                    serde_json::Value::String(s) if s == "true" => true,
                    _ => false,
                };
                if should_exit {
                    tracing::info!("🚪 Workflow exit requested via __workflow_exit__ flag");
                    run.status = crate::run::WorkflowRunStatus::Completed;
                    // Clear the flag so it doesn't affect subsequent workflows
                    let _ = js_state.set("__workflow_exit__", "false").await;
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    /// Execute a single execution cycle: state execution and potential transition
    pub async fn execute_single_cycle(&mut self, run: &mut WorkflowRun) -> ExecutorResult<bool> {
        tracing::debug!("Execute single cycle for state: {}", run.current_state);

        // Execute the state and capture any errors
        let state_error = self.execute_state_and_capture_errors(run).await?;

        // Check if abort was requested via context variable (after state execution)
        self.check_for_abort(run)?;

        // Check if exit was requested via JS global state (after state execution)
        if self.check_for_exit(run).await {
            return Ok(false); // No transition needed, workflow cleanly exiting
        }

        // Check if workflow is complete after state execution
        if self.is_workflow_finished(run) {
            return Ok(false); // No transition needed, workflow finished
        }

        // Evaluate and perform transition
        self.evaluate_and_perform_transition(run, state_error).await
    }

    /// Execute state and capture errors for later processing
    async fn execute_state_and_capture_errors(
        &mut self,
        run: &mut WorkflowRun,
    ) -> ExecutorResult<Option<ExecutorError>> {
        // Execute the state, but don't propagate action errors immediately
        // We need to check for OnFailure transitions first
        let state_result = self.execute_single_state(run).await;

        // If it's an action error, we'll handle it after checking transitions
        match state_result {
            Err(ExecutorError::ActionError(e)) => Ok(Some(ExecutorError::ActionError(e))),
            Err(ExecutorError::ManualInterventionRequired(msg)) => {
                // Manual intervention required, workflow is paused
                Ok(Some(ExecutorError::ManualInterventionRequired(msg)))
            }
            Err(other) => Err(other), // Propagate non-action errors
            Ok(()) => Ok(None),       // No error
        }
    }

    /// Evaluate transitions and perform them if available
    async fn evaluate_and_perform_transition(
        &mut self,
        run: &mut WorkflowRun,
        state_error: Option<ExecutorError>,
    ) -> ExecutorResult<bool> {
        // Early return for manual intervention case
        if let Some(ExecutorError::ManualInterventionRequired(_)) = state_error {
            return Ok(false);
        }

        // Try to find and perform transition
        let next_state = self.evaluate_transitions(run)?;

        if let Some(next_state) = next_state {
            self.perform_transition(run, next_state)?;
            return Ok(true); // Transition performed
        }

        // No valid transitions found
        if let Some(error) = state_error {
            return Err(error); // Had an error, propagate it
        }

        Ok(false) // No transition, workflow is stuck
    }

    /// Execute states with a maximum transition limit to prevent infinite loops
    pub async fn execute_state_with_limit(
        &mut self,
        run: &mut WorkflowRun,
        transition_limit: usize,
    ) -> ExecutorResult<()> {
        if transition_limit == 0 {
            return Err(ExecutorError::TransitionLimitExceeded {
                limit: transition_limit,
            });
        }

        let mut current_remaining = transition_limit;

        loop {
            tracing::debug!(
                "Workflow execution loop - current state: {}",
                run.current_state
            );
            let transition_performed = self.execute_single_cycle(run).await?;

            if !transition_performed {
                // Either workflow finished or no transitions available
                tracing::debug!("No transition performed, exiting loop");
                break;
            }

            current_remaining -= 1;
            if current_remaining == 0 {
                return Err(ExecutorError::TransitionLimitExceeded {
                    limit: transition_limit,
                });
            }
        }

        Ok(())
    }

    /// Execute the current state and evaluate transitions
    pub async fn execute_state(&mut self, run: &mut WorkflowRun) -> ExecutorResult<()> {
        self.execute_state_with_limit(run, MAX_TRANSITIONS).await
    }

    /// Execute a single state without transitioning
    pub async fn execute_single_state(&mut self, run: &mut WorkflowRun) -> ExecutorResult<()> {
        let current_state_id = run.current_state.clone();

        // Early return for terminal state [*]
        if self.handle_terminal_state(run, &current_state_id)? {
            return Ok(());
        }

        // Early return for special states (fork, join, choice)
        if let Some(result) = self.execute_special_state(run, &current_state_id).await? {
            return result;
        }

        // Get the current state
        let current_state = run
            .workflow
            .states
            .get(&current_state_id)
            .ok_or_else(|| ExecutorError::StateNotFound(current_state_id.clone()))?;

        let state_description = current_state.description.clone();
        let is_terminal = current_state.is_terminal;

        // Log state execution
        tracing::trace!(
            "Executing state: {} - {} for workflow {}",
            current_state.id,
            current_state.description,
            run.workflow.name
        );
        self.log_event(
            ExecutionEventType::StateExecution,
            format!(
                "Executing state: {} - {} for workflow {}",
                current_state.id, current_state.description, run.workflow.name
            ),
        );

        self.log_state_entry(&current_state_id, &run.workflow.name);

        // Execute the state action
        let action_executed = self
            .execute_and_time_state_action(run, &current_state_id, &state_description)
            .await?;

        // Early return if manual intervention required
        self.check_manual_intervention(run, &current_state_id)?;

        // Handle terminal states
        if is_terminal {
            run.complete();
            tracing::debug!("Terminal state reached: {}", current_state_id);
            if !action_executed {
                self.log_event(
                    ExecutionEventType::Completed,
                    "Workflow completed".to_string(),
                );
            }
        }

        Ok(())
    }

    /// Handle terminal state [*] execution
    fn handle_terminal_state(
        &mut self,
        run: &mut WorkflowRun,
        current_state_id: &StateId,
    ) -> ExecutorResult<bool> {
        if current_state_id.as_str() == "[*]" {
            tracing::debug!("Reached terminal state [*]");
            run.complete();
            return Ok(true);
        }
        Ok(false)
    }

    /// Execute special states (fork, join, choice)
    async fn execute_special_state(
        &mut self,
        run: &mut WorkflowRun,
        current_state_id: &StateId,
    ) -> ExecutorResult<Option<ExecutorResult<()>>> {
        if self.is_fork_state(run, current_state_id) {
            return Ok(Some(self.execute_fork_state(run).await));
        }

        if self.is_join_state(run, current_state_id) {
            return Ok(Some(self.execute_join_state(run).await));
        }

        if self.is_choice_state(run, current_state_id) {
            return Ok(Some(self.execute_choice_state(run).await));
        }

        Ok(None)
    }

    /// Execute state action with timing and logging
    async fn execute_and_time_state_action(
        &mut self,
        run: &mut WorkflowRun,
        current_state_id: &StateId,
        state_description: &str,
    ) -> ExecutorResult<bool> {
        let state_start_time = Instant::now();

        tracing::debug!(
            "About to execute action for state {} with description: {}",
            current_state_id,
            state_description
        );

        let action_result = self.execute_state_action(run, state_description).await;
        let action_executed = match action_result {
            Ok(executed) => {
                self.log_state_exit(current_state_id, &run.workflow.name, true);
                executed
            }
            Err(e) => {
                self.log_state_exit(current_state_id, &run.workflow.name, false);
                return Err(e);
            }
        };

        let state_duration = state_start_time.elapsed();
        self.metrics
            .record_state_execution(&run.id, current_state_id.clone(), state_duration);

        Ok(action_executed)
    }

    /// Check if manual intervention is required
    fn check_manual_intervention(
        &mut self,
        run: &WorkflowRun,
        current_state_id: &StateId,
    ) -> ExecutorResult<()> {
        if self.requires_manual_intervention(run) {
            self.log_event(
                ExecutionEventType::StateExecution,
                format!("State {current_state_id} requires manual intervention"),
            );

            if !run
                .context
                .get("manual_approval")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Err(ExecutorError::ManualInterventionRequired(format!(
                    "State {current_state_id} requires manual approval"
                )));
            }
        }
        Ok(())
    }

    /// Perform a state transition without executing the new state
    pub fn perform_transition(
        &mut self,
        run: &mut WorkflowRun,
        next_state: StateId,
    ) -> ExecutorResult<()> {
        // Verify the state exists
        if !run.workflow.states.contains_key(&next_state) {
            return Err(ExecutorError::StateNotFound(next_state.clone()));
        }

        // Track compensation states from transition metadata
        if let Some(transition) = run
            .workflow
            .transitions
            .iter()
            .find(|t| t.from_state == run.current_state && t.to_state == next_state)
        {
            if let Some(comp_state) = transition.metadata.get("compensation_state") {
                // Store compensation state in context for this transition
                let comp_key = CompensationKey::for_state(&run.current_state);
                run.context
                    .insert(comp_key.into(), Value::String(comp_state.clone()));
            }
        }

        tracing::debug!(
            "Transitioning from {} to {} for workflow {}",
            run.current_state,
            next_state,
            run.workflow.name
        );
        self.log_event(
            ExecutionEventType::StateTransition,
            format!(
                "Transitioning from {} to {} for workflow {}",
                run.current_state, next_state, run.workflow.name
            ),
        );

        // Record transition in metrics
        self.metrics.record_transition(&run.id);

        // Update the run
        run.transition_to(next_state);

        Ok(())
    }

    /// Transition to a new state (public API that includes execution)
    pub async fn transition_to(
        &mut self,
        run: &mut WorkflowRun,
        next_state: StateId,
    ) -> ExecutorResult<()> {
        self.perform_transition(run, next_state)?;
        self.execute_state(run).await
    }

    /// Find transitions TO the given state
    fn find_transitions_to_state<'a>(
        &self,
        run: &'a WorkflowRun,
        state_id: &StateId,
    ) -> Vec<&'a crate::Transition> {
        run.workflow
            .transitions
            .iter()
            .filter(|t| &t.to_state == state_id)
            .collect()
    }

    /// Get metadata value from transitions TO the current state
    fn get_transition_metadata(&self, run: &WorkflowRun, key: &str) -> Option<String> {
        let transitions = self.find_transitions_to_state(run, &run.current_state);
        for transition in transitions {
            if let Some(value) = transition.metadata.get(key) {
                return Some(value.clone());
            }
        }
        None
    }

    /// Preprocess action text (convert set_variable to set format)
    fn preprocess_action_text(&self, action_text: &str) -> String {
        if action_text.starts_with("set_variable ") {
            action_text.replace("set_variable ", "set ")
        } else {
            action_text.to_string()
        }
    }

    /// Execute action parsed from state description
    pub async fn execute_state_action(
        &mut self,
        run: &mut WorkflowRun,
        state_description: &str,
    ) -> ExecutorResult<bool> {
        // Render the state description with workflow variables (liquid templates)
        let rendered_description = run.context.render_template(state_description);

        // Parse rendered description to extract action and store-as field
        let context_hashmap = run.context.to_workflow_hashmap();
        let (action_text, store_as_var) = self.parse_state_description(&rendered_description);

        // Preprocess action text for compatibility
        let action_text = self.preprocess_action_text(&action_text);

        tracing::debug!(
            "Rendered state description: '{}' -> '{}'",
            state_description,
            rendered_description
        );
        tracing::debug!(
            "Parsed state description - action: '{}', store_as: {:?}",
            action_text,
            store_as_var
        );

        // Try to parse action
        let action = parse_action_from_description_with_context(&action_text, &context_hashmap)?;

        if let Some(action) = action {
            self.log_event(
                ExecutionEventType::StateExecution,
                format!("Executing action: {}", action.description()),
            );

            // Execute the action and handle result
            let result = self.execute_action_direct(run, action).await;
            self.handle_action_result_with_store_as(run, result, store_as_var)
                .await?;
            return Ok(true);
        }

        tracing::warn!("No action could be parsed from: '{}'", action_text);
        Ok(false)
    }

    /// Extract action text from a line if it matches the action pattern
    fn extract_action_from_line(&self, line: &str) -> Option<String> {
        if line.starts_with("**Action**:") || line.starts_with("**action**:") {
            let action = line
                .strip_prefix("**Action**:")
                .or_else(|| line.strip_prefix("**action**:"))
                .unwrap_or("")
                .trim();
            return Some(action.to_string());
        }
        None
    }

    /// Extract store-as variable from a line if it matches the pattern
    fn extract_store_as_from_line(&self, line: &str) -> Option<String> {
        if line.starts_with("**Store As**:")
            || line.starts_with("**store as**:")
            || line.starts_with("**Store as**:")
        {
            let store_var = line
                .strip_prefix("**Store As**:")
                .or_else(|| line.strip_prefix("**store as**:"))
                .or_else(|| line.strip_prefix("**Store as**:"))
                .unwrap_or("")
                .trim();
            if !store_var.is_empty() {
                return Some(store_var.to_string());
            }
        }
        None
    }

    /// Parse state description to extract action text and Store As variable
    fn parse_state_description(&self, state_description: &str) -> (String, Option<String>) {
        let mut action_text = String::new();
        let mut store_as_var = None;

        tracing::debug!("Received state description:\n{}", state_description);

        for line in state_description.lines() {
            let line = line.trim();

            // Try to extract action text
            if let Some(action) = self.extract_action_from_line(line) {
                action_text = action;
            }

            // Try to extract store-as variable
            if let Some(store_var) = self.extract_store_as_from_line(line) {
                store_as_var = Some(store_var);
            }
        }

        // If no action found, use the entire description as fallback
        if action_text.is_empty() {
            action_text = state_description.to_string();
        }

        (action_text, store_as_var)
    }

    /// Execute action directly without retry logic
    async fn execute_action_direct(
        &mut self,
        run: &mut WorkflowRun,
        action: Box<dyn crate::Action>,
    ) -> Result<Value, ActionError> {
        // Execute action with mutable WorkflowTemplateContext directly
        action.execute(&mut run.context).await
    }

    /// Set standard action result variables in context
    fn set_action_result_vars(
        &mut self,
        run: &mut WorkflowRun,
        success: bool,
        result_value: Value,
    ) {
        run.context
            .insert("success".to_string(), Value::Bool(success));
        run.context
            .insert("failure".to_string(), Value::Bool(!success));

        if !success {
            run.context
                .insert("is_error".to_string(), Value::Bool(true));
        } else if !run
            .context
            .get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            run.context
                .insert("is_error".to_string(), Value::Bool(false));
        }

        tracing::debug!(
            "Setting 'result' in workflow context. Is object: {}, Value: {:?}",
            result_value.is_object(),
            result_value
        );
        run.context.insert("result".to_string(), result_value);
        run.context
            .insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(success));
    }

    /// Store action result in the optional Store As variable
    fn store_action_result(
        &mut self,
        run: &mut WorkflowRun,
        store_as_var: Option<String>,
        result_value: &Value,
    ) {
        if let Some(store_var) = store_as_var {
            run.context
                .set_workflow_var(store_var.clone(), result_value.clone());
            tracing::debug!(
                "Stored action result in workflow variable '{}': {:?}",
                store_var,
                result_value
            );
        }
    }

    /// Handle the result of action execution with optional Store As variable
    async fn handle_action_result_with_store_as(
        &mut self,
        run: &mut WorkflowRun,
        result: Result<Value, ActionError>,
        store_as_var: Option<String>,
    ) -> ExecutorResult<()> {
        match result {
            Ok(result_value) => {
                // Set standard variables that are available after every action
                self.set_action_result_vars(run, true, result_value.clone());

                // Store result in the Store As variable if specified
                self.store_action_result(run, store_as_var, &result_value);

                self.log_event(
                    ExecutionEventType::StateExecution,
                    "Action completed successfully".to_string(),
                );
                Ok(())
            }
            Err(action_error) => self.handle_action_error(run, action_error).await,
        }
    }

    /// Handle action execution error
    async fn handle_action_error(
        &mut self,
        run: &mut WorkflowRun,
        action_error: ActionError,
    ) -> ExecutorResult<()> {
        // Set standard variables that are available after every action
        self.set_action_result_vars(run, false, Value::String(action_error.to_string()));

        // Capture error context
        self.capture_error_context(run, &action_error);

        // Log the error with appropriate details
        let error_details = self.format_action_error(&action_error);
        self.log_event(ExecutionEventType::Failed, error_details);

        // Guard clause: Check for dead letter state configuration first
        if let Some(dead_letter_state) = self.get_dead_letter_state(run) {
            return self
                .handle_dead_letter_transition(run, dead_letter_state, &action_error)
                .await;
        }

        // Execute compensation if needed
        if let Err(comp_error) = self.execute_compensation(run).await {
            self.log_event(
                ExecutionEventType::Failed,
                format!("Compensation failed: {comp_error}"),
            );
        }

        // Guard clause: Check if this state should be skipped on failure
        if self.should_skip_on_failure(run) {
            self.log_event(
                ExecutionEventType::StateExecution,
                "Skipped failed state due to skip_on_failure configuration".to_string(),
            );
            run.context
                .insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));
            return Ok(());
        }

        // Don't immediately mark workflow as failed - let transitions handle the error
        // The workflow only fails if there's no error transition available
        Err(ExecutorError::ActionError(action_error))
    }

    /// Capture error context for the action error
    fn capture_error_context(&mut self, run: &mut WorkflowRun, action_error: &ActionError) {
        let error_context = ErrorContext::new(action_error.to_string(), run.current_state.clone());
        let error_context_json = serde_json::to_value(&error_context).unwrap_or(Value::Null);
        run.context
            .insert(ErrorContext::CONTEXT_KEY.to_string(), error_context_json);
    }

    /// Format action error for logging
    fn format_action_error(&self, action_error: &ActionError) -> String {
        match action_error {
            ActionError::ClaudeError(msg) => format!("Claude command failed: {msg}"),
            ActionError::VariableError(msg) => {
                format!("Variable operation failed: {msg}")
            }
            ActionError::IoError(io_err) => format!("IO operation failed: {io_err}"),
            ActionError::JsonError(json_err) => {
                format!("JSON parsing failed: {json_err}")
            }
            ActionError::ParseError(msg) => format!("Action parsing failed: {msg}"),
            ActionError::ExecutionError(msg) => {
                format!("Action execution failed: {msg}")
            }
            ActionError::RateLimit { message, wait_time } => {
                format!("Rate limit reached: {message}. Please wait {wait_time:?} before retrying.")
            }
            ActionError::ShellSecurityError(security_error) => {
                format!("Shell security violation: {security_error}")
            }
        }
    }

    /// Prepare context for dead letter transition
    fn prepare_dead_letter_context(&mut self, run: &mut WorkflowRun, action_error: &ActionError) {
        run.context.insert(
            "dead_letter_reason".to_string(),
            Value::String(format!("Max retries exhausted: {action_error}")),
        );
    }

    /// Handle transition to dead letter state
    async fn handle_dead_letter_transition(
        &mut self,
        run: &mut WorkflowRun,
        dead_letter_state: StateId,
        action_error: &ActionError,
    ) -> ExecutorResult<()> {
        // Prepare context with dead letter reason
        self.prepare_dead_letter_context(run, action_error);

        // Transition to dead letter state
        self.log_event(
            ExecutionEventType::StateTransition,
            format!("Transitioning to dead letter state: {dead_letter_state}"),
        );
        self.perform_transition(run, dead_letter_state)?;

        // Mark action as successful to allow workflow to continue
        run.context
            .insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));
        Ok(())
    }

    /// Get dead letter state from transition metadata
    fn get_dead_letter_state(&self, run: &WorkflowRun) -> Option<StateId> {
        self.get_transition_metadata(run, "dead_letter_state")
            .map(|state| StateId::new(&state))
    }

    /// Check if state should be skipped on failure
    fn should_skip_on_failure(&self, run: &WorkflowRun) -> bool {
        self.get_transition_metadata(run, "skip_on_failure")
            .map(|v| v == "true")
            .unwrap_or(false)
    }

    /// Check if current state requires manual intervention
    pub fn requires_manual_intervention(&self, run: &WorkflowRun) -> bool {
        if let Some(state) = run.workflow.states.get(&run.current_state) {
            if let Some(intervention) = state.metadata.get("requires_manual_intervention") {
                return intervention == "true";
            }
        }
        false
    }

    /// Collect all compensation states from context
    fn collect_compensation_states(&self, run: &WorkflowRun) -> Vec<(String, StateId)> {
        let mut compensation_states = Vec::new();

        for (key, value) in run.context.iter() {
            if CompensationKey::is_compensation_key(key) {
                if let Value::String(comp_state) = value {
                    compensation_states.push((key.to_string(), StateId::new(comp_state)));
                }
            }
        }

        compensation_states
    }

    /// Execute compensation states in reverse order
    async fn execute_compensation(&mut self, run: &mut WorkflowRun) -> ExecutorResult<()> {
        self.log_event(
            ExecutionEventType::StateExecution,
            "Starting compensation/rollback".to_string(),
        );

        // Collect all compensation states stored in context
        let compensation_states = self.collect_compensation_states(run);

        // Early return if no compensation states
        if compensation_states.is_empty() {
            return Ok(());
        }

        // Execute the first compensation state
        if let Some((key, comp_state)) = compensation_states.into_iter().next() {
            self.log_event(
                ExecutionEventType::StateExecution,
                format!("Executing compensation state: {comp_state}"),
            );

            // Just transition to the compensation state, don't execute it
            // The normal workflow execution will handle it
            self.perform_transition(run, comp_state)?;

            // Remove from context after execution
            run.context.remove(&key);
        }

        Ok(())
    }

    /// Log an execution event
    pub fn log_event(&mut self, event_type: ExecutionEventType, details: String) {
        tracing::trace!("{}: {}", event_type, &details);
        let event = ExecutionEvent {
            timestamp: chrono::Utc::now(),
            event_type,
            details,
        };
        self.execution_history.push(event);

        // Trim history if it exceeds max size
        if self.execution_history.len() > self.max_history_size {
            let trim_count = self.execution_history.len() - self.max_history_size;
            self.execution_history.drain(0..trim_count);
        }
    }

    /// Get the execution history
    pub fn get_history(&self) -> &[ExecutionEvent] {
        &self.execution_history
    }

    /// Set the maximum history size
    pub fn set_max_history_size(&mut self, max_size: usize) {
        self.max_history_size = max_size;
    }

    /// Get workflow metrics
    pub fn get_metrics(&self) -> &WorkflowMetrics {
        &self.metrics
    }

    /// Get mutable access to workflow metrics
    pub fn get_metrics_mut(&mut self) -> &mut WorkflowMetrics {
        &mut self.metrics
    }

    /// Update memory metrics for a specific run
    pub fn update_memory_metrics(
        &mut self,
        run_id: &crate::WorkflowRunId,
        context_vars: usize,
        history_size: usize,
    ) {
        // Simple memory estimation - in production this would use actual memory profiling
        let estimated_memory = (context_vars * 1024) + (history_size * 256);
        let mut memory_metrics = MemoryMetrics::new();
        memory_metrics.update(estimated_memory as u64, context_vars, history_size);
        self.metrics.update_memory_metrics(run_id, memory_metrics);
    }
}

impl Default for WorkflowExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_executor() -> WorkflowExecutor {
        WorkflowExecutor::new()
    }

    #[test]
    fn test_parse_state_description() {
        let executor = setup_executor();

        // Test state description with Action and Store As
        let description = r#"**Type**: action
**Action**: set_variable step1="First step completed"
**Store As**: step1_result"#;

        let (action_text, store_as_var) = executor.parse_state_description(description);

        assert_eq!(action_text, r#"set_variable step1="First step completed""#);
        assert_eq!(store_as_var, Some("step1_result".to_string()));
    }

    #[test]
    fn test_parse_state_description_no_store_as() {
        let executor = setup_executor();

        // Test state description with only Action
        let description = r#"**Type**: action
**Action**: set_variable step1="First step completed""#;

        let (action_text, store_as_var) = executor.parse_state_description(description);

        assert_eq!(action_text, r#"set_variable step1="First step completed""#);
        assert_eq!(store_as_var, None);
    }

    #[test]
    fn test_parse_state_description_fallback() {
        let executor = setup_executor();

        // Test state description with no Action field
        let description = r#"set_variable step1="First step completed""#;

        let (action_text, store_as_var) = executor.parse_state_description(description);

        assert_eq!(action_text, r#"set_variable step1="First step completed""#);
        assert_eq!(store_as_var, None);
    }
}
