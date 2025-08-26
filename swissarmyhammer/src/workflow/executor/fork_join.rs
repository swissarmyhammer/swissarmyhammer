//! Fork/join parallel execution functionality

use super::core::WorkflowExecutor;
use super::{ExecutionEventType, ExecutorError, ExecutorResult, LAST_ACTION_RESULT_KEY};
use crate::workflow::{
    parse_action_from_description_with_context, StateId, StateType, Workflow, WorkflowRun,
    WorkflowTemplateContext,
};
use serde_json::Value;

/// Represents a parallel execution branch
#[derive(Debug)]
pub struct ParallelBranch {
    /// The state this branch is currently in
    pub current_state: StateId,
    /// The execution context for this branch
    pub context: WorkflowTemplateContext,
    /// History for this branch
    pub history: Vec<(StateId, chrono::DateTime<chrono::Utc>)>,
}

impl WorkflowExecutor {
    /// Check if a state matches a specific state type
    pub fn is_state_type(
        &self,
        run: &WorkflowRun,
        state_id: &StateId,
        state_type: StateType,
    ) -> bool {
        run.workflow
            .states
            .get(state_id)
            .map(|state| state.state_type == state_type)
            .unwrap_or(false)
    }

    /// Check if a state is a fork state
    pub fn is_fork_state(&self, run: &WorkflowRun, state_id: &StateId) -> bool {
        self.is_state_type(run, state_id, StateType::Fork)
    }

    /// Check if a state is a join state
    pub fn is_join_state(&self, run: &WorkflowRun, state_id: &StateId) -> bool {
        self.is_state_type(run, state_id, StateType::Join)
    }

    /// Check if a state is a choice state
    pub fn is_choice_state(&self, run: &WorkflowRun, state_id: &StateId) -> bool {
        self.is_state_type(run, state_id, StateType::Choice)
    }

    /// Find all outgoing transitions from a fork state
    pub fn find_fork_transitions(&self, run: &WorkflowRun, fork_state: &StateId) -> Vec<StateId> {
        run.workflow
            .transitions
            .iter()
            .filter(|t| &t.from_state == fork_state)
            .map(|t| t.to_state.clone())
            .collect()
    }

    /// Find the join state for a set of parallel branches
    ///
    /// Locates the join state where all parallel branches converge.
    /// A valid join state must:
    /// 1. Be of type StateType::Join
    /// 2. Have incoming transitions from ALL branch states
    ///
    /// # Algorithm
    /// - Examines all transitions in the workflow
    /// - For each transition from a branch state to a join-type state
    /// - Verifies all other branches also transition to the same state
    /// - Returns the first valid join state found
    ///
    /// # Returns
    /// - `Some(StateId)` if a valid join state is found
    /// - `None` if no join state exists for all branches
    pub fn find_join_state(&self, run: &WorkflowRun, branch_states: &[StateId]) -> Option<StateId> {
        // Find a state that all branches transition to
        for transition in &run.workflow.transitions {
            if branch_states.contains(&transition.from_state) {
                // Check if this target state is a join state
                if self.is_join_state(run, &transition.to_state) {
                    // Verify all branches lead to this join state
                    let all_branches_lead_here = branch_states.iter().all(|branch| {
                        run.workflow
                            .transitions
                            .iter()
                            .any(|t| &t.from_state == branch && t.to_state == transition.to_state)
                    });

                    if all_branches_lead_here {
                        return Some(transition.to_state.clone());
                    }
                }
            }
        }
        None
    }

    /// Execute a fork state - spawn parallel branches
    ///
    /// Fork states enable parallel execution by spawning multiple execution branches.
    /// Each branch starts from a different state and executes independently until
    /// they all converge at a join state.
    ///
    /// # Errors
    /// - Fork state has no outgoing transitions
    /// - Fork state has only one outgoing transition
    /// - No join state found for the fork branches
    /// - Branch execution fails or doesn't reach join state
    pub async fn execute_fork_state(&mut self, run: &mut WorkflowRun) -> ExecutorResult<()> {
        let fork_state = run.current_state.clone();

        self.log_event(
            ExecutionEventType::StateExecution,
            format!("Executing fork state: {fork_state}"),
        );

        // Validate fork state has valid transitions
        let branch_states = self.validate_fork_transitions(run, &fork_state)?;

        // Find the join state where branches will converge
        let join_state = self.find_join_state_for_branches(run, &fork_state, &branch_states)?;

        self.log_event(
            ExecutionEventType::StateExecution,
            format!(
                "Fork {} spawning {} branches to join at {}",
                fork_state,
                branch_states.len(),
                join_state
            ),
        );

        // Execute all branches in parallel
        let completed_branches = self
            .execute_parallel_branches(run, &branch_states, &join_state)
            .await?;

        // Merge contexts from all branches
        self.merge_branch_contexts(run, completed_branches)?;

        // Transition to the join state
        run.transition_to(join_state);

        Ok(())
    }

    /// Validate fork state transitions
    ///
    /// Ensures the fork state has at least 2 outgoing transitions for parallel execution.
    ///
    /// # Arguments
    /// - `run`: The workflow run context
    /// - `fork_state`: The fork state to validate
    ///
    /// # Returns
    /// - `Ok(Vec<StateId>)`: List of branch states if validation passes
    /// - `Err(ExecutorError)`: If validation fails
    fn validate_fork_transitions(
        &self,
        run: &WorkflowRun,
        fork_state: &StateId,
    ) -> ExecutorResult<Vec<StateId>> {
        let branch_states = self.find_fork_transitions(run, fork_state);

        if branch_states.is_empty() {
            return Err(ExecutorError::ExecutionFailed(
                format!(
                    "Fork state '{fork_state}' has no outgoing transitions. Fork states must have at least two outgoing transitions to parallel branches"
                ),
            ));
        }

        if branch_states.len() < 2 {
            return Err(ExecutorError::ExecutionFailed(
                format!(
                    "Fork state '{}' has only {} outgoing transition. Fork states must have at least two outgoing transitions for parallel execution",
                    fork_state,
                    branch_states.len()
                ),
            ));
        }

        Ok(branch_states)
    }

    /// Find the join state for parallel branches
    ///
    /// Locates the join state where all parallel branches must converge.
    ///
    /// # Arguments
    /// - `run`: The workflow run context
    /// - `fork_state`: The fork state that spawned the branches
    /// - `branch_states`: List of branch states to find join for
    ///
    /// # Returns
    /// - `Ok(StateId)`: The join state if found
    /// - `Err(ExecutorError)`: If no valid join state exists
    fn find_join_state_for_branches(
        &self,
        run: &WorkflowRun,
        fork_state: &StateId,
        branch_states: &[StateId],
    ) -> ExecutorResult<StateId> {
        self.find_join_state(run, branch_states).ok_or_else(|| {
            ExecutorError::ExecutionFailed(
                format!(
                    "No join state found for fork '{fork_state}' with branches: {branch_states:?}. All fork branches must eventually converge at a join state"
                ),
            )
        })
    }

    /// Execute parallel branches
    ///
    /// Executes all branches sequentially with isolated contexts until they reach the join state.
    ///
    /// # Arguments
    /// - `run`: The workflow run context
    /// - `branch_states`: List of branch states to execute
    /// - `join_state`: The join state where branches should converge
    ///
    /// # Returns
    /// - `Ok(Vec<ParallelBranch>)`: List of completed branches with their contexts
    /// - `Err(ExecutorError)`: If any branch execution fails
    async fn execute_parallel_branches(
        &mut self,
        run: &WorkflowRun,
        branch_states: &[StateId],
        join_state: &StateId,
    ) -> ExecutorResult<Vec<ParallelBranch>> {
        let mut completed_branches = Vec::new();

        for branch_state in branch_states {
            // Create a branch with a copy of the current context
            let mut branch = ParallelBranch {
                current_state: branch_state.clone(),
                context: run.context.clone(),
                history: vec![(branch_state.clone(), chrono::Utc::now())],
            };

            // Execute this branch until it reaches the join state
            self.execute_branch_to_join(&run.workflow, &mut branch, join_state)
                .await?;

            self.log_event(
                ExecutionEventType::StateExecution,
                format!("Branch {branch_state} completed"),
            );

            completed_branches.push(branch);
        }

        Ok(completed_branches)
    }

    /// Execute a join state - merge parallel contexts
    pub async fn execute_join_state(&mut self, run: &mut WorkflowRun) -> ExecutorResult<()> {
        let join_state = run.current_state.clone();

        self.log_event(
            ExecutionEventType::StateExecution,
            format!("Executing join state: {join_state}"),
        );

        // Join state execution is mostly handled in the fork state
        // Here we just log that we've reached the join point
        self.log_event(
            ExecutionEventType::StateExecution,
            format!("All branches joined at: {join_state}"),
        );

        Ok(())
    }

    /// Execute a choice state - choice states don't perform any actions
    ///
    /// Choice states are decision points that enable conditional branching.
    /// The actual conditional evaluation and transition logic is handled by
    /// the normal transition evaluation process in evaluate_transitions.
    ///
    /// Choice states simply log their execution and return, allowing the
    /// normal execution cycle to handle the conditional transitions.
    pub async fn execute_choice_state(&mut self, run: &mut WorkflowRun) -> ExecutorResult<()> {
        let choice_state = run.current_state.clone();

        self.log_event(
            ExecutionEventType::StateExecution,
            format!("Executing choice state: {choice_state}"),
        );

        // Choice states don't perform any actions themselves
        // The conditional transitions are handled by the normal transition evaluation
        self.log_event(
            ExecutionEventType::StateExecution,
            format!("Choice state '{choice_state}' ready for conditional transition evaluation"),
        );

        Ok(())
    }

    /// Execute a single branch until it reaches the join state
    ///
    /// Executes a parallel branch in isolation with its own context copy.
    /// The branch executes state actions and follows transitions until it
    /// reaches the target join state. Branch execution is sequential but
    /// isolated from other branches.
    ///
    /// # Arguments
    /// - `workflow`: The workflow definition containing states and transitions
    /// - `branch`: Mutable branch state with context and execution history
    /// - `join_state`: Target join state where this branch should converge
    ///
    /// # Errors
    /// - State not found in workflow
    /// - Transition limit exceeded (prevents infinite loops)
    /// - Branch doesn't reach join state (stuck or missing transitions)
    pub async fn execute_branch_to_join(
        &mut self,
        workflow: &Workflow,
        branch: &mut ParallelBranch,
        join_state: &StateId,
    ) -> ExecutorResult<()> {
        let mut transitions = 0;
        const MAX_BRANCH_TRANSITIONS: usize = 100;

        while &branch.current_state != join_state && transitions < MAX_BRANCH_TRANSITIONS {
            // Get the current state
            let current_state = workflow
                .states
                .get(&branch.current_state)
                .ok_or_else(|| ExecutorError::StateNotFound(branch.current_state.clone()))?;

            // Execute state action if one can be parsed from the description with liquid template rendering
            if let Some(action) = parse_action_from_description_with_context(
                &current_state.description,
                &branch.context.to_workflow_hashmap(),
            )? {
                self.log_event(
                    ExecutionEventType::StateExecution,
                    format!("Branch executing action: {}", action.description()),
                );

                match action.execute(&mut branch.context).await {
                    Ok(_result) => {
                        // Mark action as successful
                        branch
                            .context
                            .insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));
                    }
                    Err(_action_error) => {
                        // Mark action as failed
                        branch
                            .context
                            .insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(false));
                    }
                }
            }

            // Find next transition based on conditions
            let next_state = workflow
                .transitions
                .iter()
                .filter(|t| t.from_state == branch.current_state)
                .find(|t| {
                    // Evaluate the condition (simplified version)
                    use crate::workflow::ConditionType;
                    match &t.condition.condition_type {
                        ConditionType::Always => true,
                        ConditionType::OnSuccess => branch
                            .context
                            .get(LAST_ACTION_RESULT_KEY)
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        ConditionType::OnFailure => !branch
                            .context
                            .get(LAST_ACTION_RESULT_KEY)
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        _ => false, // Skip custom conditions for now
                    }
                })
                .map(|t| t.to_state.clone());

            if let Some(next) = next_state {
                branch.current_state = next.clone();
                branch.history.push((next, chrono::Utc::now()));
                transitions += 1;
            } else {
                break;
            }
        }

        if transitions >= MAX_BRANCH_TRANSITIONS {
            return Err(ExecutorError::TransitionLimitExceeded {
                limit: MAX_BRANCH_TRANSITIONS,
            });
        }

        // Check if the branch reached the join state
        if &branch.current_state != join_state {
            return Err(ExecutorError::ExecutionFailed(
                format!(
                    "Branch execution stopped at state '{}' without reaching join state '{}'. Branch may be stuck or missing required transitions",
                    branch.current_state,
                    join_state
                ),
            ));
        }

        Ok(())
    }

    /// Merge contexts from parallel branches
    ///
    /// Combines execution contexts from all parallel branches using a
    /// last-write-wins strategy. Variables from later branches override
    /// variables from earlier branches if there are conflicts.
    ///
    /// The merge strategy:
    /// 1. Iterates through branches in order
    /// 2. For each branch, copies all variables to main context
    /// 3. Skips execution-specific keys (last_action_result)
    /// 4. Merges branch execution history into main history
    ///
    /// # Future Improvements
    /// - Configurable merge strategies (first-wins, explicit conflict resolution)
    /// - Type-aware merging for complex data structures
    /// - Conflict detection and reporting
    pub fn merge_branch_contexts(
        &mut self,
        run: &mut WorkflowRun,
        branches: Vec<ParallelBranch>,
    ) -> ExecutorResult<()> {
        self.log_event(
            ExecutionEventType::StateExecution,
            format!("Merging contexts from {} branches", branches.len()),
        );

        // Simple merge strategy: combine all variables from all branches
        // In case of conflicts, later branches override earlier ones
        for branch in branches {
            for (key, value) in branch.context.iter() {
                // Skip the last_action_result key as it's execution-specific
                if key != LAST_ACTION_RESULT_KEY {
                    run.context.insert(key.clone(), value.clone());
                }
            }

            // Merge history
            run.history.extend(branch.history);
        }

        Ok(())
    }
}
