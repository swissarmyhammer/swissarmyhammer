//! Comprehensive tests for workflow graph analysis
//!
//! This module contains thorough tests for graph utilities including
//! reachability analysis, cycle detection, path finding, and topological sorting.

use super::*;
use crate::{test_helpers::*, ConditionType, WorkflowName};

/// Helper function to create a workflow with unreachable states
fn create_workflow_with_unreachable_states() -> Workflow {
    let mut workflow = Workflow::new(
        WorkflowName::new("test"),
        "Test workflow".to_string(),
        StateId::new("start"),
    );

    // Add reachable states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("middle", "Middle state", false));
    workflow.add_state(create_state("end", "End state", true));

    // Add unreachable states
    workflow.add_state(create_state("unreachable1", "Unreachable state 1", false));
    workflow.add_state(create_state("unreachable2", "Unreachable state 2", false));

    // Add transitions for reachable states only
    workflow.add_transition(create_transition("start", "middle", ConditionType::Always));
    workflow.add_transition(create_transition("middle", "end", ConditionType::Always));

    // Add transition between unreachable states
    workflow.add_transition(create_transition(
        "unreachable1",
        "unreachable2",
        ConditionType::Always,
    ));

    workflow
}

/// Helper function to create a workflow with multiple cycles
fn create_workflow_with_multiple_cycles() -> Workflow {
    let mut workflow = Workflow::new(
        WorkflowName::new("test"),
        "Test workflow".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("a", "State A", false));
    workflow.add_state(create_state("b", "State B", false));
    workflow.add_state(create_state("c", "State C", false));
    workflow.add_state(create_state("d", "State D", false));

    // Create cycle 1: a -> b -> a
    workflow.add_transition(create_transition("start", "a", ConditionType::Always));
    workflow.add_transition(create_transition("a", "b", ConditionType::Always));
    workflow.add_transition(create_transition("b", "a", ConditionType::Always));

    // Create cycle 2: c -> d -> c
    workflow.add_transition(create_transition("start", "c", ConditionType::Always));
    workflow.add_transition(create_transition("c", "d", ConditionType::Always));
    workflow.add_transition(create_transition("d", "c", ConditionType::Always));

    workflow
}

/// Helper function to create a complex workflow for path finding
fn create_complex_workflow_for_paths() -> Workflow {
    let mut workflow = Workflow::new(
        WorkflowName::new("test"),
        "Test workflow".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("a", "State A", false));
    workflow.add_state(create_state("b", "State B", false));
    workflow.add_state(create_state("c", "State C", false));
    workflow.add_state(create_state("end", "End state", true));

    // Create multiple paths from start to end
    // Path 1: start -> a -> end
    workflow.add_transition(create_transition("start", "a", ConditionType::Always));
    workflow.add_transition(create_transition("a", "end", ConditionType::Always));

    // Path 2: start -> b -> end
    workflow.add_transition(create_transition("start", "b", ConditionType::Always));
    workflow.add_transition(create_transition("b", "end", ConditionType::Always));

    // Path 3: start -> c -> a -> end
    workflow.add_transition(create_transition("start", "c", ConditionType::Always));
    workflow.add_transition(create_transition("c", "a", ConditionType::Always));

    // Path 4: start -> b -> c -> a -> end
    workflow.add_transition(create_transition("b", "c", ConditionType::Always));

    workflow
}

/// Helper function to create a workflow with self-loops
fn create_workflow_with_self_loops() -> Workflow {
    let mut workflow = Workflow::new(
        WorkflowName::new("test"),
        "Test workflow".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("loop", "Loop state", false));
    workflow.add_state(create_state("end", "End state", true));

    // Add transitions including self-loop
    workflow.add_transition(create_transition("start", "loop", ConditionType::Always));
    workflow.add_transition(create_transition("loop", "loop", ConditionType::Always)); // Self-loop
    workflow.add_transition(create_transition("loop", "end", ConditionType::Always));

    workflow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_analyzer_creation() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        // Test that analyzer is created successfully by calling a method
        let reachable = analyzer.find_reachable_states(&StateId::new("start"));
        assert!(!reachable.is_empty());
    }

    #[test]
    fn test_find_reachable_states_basic() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let reachable = analyzer.find_reachable_states(&StateId::new("start"));

        assert_eq!(reachable.len(), 2);
        assert!(reachable.contains(&StateId::new("start")));
        assert!(reachable.contains(&StateId::new("end")));
    }

    #[test]
    fn test_find_reachable_states_empty() {
        let workflow = Workflow::new(
            WorkflowName::new("empty"),
            "Empty workflow".to_string(),
            StateId::new("start"),
        );
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let reachable = analyzer.find_reachable_states(&StateId::new("start"));

        assert_eq!(reachable.len(), 1);
        assert!(reachable.contains(&StateId::new("start")));
    }

    #[test]
    fn test_find_reachable_states_with_cycles() {
        let workflow = create_workflow_with_self_loops();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let reachable = analyzer.find_reachable_states(&StateId::new("start"));

        assert_eq!(reachable.len(), 3);
        assert!(reachable.contains(&StateId::new("start")));
        assert!(reachable.contains(&StateId::new("loop")));
        assert!(reachable.contains(&StateId::new("end")));
    }

    #[test]
    fn test_find_unreachable_states() {
        let workflow = create_workflow_with_unreachable_states();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let unreachable = analyzer.find_unreachable_states();

        assert_eq!(unreachable.len(), 2);
        assert!(unreachable.contains(&StateId::new("unreachable1")));
        assert!(unreachable.contains(&StateId::new("unreachable2")));
    }

    #[test]
    fn test_find_unreachable_states_none() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let unreachable = analyzer.find_unreachable_states();

        assert_eq!(unreachable.len(), 0);
    }

    #[test]
    fn test_detect_cycle_from_no_cycle() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let cycle = analyzer.detect_cycle_from(&StateId::new("start"));

        assert!(cycle.is_none());
    }

    #[test]
    fn test_detect_cycle_from_with_cycle() {
        let workflow = create_workflow_with_self_loops();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let cycle = analyzer.detect_cycle_from(&StateId::new("start"));

        assert!(cycle.is_some());
        let cycle_path = cycle.unwrap();
        assert!(cycle_path.contains(&StateId::new("loop")));
    }

    #[test]
    fn test_detect_cycle_from_simple_cycle() {
        let mut workflow = create_basic_workflow();
        // Create a simple cycle: start -> end -> start
        workflow.add_transition(create_transition("end", "start", ConditionType::Always));

        let analyzer = WorkflowGraphAnalyzer::new(&workflow);
        let cycle = analyzer.detect_cycle_from(&StateId::new("start"));

        assert!(cycle.is_some());
        let cycle_path = cycle.unwrap();
        assert!(cycle_path.len() >= 2);
        assert!(cycle_path.contains(&StateId::new("start")));
        assert!(cycle_path.contains(&StateId::new("end")));
    }

    #[test]
    fn test_detect_all_cycles_no_cycles() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let cycles = analyzer.detect_all_cycles();

        assert_eq!(cycles.len(), 0);
    }

    #[test]
    fn test_detect_all_cycles_single_cycle() {
        let workflow = create_workflow_with_self_loops();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let cycles = analyzer.detect_all_cycles();

        assert!(!cycles.is_empty());
        // Should find the self-loop cycle
        assert!(cycles
            .iter()
            .any(|cycle| cycle.contains(&StateId::new("loop"))));
    }

    #[test]
    fn test_detect_all_cycles_multiple_cycles() {
        let workflow = create_workflow_with_multiple_cycles();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let cycles = analyzer.detect_all_cycles();

        assert!(cycles.len() >= 2);
        // Should find cycles involving a-b and c-d
        assert!(cycles
            .iter()
            .any(|cycle| cycle.contains(&StateId::new("a")) && cycle.contains(&StateId::new("b"))));
        assert!(cycles
            .iter()
            .any(|cycle| cycle.contains(&StateId::new("c")) && cycle.contains(&StateId::new("d"))));
    }

    #[test]
    fn test_find_paths_single_path() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let paths = analyzer.find_paths(&StateId::new("start"), &StateId::new("end"));

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 2);
        assert_eq!(paths[0][0], StateId::new("start"));
        assert_eq!(paths[0][1], StateId::new("end"));
    }

    #[test]
    fn test_find_paths_multiple_paths() {
        let workflow = create_complex_workflow_for_paths();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let paths = analyzer.find_paths(&StateId::new("start"), &StateId::new("end"));

        assert!(paths.len() >= 2);

        // Should find direct paths: start -> a -> end, start -> b -> end
        assert!(paths
            .iter()
            .any(|path| path.len() == 3 && path[1] == StateId::new("a")));
        assert!(paths
            .iter()
            .any(|path| path.len() == 3 && path[1] == StateId::new("b")));

        // Should find longer paths too
        assert!(paths.iter().any(|path| path.len() >= 4));
    }

    #[test]
    fn test_find_paths_no_path() {
        let workflow = create_workflow_with_unreachable_states();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let paths = analyzer.find_paths(&StateId::new("start"), &StateId::new("unreachable1"));

        assert_eq!(paths.len(), 0);
    }

    #[test]
    fn test_find_paths_same_state() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let paths = analyzer.find_paths(&StateId::new("start"), &StateId::new("start"));

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 1);
        assert_eq!(paths[0][0], StateId::new("start"));
    }

    #[test]
    fn test_build_adjacency_list_basic() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let adjacency = analyzer.build_adjacency_list();

        assert_eq!(adjacency.len(), 2);
        assert!(adjacency.contains_key(&StateId::new("start")));
        assert!(adjacency.contains_key(&StateId::new("end")));

        let start_neighbors = adjacency.get(&StateId::new("start")).unwrap();
        assert_eq!(start_neighbors.len(), 1);
        assert_eq!(start_neighbors[0], StateId::new("end"));

        let end_neighbors = adjacency.get(&StateId::new("end")).unwrap();
        assert_eq!(end_neighbors.len(), 0);
    }

    #[test]
    fn test_build_adjacency_list_complex() {
        let workflow = create_complex_workflow_for_paths();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let adjacency = analyzer.build_adjacency_list();

        assert_eq!(adjacency.len(), 5);

        // Check start state has multiple neighbors
        let start_neighbors = adjacency.get(&StateId::new("start")).unwrap();
        assert!(start_neighbors.len() >= 3);
        assert!(start_neighbors.contains(&StateId::new("a")));
        assert!(start_neighbors.contains(&StateId::new("b")));
        assert!(start_neighbors.contains(&StateId::new("c")));
    }

    #[test]
    fn test_build_adjacency_list_empty() {
        let workflow = Workflow::new(
            WorkflowName::new("empty"),
            "Empty workflow".to_string(),
            StateId::new("start"),
        );
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let adjacency = analyzer.build_adjacency_list();

        assert_eq!(adjacency.len(), 0);
    }

    #[test]
    fn test_topological_sort_acyclic() {
        let workflow = create_basic_workflow();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let sorted = analyzer.topological_sort();

        assert!(sorted.is_some());
        let sorted_states = sorted.unwrap();
        assert_eq!(sorted_states.len(), 2);

        // Start should come before end
        let start_pos = sorted_states
            .iter()
            .position(|s| s == &StateId::new("start"))
            .unwrap();
        let end_pos = sorted_states
            .iter()
            .position(|s| s == &StateId::new("end"))
            .unwrap();
        assert!(start_pos < end_pos);
    }

    #[test]
    fn test_topological_sort_with_cycle() {
        let workflow = create_workflow_with_self_loops();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let sorted = analyzer.topological_sort();

        assert!(sorted.is_none());
    }

    #[test]
    fn test_topological_sort_complex_acyclic() {
        let mut workflow = Workflow::new(
            WorkflowName::new("complex"),
            "Complex workflow".to_string(),
            StateId::new("start"),
        );

        // Create a DAG: start -> a -> c, start -> b -> c, c -> end
        workflow.add_state(create_state("start", "Start", false));
        workflow.add_state(create_state("a", "A", false));
        workflow.add_state(create_state("b", "B", false));
        workflow.add_state(create_state("c", "C", false));
        workflow.add_state(create_state("end", "End", true));

        workflow.add_transition(create_transition("start", "a", ConditionType::Always));
        workflow.add_transition(create_transition("start", "b", ConditionType::Always));
        workflow.add_transition(create_transition("a", "c", ConditionType::Always));
        workflow.add_transition(create_transition("b", "c", ConditionType::Always));
        workflow.add_transition(create_transition("c", "end", ConditionType::Always));

        let analyzer = WorkflowGraphAnalyzer::new(&workflow);
        let sorted = analyzer.topological_sort();

        assert!(sorted.is_some());
        let sorted_states = sorted.unwrap();
        assert_eq!(sorted_states.len(), 5);

        // Check ordering constraints
        let start_pos = sorted_states
            .iter()
            .position(|s| s == &StateId::new("start"))
            .unwrap();
        let a_pos = sorted_states
            .iter()
            .position(|s| s == &StateId::new("a"))
            .unwrap();
        let b_pos = sorted_states
            .iter()
            .position(|s| s == &StateId::new("b"))
            .unwrap();
        let c_pos = sorted_states
            .iter()
            .position(|s| s == &StateId::new("c"))
            .unwrap();
        let end_pos = sorted_states
            .iter()
            .position(|s| s == &StateId::new("end"))
            .unwrap();

        assert!(start_pos < a_pos);
        assert!(start_pos < b_pos);
        assert!(a_pos < c_pos);
        assert!(b_pos < c_pos);
        assert!(c_pos < end_pos);
    }

    #[test]
    fn test_topological_sort_empty_workflow() {
        let workflow = Workflow::new(
            WorkflowName::new("empty"),
            "Empty workflow".to_string(),
            StateId::new("start"),
        );
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        let sorted = analyzer.topological_sort();

        assert!(sorted.is_some());
        let sorted_states = sorted.unwrap();
        assert_eq!(sorted_states.len(), 0);
    }

    #[test]
    fn test_topological_sort_single_state() {
        let mut workflow = Workflow::new(
            WorkflowName::new("single"),
            "Single state".to_string(),
            StateId::new("only"),
        );

        workflow.add_state(create_state("only", "Only state", true));

        let analyzer = WorkflowGraphAnalyzer::new(&workflow);
        let sorted = analyzer.topological_sort();

        assert!(sorted.is_some());
        let sorted_states = sorted.unwrap();
        assert_eq!(sorted_states.len(), 1);
        assert_eq!(sorted_states[0], StateId::new("only"));
    }

    #[test]
    fn test_graph_error_display() {
        let error1 = GraphError::CycleDetected(StateId::new("test"));
        assert!(error1.to_string().contains("cycle"));
        assert!(error1.to_string().contains("test"));

        let error2 = GraphError::StateNotFound(StateId::new("missing"));
        assert!(error2.to_string().contains("not found"));
        assert!(error2.to_string().contains("missing"));
    }

    #[test]
    fn test_analyzer_with_isolated_states() {
        let mut workflow = Workflow::new(
            WorkflowName::new("isolated"),
            "Workflow with isolated states".to_string(),
            StateId::new("start"),
        );

        // Add connected states
        workflow.add_state(create_state("start", "Start", false));
        workflow.add_state(create_state("middle", "Middle", false));
        workflow.add_state(create_state("end", "End", true));

        // Add isolated state (no transitions to/from it)
        workflow.add_state(create_state("isolated", "Isolated", false));

        // Add transitions for connected states
        workflow.add_transition(create_transition("start", "middle", ConditionType::Always));
        workflow.add_transition(create_transition("middle", "end", ConditionType::Always));

        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        // Test reachability
        let reachable = analyzer.find_reachable_states(&StateId::new("start"));
        assert_eq!(reachable.len(), 3);
        assert!(!reachable.contains(&StateId::new("isolated")));

        // Test unreachable states
        let unreachable = analyzer.find_unreachable_states();
        assert_eq!(unreachable.len(), 1);
        assert!(unreachable.contains(&StateId::new("isolated")));

        // Test adjacency list
        let adjacency = analyzer.build_adjacency_list();
        assert_eq!(adjacency.len(), 4);
        let isolated_neighbors = adjacency.get(&StateId::new("isolated")).unwrap();
        assert_eq!(isolated_neighbors.len(), 0);
    }

    #[test]
    fn test_find_paths_with_cycle() {
        let workflow = create_workflow_with_self_loops();
        let analyzer = WorkflowGraphAnalyzer::new(&workflow);

        // Should still find paths even with cycles due to visited tracking
        let paths = analyzer.find_paths(&StateId::new("start"), &StateId::new("end"));

        assert!(!paths.is_empty());
        // Should find path: start -> loop -> end
        assert!(paths.iter().any(|path| path.len() == 3
            && path[0] == StateId::new("start")
            && path[1] == StateId::new("loop")
            && path[2] == StateId::new("end")));
    }

    #[test]
    fn test_adjacency_list_with_multiple_transitions() {
        let mut workflow = Workflow::new(
            WorkflowName::new("multi"),
            "Multiple transitions".to_string(),
            StateId::new("start"),
        );

        workflow.add_state(create_state("start", "Start", false));
        workflow.add_state(create_state("target", "Target", false));

        // Add multiple transitions from start to target
        workflow.add_transition(create_transition("start", "target", ConditionType::Always));
        workflow.add_transition(create_transition(
            "start",
            "target",
            ConditionType::OnSuccess,
        ));

        let analyzer = WorkflowGraphAnalyzer::new(&workflow);
        let adjacency = analyzer.build_adjacency_list();

        // Should have both transitions in adjacency list
        let start_neighbors = adjacency.get(&StateId::new("start")).unwrap();
        assert_eq!(start_neighbors.len(), 2);
        assert!(start_neighbors.iter().all(|n| n == &StateId::new("target")));
    }
}
