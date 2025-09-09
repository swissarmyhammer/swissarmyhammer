//! Tests for workflow execution visualization
//!
//! This module contains comprehensive tests for the workflow visualization functionality,
//! including execution trace generation, Mermaid diagram generation, HTML output,
//! and security features.

use crate::visualization::{
    MAX_EXECUTION_STEPS, MAX_PATH_LENGTH_FULL, MAX_PATH_LENGTH_MINIMAL,
};
use crate::{
    test_helpers::*, visualization::*, ConditionType, MemoryMetrics, RunMetrics, StateId,
    WorkflowName, WorkflowRun, WorkflowRunId, WorkflowRunStatus,
};
use chrono::Utc;
use std::collections::HashMap;
use std::time::Duration;

/// Helper function to create a sample workflow run
fn create_sample_workflow_run() -> WorkflowRun {
    let mut workflow = create_basic_workflow();

    // Add a processing state
    workflow.add_state(create_state("process", "Processing state", false));
    workflow.add_transition(create_transition("start", "process", ConditionType::Always));
    workflow.add_transition(create_transition("process", "end", ConditionType::Always));

    let mut run = WorkflowRun::new(workflow);

    // Simulate some execution history
    run.transition_to(StateId::new("process"));
    run.transition_to(StateId::new("end"));
    run.status = WorkflowRunStatus::Completed;
    run.completed_at = Some(Utc::now());

    run
}

/// Helper function to create sample run metrics
fn create_sample_run_metrics() -> RunMetrics {
    let mut state_durations = HashMap::new();
    state_durations.insert(StateId::new("start"), Duration::from_millis(100));
    state_durations.insert(StateId::new("process"), Duration::from_millis(500));
    state_durations.insert(StateId::new("end"), Duration::from_millis(50));

    RunMetrics {
        run_id: WorkflowRunId::new(),
        workflow_name: WorkflowName::new("Test Workflow"),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        status: WorkflowRunStatus::Completed,
        total_duration: Some(Duration::from_millis(650)),
        state_durations,
        transition_count: 2,
        memory_metrics: MemoryMetrics {
            peak_memory_bytes: 1024 * 1024,
            initial_memory_bytes: 512 * 1024,
            final_memory_bytes: 600 * 1024,
            context_variables_count: 5,
            history_size: 3,
        },
        error_details: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_visualizer_creation() {
        let visualizer = ExecutionVisualizer::new();
        assert!(visualizer.include_timing);
        assert!(visualizer.include_counts);
        assert!(visualizer.include_status);
        assert_eq!(visualizer.max_path_length, MAX_PATH_LENGTH_FULL);
    }

    #[test]
    fn test_execution_visualizer_minimal() {
        let visualizer = ExecutionVisualizer::minimal();
        assert!(!visualizer.include_timing);
        assert!(!visualizer.include_counts);
        assert!(visualizer.include_status);
        assert_eq!(visualizer.max_path_length, MAX_PATH_LENGTH_MINIMAL);
    }

    #[test]
    fn test_execution_visualizer_default() {
        let visualizer = ExecutionVisualizer::default();
        assert!(visualizer.include_timing);
        assert!(visualizer.include_counts);
        assert!(visualizer.include_status);
        assert_eq!(visualizer.max_path_length, MAX_PATH_LENGTH_FULL);
    }

    #[test]
    fn test_generate_trace_basic() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();

        let trace = visualizer.generate_trace(&run);

        assert_eq!(trace.run_id, run.id.to_string());
        assert_eq!(trace.workflow_name, run.workflow.name.to_string());
        assert_eq!(trace.status, WorkflowRunStatus::Completed);
        assert_eq!(trace.execution_path.len(), 3); // start, process, end
        assert!(trace.total_duration.is_some());
        assert!(trace.completed_at.is_some());
    }

    #[test]
    fn test_generate_trace_with_metrics() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let metrics = create_sample_run_metrics();

        let trace = visualizer.generate_trace_with_metrics(&run, &metrics);

        assert_eq!(trace.run_id, run.id.to_string());
        assert_eq!(trace.workflow_name, run.workflow.name.to_string());
        assert_eq!(trace.status, WorkflowRunStatus::Completed);
        assert_eq!(trace.execution_path.len(), 3);

        // Check that metrics are applied
        assert_eq!(trace.total_duration, metrics.total_duration);
        assert_eq!(trace.error_details, metrics.error_details);

        // Check that step durations are applied
        for step in &trace.execution_path {
            if let Some(expected_duration) = metrics.state_durations.get(&step.state_id) {
                assert_eq!(step.duration, Some(*expected_duration));
            }
        }
    }

    #[test]
    fn test_execution_step_creation() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();

        let trace = visualizer.generate_trace(&run);

        // Check first step
        let first_step = &trace.execution_path[0];
        assert_eq!(first_step.state_id, StateId::new("start"));
        assert_eq!(first_step.state_description, "Start state");
        assert!(first_step.success);
        assert!(first_step.error.is_none());
        assert_eq!(first_step.transition_taken, Some(StateId::new("process")));

        // Check last step
        let last_step = &trace.execution_path[2];
        assert_eq!(last_step.state_id, StateId::new("end"));
        assert_eq!(last_step.state_description, "End state");
        assert!(last_step.success);
        assert!(last_step.error.is_none());
        assert!(last_step.transition_taken.is_none()); // No transition from end state
    }

    #[test]
    fn test_generate_mermaid_with_execution() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let trace = visualizer.generate_trace(&run);

        let mermaid = visualizer.generate_mermaid_with_execution(&run.workflow, &trace);

        // Check basic structure
        assert!(mermaid.contains("stateDiagram-v2"));
        assert!(mermaid.contains("title: Test Workflow - Execution Trace"));

        // Check that states are included
        assert!(mermaid.contains("start: ✓Start state"));
        assert!(mermaid.contains("process: ✓Processing state"));
        assert!(mermaid.contains("end: ✓End state"));

        // Check transitions
        assert!(mermaid.contains("start --> process"));
        assert!(mermaid.contains("process --> end"));

        // Check execution path annotations
        assert!(mermaid.contains("Step 1"));
        assert!(mermaid.contains("Step 2"));
        assert!(mermaid.contains("Step 3"));
    }

    #[test]
    fn test_generate_mermaid_with_timing() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let metrics = create_sample_run_metrics();
        let trace = visualizer.generate_trace_with_metrics(&run, &metrics);

        let mermaid = visualizer.generate_mermaid_with_execution(&run.workflow, &trace);

        // Check that timing information is included
        assert!(mermaid.contains("100ms") || mermaid.contains("0.1s"));
        assert!(mermaid.contains("500ms") || mermaid.contains("0.5s"));
        assert!(mermaid.contains("50ms") || mermaid.contains("0.05s"));
    }

    #[test]
    fn test_generate_mermaid_without_timing() {
        let visualizer = ExecutionVisualizer::minimal();
        let run = create_sample_workflow_run();
        let trace = visualizer.generate_trace(&run);

        let mermaid = visualizer.generate_mermaid_with_execution(&run.workflow, &trace);

        // Check that timing information is NOT included
        assert!(!mermaid.contains("ms"));
        assert!(!mermaid.contains("0.1s"));
        assert!(!mermaid.contains("0.5s"));
        assert!(!mermaid.contains("Duration"));
        // Should still have step annotations
        assert!(mermaid.contains("Step 1"));
        assert!(mermaid.contains("Step 2"));
    }

    #[test]
    fn test_generate_html_basic() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let trace = visualizer.generate_trace(&run);

        let html = visualizer.generate_html(&run.workflow, &trace);

        // Check basic HTML structure
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>Workflow Execution Trace: Test Workflow</title>"));
        assert!(html.contains("<h1>Workflow Execution Trace</h1>"));
        assert!(html.contains("mermaid.min.js"));
        assert!(html.contains("class=\"mermaid\""));

        // Check execution info
        assert!(html.contains("Run ID:"));
        assert!(html.contains("Status:"));
        assert!(html.contains("Duration:"));
        assert!(html.contains("Started:"));
        assert!(html.contains("Completed:"));
    }

    #[test]
    fn test_generate_html_xss_prevention() {
        let visualizer = ExecutionVisualizer::new();
        let mut run = create_sample_workflow_run();

        // Inject potential XSS content
        run.workflow.name = WorkflowName::new("<script>alert('xss')</script>");

        let trace = visualizer.generate_trace(&run);
        let html = visualizer.generate_html(&run.workflow, &trace);

        // Check that dangerous content is escaped or not present
        assert!(!html.contains("<script>alert('xss')</script>"));
        // The HTML should contain the escaped version or be safe
        assert!(html.contains("&lt;script&gt;") || !html.contains("script>alert"));
    }

    #[test]
    fn test_generate_html_dos_protection() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();

        // Create a trace with too many execution steps
        let mut execution_path = Vec::new();
        for i in 0..MAX_EXECUTION_STEPS + 1 {
            execution_path.push(ExecutionStep {
                state_id: StateId::new(format!("state_{i}")),
                state_description: format!("State {i}"),
                duration: Some(Duration::from_millis(100)),
                timestamp: Utc::now(),
                success: true,
                error: None,
                transition_taken: None,
            });
        }

        let trace = ExecutionTrace {
            run_id: run.id.to_string(),
            workflow_name: run.workflow.name.to_string(),
            execution_path,
            status: WorkflowRunStatus::Completed,
            total_duration: Some(Duration::from_secs(10)),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            error_details: None,
        };

        let html = visualizer.generate_html(&run.workflow, &trace);

        // Check that DoS protection is triggered
        assert!(html.contains("Error: Execution trace too large"));
        assert!(html.contains(&format!("maximum allowed is {MAX_EXECUTION_STEPS}")));
    }

    #[test]
    fn test_html_escape_integration() {
        let visualizer = ExecutionVisualizer::new();
        let mut run = create_sample_workflow_run();

        // Test XSS prevention through HTML generation
        run.workflow.name = WorkflowName::new("<script>alert('test')</script>");

        let trace = visualizer.generate_trace(&run);
        let html = visualizer.generate_html(&run.workflow, &trace);

        // Check that dangerous content is not present in executable form
        assert!(!html.contains("<script>alert('test')</script>"));
        // The HTML should be safe from XSS
        assert!(html.contains("&lt;script&gt;") || !html.contains("script>alert"));
    }

    #[test]
    fn test_export_trace_json() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let trace = visualizer.generate_trace(&run);

        let json = visualizer.export_trace_json(&trace).unwrap();

        // Check that JSON contains expected fields
        assert!(json.contains("\"run_id\""));
        assert!(json.contains("\"workflow_name\""));
        assert!(json.contains("\"execution_path\""));
        assert!(json.contains("\"status\""));
        assert!(json.contains("\"total_duration\""));
    }

    #[test]
    fn test_generate_execution_report() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let metrics = create_sample_run_metrics();
        let trace = visualizer.generate_trace_with_metrics(&run, &metrics);

        let report = visualizer.generate_execution_report(&trace);

        // Check report structure
        assert!(report.contains("# Execution Report: Test Workflow"));
        assert!(report.contains("**Run ID:**"));
        assert!(report.contains("**Status:**"));
        assert!(report.contains("**Started:**"));
        assert!(report.contains("**Completed:**"));
        assert!(report.contains("**Total Duration:**"));
        assert!(report.contains("## Execution Path"));

        // Check execution steps
        assert!(report.contains("1. ✓ start - Start state"));
        assert!(report.contains("2. ✓ process - Processing state"));
        assert!(report.contains("3. ✓ end - End state"));
    }

    #[test]
    fn test_generate_execution_report_with_errors() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let mut metrics = create_sample_run_metrics();
        metrics.error_details = Some("Test error occurred".to_string());

        let trace = visualizer.generate_trace_with_metrics(&run, &metrics);
        let report = visualizer.generate_execution_report(&trace);

        // Check error details section
        assert!(report.contains("## Error Details"));
        assert!(report.contains("Test error occurred"));
    }

    #[test]
    fn test_visualization_format_display() {
        assert_eq!(format!("{}", VisualizationFormat::Mermaid), "mermaid");
        assert_eq!(format!("{}", VisualizationFormat::Dot), "dot");
        assert_eq!(format!("{}", VisualizationFormat::Json), "json");
        assert_eq!(format!("{}", VisualizationFormat::Html), "html");
    }

    #[test]
    fn test_visualization_options_default() {
        let options = VisualizationOptions::default();
        assert!(options.title.is_none());
        assert!(options.show_timing);
        assert!(options.show_counts);
        assert!(!options.show_path_only);
        assert!(options.max_states.is_none());

        // Check color scheme defaults
        assert_eq!(options.color_scheme.success_color, "#90EE90");
        assert_eq!(options.color_scheme.error_color, "#FFB6C1");
        assert_eq!(options.color_scheme.active_color, "#87CEEB");
        assert_eq!(options.color_scheme.unvisited_color, "#F0F0F0");
        assert_eq!(options.color_scheme.transition_color, "#696969");
    }

    #[test]
    fn test_color_scheme_default() {
        let color_scheme = ColorScheme::default();
        assert_eq!(color_scheme.success_color, "#90EE90");
        assert_eq!(color_scheme.error_color, "#FFB6C1");
        assert_eq!(color_scheme.active_color, "#87CEEB");
        assert_eq!(color_scheme.unvisited_color, "#F0F0F0");
        assert_eq!(color_scheme.transition_color, "#696969");
    }

    #[test]
    fn test_execution_trace_serialization() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let trace = visualizer.generate_trace(&run);

        // Test JSON serialization
        let json = serde_json::to_string(&trace).unwrap();
        let deserialized: ExecutionTrace = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.run_id, trace.run_id);
        assert_eq!(deserialized.workflow_name, trace.workflow_name);
        assert_eq!(deserialized.status, trace.status);
        assert_eq!(
            deserialized.execution_path.len(),
            trace.execution_path.len()
        );
    }

    #[test]
    fn test_execution_step_serialization() {
        let step = ExecutionStep {
            state_id: StateId::new("test"),
            state_description: "Test state".to_string(),
            duration: Some(Duration::from_millis(100)),
            timestamp: Utc::now(),
            success: true,
            error: None,
            transition_taken: Some(StateId::new("next")),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ExecutionStep = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.state_id, step.state_id);
        assert_eq!(deserialized.state_description, step.state_description);
        assert_eq!(deserialized.duration, step.duration);
        assert_eq!(deserialized.success, step.success);
        assert_eq!(deserialized.error, step.error);
        assert_eq!(deserialized.transition_taken, step.transition_taken);
    }

    #[test]
    fn test_generate_trace_empty_workflow() {
        let visualizer = ExecutionVisualizer::new();
        let workflow = create_basic_workflow();
        let run = WorkflowRun::new(workflow);

        let trace = visualizer.generate_trace(&run);

        assert_eq!(trace.execution_path.len(), 1); // Just the initial state
        assert_eq!(trace.execution_path[0].state_id, StateId::new("start"));
        assert_eq!(trace.status, WorkflowRunStatus::Running);
        assert!(trace.completed_at.is_none());
    }

    #[test]
    fn test_generate_mermaid_empty_execution() {
        let visualizer = ExecutionVisualizer::new();
        let workflow = create_basic_workflow();
        let run = WorkflowRun::new(workflow.clone());
        let trace = visualizer.generate_trace(&run);

        let mermaid = visualizer.generate_mermaid_with_execution(&workflow, &trace);

        assert!(mermaid.contains("stateDiagram-v2"));
        assert!(mermaid.contains("start: ✓Start state"));
        assert!(mermaid.contains("end: End state")); // Not executed
    }

    #[test]
    fn test_constants_are_reasonable() {
        // Verify constants are properly exposed and have expected values
        // These assertions document the expected values of public constants
        assert_eq!(MAX_PATH_LENGTH_FULL, 1000);
        assert_eq!(MAX_PATH_LENGTH_MINIMAL, 100);
        assert_eq!(MAX_EXECUTION_STEPS, 500);
    }

    #[test]
    fn test_mermaid_state_and_transition_formatting() {
        let visualizer = ExecutionVisualizer::new();
        let run = create_sample_workflow_run();
        let trace = visualizer.generate_trace(&run);

        let mermaid = visualizer.generate_mermaid_with_execution(&run.workflow, &trace);

        // Test that executed states have checkmarks
        assert!(mermaid.contains("start: ✓Start state"));
        assert!(mermaid.contains("process: ✓Processing state"));
        assert!(mermaid.contains("end: ✓End state"));

        // Test that transitions show execution status
        assert!(mermaid.contains("start --> process"));
        assert!(mermaid.contains("process --> end"));
        assert!(mermaid.contains("(taken)"));
    }

    #[test]
    fn test_mermaid_unexecuted_states() {
        let visualizer = ExecutionVisualizer::new();
        let mut workflow = create_basic_workflow();

        // Add an unexecuted state
        workflow.add_state(create_state("unused", "Unused state", false));

        let run = WorkflowRun::new(workflow.clone());
        let trace = visualizer.generate_trace(&run);

        let mermaid = visualizer.generate_mermaid_with_execution(&workflow, &trace);

        // Check that unexecuted states don't have checkmarks
        assert!(mermaid.contains("unused: Unused state"));
        assert!(!mermaid.contains("unused: ✓Unused state"));
    }
}
