//! Graph utilities for workflow analysis
//!
//! This module provides generic graph operations for workflows including
//! reachability analysis, cycle detection, and path finding.

use super::{StateId, Workflow};
use std::collections::{HashMap, HashSet, VecDeque};

/// Result of graph analysis operations
pub type GraphResult<T> = Result<T, GraphError>;

/// Errors that can occur during graph analysis
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// A cycle was detected in the graph starting from the given state
    #[error("Graph contains a cycle starting from state: {0}")]
    CycleDetected(StateId),

    /// The specified state was not found in the workflow
    #[error("State not found in workflow: {0}")]
    StateNotFound(StateId),
}

/// Analyzes workflow graph structure
pub struct WorkflowGraphAnalyzer<'a> {
    workflow: &'a Workflow,
}

impl<'a> WorkflowGraphAnalyzer<'a> {
    /// Creates a new graph analyzer for the given workflow
    pub fn new(workflow: &'a Workflow) -> Self {
        Self { workflow }
    }

    /// Finds all states reachable from the given starting state
    pub fn find_reachable_states(&self, from: &StateId) -> HashSet<StateId> {
        let mut reachable = HashSet::new();
        let mut to_visit = VecDeque::new();
        to_visit.push_back(from.clone());

        while let Some(state_id) = to_visit.pop_front() {
            if reachable.contains(&state_id) {
                continue;
            }
            reachable.insert(state_id.clone());

            // Find all transitions from this state
            for transition in &self.workflow.transitions {
                if transition.from_state == state_id {
                    to_visit.push_back(transition.to_state.clone());
                }
            }
        }

        reachable
    }

    /// Finds all unreachable states in the workflow
    pub fn find_unreachable_states(&self) -> Vec<StateId> {
        let reachable = self.find_reachable_states(&self.workflow.initial_state);

        self.workflow
            .states
            .keys()
            .filter(|state_id| !reachable.contains(state_id))
            .cloned()
            .collect()
    }

    /// Detects cycles in the workflow starting from the given state
    pub fn detect_cycle_from(&self, start: &StateId) -> Option<Vec<StateId>> {
        let mut visited = HashSet::new();
        let mut path = Vec::new();

        if self.has_cycle_dfs(start, &mut visited, &mut path) {
            Some(path)
        } else {
            None
        }
    }

    /// Detects all cycles in the workflow
    pub fn detect_all_cycles(&self) -> Vec<Vec<StateId>> {
        let mut cycles = Vec::new();
        let mut global_visited = HashSet::new();

        for state_id in self.workflow.states.keys() {
            if !global_visited.contains(state_id) {
                let mut local_visited = HashSet::new();
                let mut path = Vec::new();

                if self.detect_cycle_from_state(
                    state_id,
                    &mut local_visited,
                    &mut path,
                    &mut cycles,
                ) {
                    global_visited.extend(local_visited);
                }
            }
        }

        cycles
    }

    /// Finds all paths from one state to another
    pub fn find_paths(&self, from: &StateId, to: &StateId) -> Vec<Vec<StateId>> {
        let mut paths = Vec::new();
        let mut current_path = vec![from.clone()];
        let mut visited = HashSet::new();

        self.find_paths_dfs(from, to, &mut visited, &mut current_path, &mut paths);

        paths
    }

    /// Builds an adjacency list representation of the workflow graph
    pub fn build_adjacency_list(&self) -> HashMap<StateId, Vec<StateId>> {
        let mut adjacency = HashMap::new();

        // Initialize with all states
        for state_id in self.workflow.states.keys() {
            adjacency.insert(state_id.clone(), Vec::new());
        }

        // Add transitions
        for transition in &self.workflow.transitions {
            adjacency
                .entry(transition.from_state.clone())
                .or_insert_with(Vec::new)
                .push(transition.to_state.clone());
        }

        adjacency
    }

    /// Performs topological sort on the workflow graph
    /// Returns None if the graph contains cycles
    pub fn topological_sort(&self) -> Option<Vec<StateId>> {
        let adjacency = self.build_adjacency_list();
        let mut in_degree = HashMap::new();

        // Calculate in-degrees
        for state_id in self.workflow.states.keys() {
            in_degree.insert(state_id.clone(), 0);
        }

        for neighbors in adjacency.values() {
            for neighbor in neighbors {
                *in_degree.get_mut(neighbor).unwrap() += 1;
            }
        }

        // Find all nodes with in-degree 0
        let mut queue = VecDeque::new();
        for (state_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(state_id.clone());
            }
        }

        let mut sorted = Vec::new();

        while let Some(state_id) = queue.pop_front() {
            sorted.push(state_id.clone());

            if let Some(neighbors) = adjacency.get(&state_id) {
                for neighbor in neighbors {
                    let degree = in_degree.get_mut(neighbor).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        // If we processed all nodes, there are no cycles
        if sorted.len() == self.workflow.states.len() {
            Some(sorted)
        } else {
            None
        }
    }

    // Helper method for cycle detection using DFS
    fn has_cycle_dfs(
        &self,
        state: &StateId,
        visited: &mut HashSet<StateId>,
        path: &mut Vec<StateId>,
    ) -> bool {
        if path.contains(state) {
            // Found a cycle - trim path to show just the cycle
            if let Some(pos) = path.iter().position(|s| s == state) {
                path.drain(..pos);
            }
            path.push(state.clone());
            return true;
        }

        if visited.contains(state) {
            return false;
        }

        visited.insert(state.clone());
        path.push(state.clone());

        // Check all outgoing transitions
        for transition in &self.workflow.transitions {
            if transition.from_state == *state
                && self.has_cycle_dfs(&transition.to_state, visited, path)
            {
                return true;
            }
        }

        path.pop();
        false
    }

    // Helper for detecting cycles and collecting them
    fn detect_cycle_from_state(
        &self,
        state: &StateId,
        visited: &mut HashSet<StateId>,
        path: &mut Vec<StateId>,
        cycles: &mut Vec<Vec<StateId>>,
    ) -> bool {
        path.push(state.clone());
        visited.insert(state.clone());

        for transition in &self.workflow.transitions {
            if transition.from_state == *state {
                if path.contains(&transition.to_state) {
                    // Found a cycle
                    if let Some(pos) = path.iter().position(|s| s == &transition.to_state) {
                        let mut cycle = path[pos..].to_vec();
                        cycle.push(transition.to_state.clone());
                        cycles.push(cycle);
                    }
                } else if !visited.contains(&transition.to_state) {
                    self.detect_cycle_from_state(&transition.to_state, visited, path, cycles);
                }
            }
        }

        path.pop();
        true
    }

    // Helper for finding paths using DFS
    fn find_paths_dfs(
        &self,
        current: &StateId,
        target: &StateId,
        visited: &mut HashSet<StateId>,
        current_path: &mut Vec<StateId>,
        all_paths: &mut Vec<Vec<StateId>>,
    ) {
        if current == target {
            all_paths.push(current_path.clone());
            return;
        }

        visited.insert(current.clone());

        for transition in &self.workflow.transitions {
            if transition.from_state == *current && !visited.contains(&transition.to_state) {
                current_path.push(transition.to_state.clone());
                self.find_paths_dfs(
                    &transition.to_state,
                    target,
                    visited,
                    current_path,
                    all_paths,
                );
                current_path.pop();
            }
        }

        visited.remove(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConditionType, State, StateType, Transition, TransitionCondition, WorkflowName,
    };

    fn create_test_workflow() -> Workflow {
        let mut workflow = Workflow::new(
            WorkflowName::new("test"),
            "Test workflow".to_string(),
            StateId::new("start"),
        );

        // Add states
        workflow.add_state(State {
            id: StateId::new("start"),
            description: "Start state".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_state(State {
            id: StateId::new("middle"),
            description: "Middle state".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_state(State {
            id: StateId::new("end"),
            description: "End state".to_string(),
            state_type: StateType::Normal,
            is_terminal: true,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        // Add transitions
        workflow.add_transition(Transition {
            from_state: StateId::new("start"),
            to_state: StateId::new("middle"),
            condition: TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        });

        workflow.add_transition(Transition {
            from_state: StateId::new("middle"),
            to_state: StateId::new("end"),
            condition: TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        });

        workflow
    }

    #[test]
    fn test_find_reachable_states() {
        let workflow = create_test_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let reachable = analyzer.find_reachable_states(&StateId::new("start"));
        assert_eq!(reachable.len(), 3);
        assert!(reachable.contains(&StateId::new("start")));
        assert!(reachable.contains(&StateId::new("middle")));
        assert!(reachable.contains(&StateId::new("end")));
    }

    #[test]
    fn test_detect_cycle() {
        let mut workflow = create_test_workflow();

        // Add a cycle
        workflow.add_transition(Transition {
            from_state: StateId::new("end"),
            to_state: StateId::new("start"),
            condition: TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        });

        let analyzer = WorkflowGraphAnalyzer::new(&workflow);
        let cycle = analyzer.detect_cycle_from(&StateId::new("start"));

        assert!(cycle.is_some());
    }
}
