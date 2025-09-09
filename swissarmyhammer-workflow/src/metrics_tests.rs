//! Tests for workflow metrics collection

use super::metrics::*;
use crate::workflow::{StateId, WorkflowName, WorkflowRunId, WorkflowRunStatus};
use chrono::Utc;
use std::time::Duration;

// Import the constant from the parent module
use super::metrics::MAX_TREND_DATA_POINTS;

#[test]
fn test_workflow_metrics_new() {
    let metrics = WorkflowMetrics::new();
    
    assert_eq!(metrics.run_metrics.len(), 0);
    assert_eq!(metrics.workflow_metrics.len(), 0);
    assert_eq!(metrics.global_metrics.total_runs, 0);
    assert_eq!(metrics.global_metrics.success_rate, 0.0);
}

#[test]
fn test_start_run() {
    let mut metrics = WorkflowMetrics::new();
    let run_id = WorkflowRunId::new();
    let workflow_name = WorkflowName::new("test_workflow");
    
    metrics.start_run(run_id.clone(), workflow_name.clone());
    
    assert_eq!(metrics.run_metrics.len(), 1);
    assert!(metrics.run_metrics.contains_key(&run_id));
    
    let run_metrics = metrics.run_metrics.get(&run_id).unwrap();
    assert_eq!(run_metrics.workflow_name, workflow_name);
    assert_eq!(run_metrics.status, WorkflowRunStatus::Running);
    assert_eq!(run_metrics.transition_count, 0);
}

#[test]
fn test_record_state_execution() {
    let mut metrics = WorkflowMetrics::new();
    let run_id = WorkflowRunId::new();
    let workflow_name = WorkflowName::new("test_workflow");
    
    metrics.start_run(run_id.clone(), workflow_name);
    
    let state_id = StateId::new("test_state");
    let duration = Duration::from_secs(2);
    
    metrics.record_state_execution(&run_id, state_id.clone(), duration);
    
    let run_metrics = metrics.run_metrics.get(&run_id).unwrap();
    assert_eq!(run_metrics.state_durations.get(&state_id), Some(&duration));
}

#[test]
fn test_record_transition() {
    let mut metrics = WorkflowMetrics::new();
    let run_id = WorkflowRunId::new();
    let workflow_name = WorkflowName::new("test_workflow");
    
    metrics.start_run(run_id.clone(), workflow_name);
    
    metrics.record_transition(&run_id);
    metrics.record_transition(&run_id);
    
    let run_metrics = metrics.run_metrics.get(&run_id).unwrap();
    assert_eq!(run_metrics.transition_count, 2);
}

#[test]
fn test_complete_run() {
    let mut metrics = WorkflowMetrics::new();
    let run_id = WorkflowRunId::new();
    let workflow_name = WorkflowName::new("test_workflow");
    
    metrics.start_run(run_id.clone(), workflow_name.clone());
    
    metrics.complete_run(&run_id, WorkflowRunStatus::Completed, None);
    
    let run_metrics = metrics.run_metrics.get(&run_id).unwrap();
    assert_eq!(run_metrics.status, WorkflowRunStatus::Completed);
    assert!(run_metrics.completed_at.is_some());
    assert!(run_metrics.total_duration.is_some());
    
    // Check that workflow summary was updated
    let workflow_summary = metrics.workflow_metrics.get(&workflow_name).unwrap();
    assert_eq!(workflow_summary.total_runs, 1);
    assert_eq!(workflow_summary.successful_runs, 1);
    assert_eq!(workflow_summary.failed_runs, 0);
}

#[test]
fn test_complete_run_with_error() {
    let mut metrics = WorkflowMetrics::new();
    let run_id = WorkflowRunId::new();
    let workflow_name = WorkflowName::new("test_workflow");
    
    metrics.start_run(run_id.clone(), workflow_name.clone());
    
    let error_message = "Test error".to_string();
    metrics.complete_run(&run_id, WorkflowRunStatus::Failed, Some(error_message.clone()));
    
    let run_metrics = metrics.run_metrics.get(&run_id).unwrap();
    assert_eq!(run_metrics.status, WorkflowRunStatus::Failed);
    assert_eq!(run_metrics.error_details, Some(error_message));
    
    // Check that workflow summary was updated
    let workflow_summary = metrics.workflow_metrics.get(&workflow_name).unwrap();
    assert_eq!(workflow_summary.total_runs, 1);
    assert_eq!(workflow_summary.successful_runs, 0);
    assert_eq!(workflow_summary.failed_runs, 1);
}

#[test]
fn test_workflow_summary_metrics() {
    let mut metrics = WorkflowMetrics::new();
    let workflow_name = WorkflowName::new("test_workflow");
    
    // Run multiple workflows to test summary calculations
    for i in 0..5 {
        let run_id = WorkflowRunId::new();
        metrics.start_run(run_id.clone(), workflow_name.clone());
        
        // Add some state executions
        for j in 0..3 {
            let state_id = StateId::new(format!("state_{}", j));
            let duration = Duration::from_millis(100 * (i + 1) as u64);
            metrics.record_state_execution(&run_id, state_id, duration);
        }
        
        // Add transitions
        for _ in 0..2 {
            metrics.record_transition(&run_id);
        }
        
        // Complete with success or failure
        let status = if i % 2 == 0 {
            WorkflowRunStatus::Completed
        } else {
            WorkflowRunStatus::Failed
        };
        metrics.complete_run(&run_id, status, None);
    }
    
    let workflow_summary = metrics.workflow_metrics.get(&workflow_name).unwrap();
    assert_eq!(workflow_summary.total_runs, 5);
    assert_eq!(workflow_summary.successful_runs, 3); // 0, 2, 4
    assert_eq!(workflow_summary.failed_runs, 2); // 1, 3
    assert_eq!(workflow_summary.success_rate(), 0.6); // 3/5
    assert_eq!(workflow_summary.average_transitions, 2.0);
    assert!(workflow_summary.average_duration.is_some());
}

#[test]
fn test_global_metrics() {
    let mut metrics = WorkflowMetrics::new();
    
    // Create multiple workflows
    let workflow1 = WorkflowName::new("workflow1");
    let workflow2 = WorkflowName::new("workflow2");
    
    // Run workflow1 twice (1 success, 1 failure)
    for i in 0..2 {
        let run_id = WorkflowRunId::new();
        metrics.start_run(run_id.clone(), workflow1.clone());
        
        let status = if i == 0 {
            WorkflowRunStatus::Completed
        } else {
            WorkflowRunStatus::Failed
        };
        metrics.complete_run(&run_id, status, None);
    }
    
    // Run workflow2 once (success)
    let run_id = WorkflowRunId::new();
    metrics.start_run(run_id.clone(), workflow2.clone());
    metrics.complete_run(&run_id, WorkflowRunStatus::Completed, None);
    
    let global_metrics = metrics.get_global_metrics();
    assert_eq!(global_metrics.total_runs, 3);
    assert_eq!(global_metrics.success_rate, 2.0 / 3.0); // 2 successes out of 3
    assert_eq!(global_metrics.unique_workflows, 2);
    assert_eq!(global_metrics.active_workflows, 0); // All completed
}

#[test]
fn test_memory_metrics() {
    let mut memory_metrics = MemoryMetrics::new();
    
    assert_eq!(memory_metrics.peak_memory_bytes, 0);
    assert_eq!(memory_metrics.context_variables_count, 0);
    assert_eq!(memory_metrics.history_size, 0);
    
    // Update memory metrics
    memory_metrics.update(1024, 5, 10);
    assert_eq!(memory_metrics.peak_memory_bytes, 1024);
    assert_eq!(memory_metrics.context_variables_count, 5);
    assert_eq!(memory_metrics.history_size, 10);
    
    // Update with higher memory - should update peak
    memory_metrics.update(2048, 8, 15);
    assert_eq!(memory_metrics.peak_memory_bytes, 2048);
    assert_eq!(memory_metrics.context_variables_count, 8);
    assert_eq!(memory_metrics.history_size, 15);
    
    // Update with lower memory - should not update peak
    memory_metrics.update(512, 3, 5);
    assert_eq!(memory_metrics.peak_memory_bytes, 2048); // Still the peak
    assert_eq!(memory_metrics.context_variables_count, 3);
    assert_eq!(memory_metrics.history_size, 5);
}

#[test]
fn test_resource_trends() {
    let mut trends = ResourceTrends::new();
    
    // Add some data points
    trends.add_memory_point(1024);
    trends.add_cpu_point(25.5);
    trends.add_throughput_point(10.0);
    
    assert_eq!(trends.memory_trend.len(), 1);
    assert_eq!(trends.cpu_trend.len(), 1);
    assert_eq!(trends.throughput_trend.len(), 1);
    
    assert_eq!(trends.memory_trend[0].1, 1024);
    assert_eq!(trends.cpu_trend[0].1, 25.5);
    assert_eq!(trends.throughput_trend[0].1, 10.0);
    
    // Add many data points to test truncation
    for i in 0..150 {
        trends.add_memory_point(i * 100);
        trends.add_cpu_point(i as f64);
        trends.add_throughput_point(i as f64 / 10.0);
    }
    
    // Should be truncated to MAX_TREND_DATA_POINTS points
    assert_eq!(trends.memory_trend.len(), MAX_TREND_DATA_POINTS);
    assert_eq!(trends.cpu_trend.len(), MAX_TREND_DATA_POINTS);
    assert_eq!(trends.throughput_trend.len(), MAX_TREND_DATA_POINTS);
}

#[test]
fn test_hot_states_tracking() {
    let mut summary = WorkflowSummaryMetrics::new(WorkflowName::new("test_workflow"));
    
    // Simulate state executions
    let mut state_durations = std::collections::HashMap::new();
    state_durations.insert(StateId::new("state1"), Duration::from_millis(100));
    state_durations.insert(StateId::new("state2"), Duration::from_millis(200));
    
    // First execution
    summary.update_hot_states(&state_durations);
    
    assert_eq!(summary.hot_states.len(), 2);
    assert_eq!(summary.hot_states[0].execution_count, 1);
    assert_eq!(summary.hot_states[1].execution_count, 1);
    
    // Second execution - state1 only
    let mut state_durations2 = std::collections::HashMap::new();
    state_durations2.insert(StateId::new("state1"), Duration::from_millis(150));
    
    summary.update_hot_states(&state_durations2);
    
    // state1 should now have 2 executions and be first (sorted by count)
    assert_eq!(summary.hot_states.len(), 2);
    let state1_count = summary.hot_states.iter().find(|s| s.state_id == StateId::new("state1")).unwrap();
    assert_eq!(state1_count.execution_count, 2);
    assert_eq!(state1_count.total_duration, Duration::from_millis(250));
    assert_eq!(state1_count.average_duration, Duration::from_millis(125));
    
    let state2_count = summary.hot_states.iter().find(|s| s.state_id == StateId::new("state2")).unwrap();
    assert_eq!(state2_count.execution_count, 1);
}