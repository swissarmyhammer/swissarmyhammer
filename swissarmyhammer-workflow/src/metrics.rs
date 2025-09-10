//! Workflow execution metrics collection
//!
//! This module provides comprehensive metrics tracking for workflow execution,
//! including timing, success/failure rates, and resource usage statistics.

use crate::{StateId, WorkflowName, WorkflowRunId, WorkflowRunStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Maximum number of data points to keep in resource trends
pub const MAX_TREND_DATA_POINTS: usize = 100;

/// Maximum number of run metrics to keep in memory
pub const MAX_RUN_METRICS: usize = 1000;

/// Maximum number of workflow metrics to keep in memory
pub const MAX_WORKFLOW_METRICS: usize = 100;

/// Maximum number of state durations per run
pub const MAX_STATE_DURATIONS_PER_RUN: usize = 50;

/// Maximum age of completed runs before cleanup (in days)
pub const MAX_COMPLETED_RUN_AGE_DAYS: i64 = 7;

/// Maximum age of workflow summary metrics before cleanup (in days)
pub const MAX_WORKFLOW_SUMMARY_AGE_DAYS: i64 = 30;

/// Metrics collector for workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowMetrics {
    /// Metrics for individual workflow runs
    pub run_metrics: HashMap<WorkflowRunId, RunMetrics>,
    /// Aggregated metrics by workflow name
    pub workflow_metrics: HashMap<WorkflowName, WorkflowSummaryMetrics>,
    /// Global execution statistics
    pub global_metrics: GlobalMetrics,
}

/// Metrics for a single workflow run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    /// Unique run identifier
    pub run_id: WorkflowRunId,
    /// Name of the workflow
    pub workflow_name: WorkflowName,
    /// When the run started
    pub started_at: DateTime<Utc>,
    /// When the run completed (if completed)
    pub completed_at: Option<DateTime<Utc>>,
    /// Final status of the run
    pub status: WorkflowRunStatus,
    /// Total execution duration
    pub total_duration: Option<Duration>,
    /// Per-state execution times
    pub state_durations: HashMap<StateId, Duration>,
    /// Number of state transitions
    pub transition_count: usize,
    /// Memory usage metrics
    pub memory_metrics: MemoryMetrics,
    /// Error details if run failed
    pub error_details: Option<String>,
}

/// Memory usage metrics for a workflow run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Peak memory usage during execution
    pub peak_memory_bytes: u64,
    /// Memory usage at start
    pub initial_memory_bytes: u64,
    /// Memory usage at end
    pub final_memory_bytes: u64,
    /// Number of context variables
    pub context_variables_count: usize,
    /// Size of execution history
    pub history_size: usize,
}

/// Aggregated metrics for a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSummaryMetrics {
    /// Workflow name
    pub workflow_name: WorkflowName,
    /// Total number of runs
    pub total_runs: usize,
    /// Number of successful runs
    pub successful_runs: usize,
    /// Number of failed runs
    pub failed_runs: usize,
    /// Number of cancelled runs
    pub cancelled_runs: usize,
    /// Average execution duration
    pub average_duration: Option<Duration>,
    /// Minimum execution duration
    pub min_duration: Option<Duration>,
    /// Maximum execution duration
    pub max_duration: Option<Duration>,
    /// Average number of transitions
    pub average_transitions: f64,
    /// Most frequently executed states
    pub hot_states: Vec<StateExecutionCount>,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// State execution count for hot state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateExecutionCount {
    /// State identifier
    pub state_id: StateId,
    /// Number of times executed
    pub execution_count: usize,
    /// Total time spent in this state
    pub total_duration: Duration,
    /// Average time per execution
    pub average_duration: Duration,
}

/// Global metrics across all workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMetrics {
    /// Total number of workflow runs
    pub total_runs: usize,
    /// Overall success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Total execution time across all runs
    pub total_execution_time: Duration,
    /// Average execution time across all runs
    pub average_execution_time: Duration,
    /// Number of active workflows
    pub active_workflows: usize,
    /// Number of unique workflows executed
    pub unique_workflows: usize,
    /// System resource usage trends
    pub resource_trends: ResourceTrends,
}

/// Resource usage trends over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTrends {
    /// Memory usage trend (bytes over time)
    pub memory_trend: Vec<(DateTime<Utc>, u64)>,
    /// CPU usage trend (percentage over time)
    pub cpu_trend: Vec<(DateTime<Utc>, f64)>,
    /// Throughput trend (runs per hour)
    pub throughput_trend: Vec<(DateTime<Utc>, f64)>,
}

impl WorkflowMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            run_metrics: HashMap::new(),
            workflow_metrics: HashMap::new(),
            global_metrics: GlobalMetrics::new(),
        }
    }

    /// Start tracking a new workflow run
    pub fn start_run(&mut self, run_id: WorkflowRunId, workflow_name: WorkflowName) {
        // Validate inputs
        if !Self::is_valid_workflow_name(&workflow_name) {
            return;
        }
        let run_metrics = RunMetrics {
            run_id,
            workflow_name: workflow_name.clone(),
            started_at: Utc::now(),
            completed_at: None,
            status: WorkflowRunStatus::Running,
            total_duration: None,
            state_durations: HashMap::new(),
            transition_count: 0,
            memory_metrics: MemoryMetrics::new(),
            error_details: None,
        };

        self.run_metrics.insert(run_id, run_metrics);

        // Enforce bounds checking - remove oldest run metrics if we exceed the limit
        if self.run_metrics.len() > MAX_RUN_METRICS {
            self.cleanup_old_run_metrics();
        }

        self.update_global_metrics();
    }

    /// Record state execution time
    pub fn record_state_execution(
        &mut self,
        run_id: &WorkflowRunId,
        state_id: StateId,
        duration: Duration,
    ) {
        // Validate inputs
        if !Self::is_valid_state_id(&state_id) {
            return;
        }

        if let Some(run_metrics) = self.run_metrics.get_mut(run_id) {
            // Enforce bounds checking - don't allow too many state durations per run
            if run_metrics.state_durations.len() >= MAX_STATE_DURATIONS_PER_RUN {
                return;
            }
            run_metrics.state_durations.insert(state_id, duration);
        }
    }

    /// Record state transition
    pub fn record_transition(&mut self, run_id: &WorkflowRunId) {
        if let Some(run_metrics) = self.run_metrics.get_mut(run_id) {
            run_metrics.transition_count += 1;
        }
    }

    /// Complete a workflow run
    pub fn complete_run(
        &mut self,
        run_id: &WorkflowRunId,
        status: WorkflowRunStatus,
        error_details: Option<String>,
    ) {
        let workflow_name = if let Some(run_metrics) = self.run_metrics.get_mut(run_id) {
            let now = Utc::now();
            run_metrics.completed_at = Some(now);
            run_metrics.status = status;
            run_metrics.error_details = error_details;
            run_metrics.total_duration = Some(
                now.signed_duration_since(run_metrics.started_at)
                    .to_std()
                    .unwrap_or(Duration::ZERO),
            );
            run_metrics.workflow_name.clone()
        } else {
            return;
        };

        // Update workflow summary metrics
        if let Some(run_metrics) = self.run_metrics.get(run_id).cloned() {
            self.update_workflow_summary(&workflow_name, &run_metrics);
        }
        self.update_global_metrics();
    }

    /// Update memory metrics for a run
    pub fn update_memory_metrics(&mut self, run_id: &WorkflowRunId, memory_metrics: MemoryMetrics) {
        if let Some(run_metrics) = self.run_metrics.get_mut(run_id) {
            run_metrics.memory_metrics = memory_metrics;
        }
    }

    /// Get metrics for a specific run
    pub fn get_run_metrics(&self, run_id: &WorkflowRunId) -> Option<&RunMetrics> {
        self.run_metrics.get(run_id)
    }

    /// Get summary metrics for a workflow
    pub fn get_workflow_summary(
        &self,
        workflow_name: &WorkflowName,
    ) -> Option<&WorkflowSummaryMetrics> {
        self.workflow_metrics.get(workflow_name)
    }

    /// Get global metrics
    pub fn get_global_metrics(&self) -> &GlobalMetrics {
        &self.global_metrics
    }

    /// Update workflow summary metrics
    fn update_workflow_summary(&mut self, workflow_name: &WorkflowName, run_metrics: &RunMetrics) {
        // Enforce bounds checking for workflow metrics
        if self.workflow_metrics.len() >= MAX_WORKFLOW_METRICS
            && !self.workflow_metrics.contains_key(workflow_name)
        {
            return; // Skip if we would exceed the limit for a new workflow
        }

        let summary = self
            .workflow_metrics
            .entry(workflow_name.clone())
            .or_insert_with(|| WorkflowSummaryMetrics::new(workflow_name.clone()));

        summary.total_runs += 1;
        match run_metrics.status {
            WorkflowRunStatus::Completed => summary.successful_runs += 1,
            WorkflowRunStatus::Failed => summary.failed_runs += 1,
            WorkflowRunStatus::Cancelled => summary.cancelled_runs += 1,
            _ => {}
        }

        if let Some(duration) = run_metrics.total_duration {
            summary.update_duration_stats(duration);
        }

        summary.average_transitions = (summary.average_transitions
            * (summary.total_runs - 1) as f64
            + run_metrics.transition_count as f64)
            / summary.total_runs as f64;
        summary.update_hot_states(&run_metrics.state_durations);
        summary.last_updated = Utc::now();
    }

    /// Update global metrics
    fn update_global_metrics(&mut self) {
        let total_runs = self.run_metrics.len();
        let successful_runs = self
            .run_metrics
            .values()
            .filter(|r| r.status == WorkflowRunStatus::Completed)
            .count();

        self.global_metrics.total_runs = total_runs;
        self.global_metrics.success_rate = if total_runs > 0 {
            successful_runs as f64 / total_runs as f64
        } else {
            0.0
        };
        self.global_metrics.unique_workflows = self.workflow_metrics.len();
        self.global_metrics.active_workflows = self
            .run_metrics
            .values()
            .filter(|r| r.status == WorkflowRunStatus::Running)
            .count();

        // Calculate total and average execution times
        let completed_runs: Vec<_> = self
            .run_metrics
            .values()
            .filter_map(|r| r.total_duration)
            .collect();
        if !completed_runs.is_empty() {
            self.global_metrics.total_execution_time = completed_runs.iter().sum();
            let total_nanos = completed_runs.iter().map(|d| d.as_nanos()).sum::<u128>();
            let avg_nanos = total_nanos / completed_runs.len() as u128;
            self.global_metrics.average_execution_time = Duration::from_nanos(avg_nanos as u64);
        }
    }

    /// Validate workflow name
    fn is_valid_workflow_name(workflow_name: &WorkflowName) -> bool {
        !workflow_name.as_str().trim().is_empty()
    }

    /// Validate state ID
    fn is_valid_state_id(state_id: &StateId) -> bool {
        !state_id.as_str().trim().is_empty()
    }

    /// Clean up old run metrics when limit is exceeded
    fn cleanup_old_run_metrics(&mut self) {
        // Find the oldest completed runs and remove them
        let mut completed_runs: Vec<_> = self
            .run_metrics
            .iter()
            .filter(|(_, run)| run.completed_at.is_some())
            .map(|(id, run)| (*id, run.completed_at.unwrap()))
            .collect();

        // Sort by completion time (oldest first)
        completed_runs.sort_by_key(|(_, completed_at)| *completed_at);

        // Remove the oldest runs to get back under the limit
        let excess_count = self.run_metrics.len().saturating_sub(MAX_RUN_METRICS);
        completed_runs
            .into_iter()
            .take(excess_count)
            .for_each(|(run_id, _)| {
                self.run_metrics.remove(&run_id);
            });
    }

    /// Comprehensive cleanup of old metrics data
    pub fn cleanup_old_metrics(&mut self) {
        let now = Utc::now();
        let mut removed_runs = 0;
        let mut removed_workflows = 0;

        // Clean up old completed runs
        let cutoff_date = now - chrono::Duration::days(MAX_COMPLETED_RUN_AGE_DAYS);
        let runs_to_remove: Vec<_> = self
            .run_metrics
            .iter()
            .filter(|(_, run)| {
                if let Some(completed_at) = run.completed_at {
                    completed_at < cutoff_date
                } else {
                    false
                }
            })
            .map(|(id, _)| *id)
            .collect();

        for run_id in runs_to_remove {
            self.run_metrics.remove(&run_id);
            removed_runs += 1;
        }

        // Clean up old workflow summary metrics
        let workflow_cutoff_date = now - chrono::Duration::days(MAX_WORKFLOW_SUMMARY_AGE_DAYS);
        let workflows_to_remove: Vec<_> = self
            .workflow_metrics
            .iter()
            .filter(|(_, summary)| summary.last_updated < workflow_cutoff_date)
            .map(|(name, _)| name.clone())
            .collect();

        for workflow_name in workflows_to_remove {
            self.workflow_metrics.remove(&workflow_name);
            removed_workflows += 1;
        }

        // Update global metrics after cleanup
        self.update_global_metrics();

        if removed_runs > 0 || removed_workflows > 0 {
            tracing::info!(
                "Metrics cleanup completed: removed {} old runs and {} old workflow summaries",
                removed_runs,
                removed_workflows
            );
        }
    }
}

impl MemoryMetrics {
    /// Create new memory metrics
    pub fn new() -> Self {
        Self {
            peak_memory_bytes: 0,
            initial_memory_bytes: 0,
            final_memory_bytes: 0,
            context_variables_count: 0,
            history_size: 0,
        }
    }

    /// Update memory metrics
    pub fn update(&mut self, current_memory: u64, context_vars: usize, history_size: usize) {
        if current_memory > self.peak_memory_bytes {
            self.peak_memory_bytes = current_memory;
        }
        self.context_variables_count = context_vars;
        self.history_size = history_size;
    }
}

impl WorkflowSummaryMetrics {
    /// Create new workflow summary metrics
    pub fn new(workflow_name: WorkflowName) -> Self {
        Self {
            workflow_name,
            total_runs: 0,
            successful_runs: 0,
            failed_runs: 0,
            cancelled_runs: 0,
            average_duration: None,
            min_duration: None,
            max_duration: None,
            average_transitions: 0.0,
            hot_states: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    /// Update duration statistics
    fn update_duration_stats(&mut self, duration: Duration) {
        if let Some(avg) = self.average_duration {
            let total_nanos = avg.as_nanos() * (self.total_runs - 1) as u128 + duration.as_nanos();
            let avg_nanos = total_nanos / self.total_runs as u128;
            self.average_duration = Some(Duration::from_nanos(avg_nanos as u64));
        } else {
            self.average_duration = Some(duration);
        }

        if let Some(min) = self.min_duration {
            if duration < min {
                self.min_duration = Some(duration);
            }
        } else {
            self.min_duration = Some(duration);
        }

        if let Some(max) = self.max_duration {
            if duration > max {
                self.max_duration = Some(duration);
            }
        } else {
            self.max_duration = Some(duration);
        }
    }

    /// Update hot states tracking
    fn update_hot_states(&mut self, state_durations: &HashMap<StateId, Duration>) {
        for (state_id, duration) in state_durations {
            if let Some(state_count) = self.hot_states.iter_mut().find(|s| s.state_id == *state_id)
            {
                state_count.execution_count += 1;
                state_count.total_duration += *duration;
                let avg_nanos =
                    state_count.total_duration.as_nanos() / state_count.execution_count as u128;
                state_count.average_duration = Duration::from_nanos(avg_nanos as u64);
            } else {
                self.hot_states.push(StateExecutionCount {
                    state_id: state_id.clone(),
                    execution_count: 1,
                    total_duration: *duration,
                    average_duration: *duration,
                });
            }
        }

        // Sort by execution count (descending) and keep top 10
        self.hot_states
            .sort_by(|a, b| b.execution_count.cmp(&a.execution_count));
        self.hot_states.truncate(10);
    }

    /// Get success rate for this workflow
    pub fn success_rate(&self) -> f64 {
        if self.total_runs > 0 {
            self.successful_runs as f64 / self.total_runs as f64
        } else {
            0.0
        }
    }
}

impl GlobalMetrics {
    /// Create new global metrics
    pub fn new() -> Self {
        Self {
            total_runs: 0,
            success_rate: 0.0,
            total_execution_time: Duration::ZERO,
            average_execution_time: Duration::ZERO,
            active_workflows: 0,
            unique_workflows: 0,
            resource_trends: ResourceTrends::new(),
        }
    }
}

impl Default for GlobalMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceTrends {
    /// Create new resource trends
    pub fn new() -> Self {
        Self {
            memory_trend: Vec::new(),
            cpu_trend: Vec::new(),
            throughput_trend: Vec::new(),
        }
    }

    /// Add memory usage data point
    pub fn add_memory_point(&mut self, memory_bytes: u64) {
        self.memory_trend.push((Utc::now(), memory_bytes));
        // Keep only last MAX_TREND_DATA_POINTS data points
        if self.memory_trend.len() > MAX_TREND_DATA_POINTS {
            self.memory_trend.remove(0);
        }
    }

    /// Add CPU usage data point
    pub fn add_cpu_point(&mut self, cpu_percentage: f64) {
        self.cpu_trend.push((Utc::now(), cpu_percentage));
        // Keep only last MAX_TREND_DATA_POINTS data points
        if self.cpu_trend.len() > MAX_TREND_DATA_POINTS {
            self.cpu_trend.remove(0);
        }
    }

    /// Add throughput data point
    pub fn add_throughput_point(&mut self, runs_per_hour: f64) {
        self.throughput_trend.push((Utc::now(), runs_per_hour));
        // Keep only last MAX_TREND_DATA_POINTS data points
        if self.throughput_trend.len() > MAX_TREND_DATA_POINTS {
            self.throughput_trend.remove(0);
        }
    }
}

impl Default for WorkflowMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MemoryMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ResourceTrends {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StateId, WorkflowName, WorkflowRunId, WorkflowRunStatus};
    use std::time::Duration;

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

        metrics.start_run(run_id, workflow_name.clone());

        assert_eq!(metrics.run_metrics.len(), 1);
        assert!(metrics.run_metrics.contains_key(&run_id));

        let run_metrics = metrics
            .run_metrics
            .get(&run_id)
            .expect("Run metrics should exist after start_run");
        assert_eq!(run_metrics.workflow_name, workflow_name);
        assert_eq!(run_metrics.status, WorkflowRunStatus::Running);
        assert_eq!(run_metrics.transition_count, 0);
    }

    #[test]
    fn test_record_state_execution() {
        let mut metrics = WorkflowMetrics::new();
        let run_id = WorkflowRunId::new();
        let workflow_name = WorkflowName::new("test_workflow");

        metrics.start_run(run_id, workflow_name);

        let state_id = StateId::new("test_state");
        let duration = Duration::from_secs(2);

        metrics.record_state_execution(&run_id, state_id.clone(), duration);

        let run_metrics = metrics
            .run_metrics
            .get(&run_id)
            .expect("Run metrics should exist after start_run");
        assert_eq!(run_metrics.state_durations.get(&state_id), Some(&duration));
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
}
