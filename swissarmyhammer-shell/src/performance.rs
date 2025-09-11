//! Performance monitoring and profiling for shell command execution
//!
//! This module provides comprehensive performance monitoring capabilities for shell operations,
//! including execution time tracking, memory usage monitoring, and resource utilization metrics.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Performance threshold configuration constants
const DEFAULT_OVERHEAD_THRESHOLD_MS: u64 = 100;
const DEFAULT_MEMORY_GROWTH_THRESHOLD_BYTES: u64 = 50 * 1024 * 1024; // 50MB
const DEFAULT_CLEANUP_THRESHOLD_SECS: u64 = 1;

/// Performance metrics for shell command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellPerformanceMetrics {
    /// Command that was executed
    pub command: String,
    /// Total execution time including overhead
    pub total_execution_time: Duration,
    /// Time spent in command execution only
    pub command_execution_time: Duration,
    /// Time spent in setup and cleanup
    pub overhead_time: Duration,
    /// Peak memory usage during execution (bytes)
    pub peak_memory_usage: u64,
    /// Memory usage at start of execution (bytes)
    pub initial_memory_usage: u64,
    /// Exit code of the command
    pub exit_code: i32,
    /// Size of stdout output (bytes)
    pub stdout_size: usize,
    /// Size of stderr output (bytes)
    pub stderr_size: usize,
    /// Whether output was truncated due to size limits
    pub output_truncated: bool,
    /// Number of processes spawned
    pub process_count: u32,
    /// Time to clean up processes after completion/timeout
    pub cleanup_time: Duration,
    /// Whether the command timed out
    pub timed_out: bool,
}

impl ShellPerformanceMetrics {
    /// Create new performance metrics
    pub fn new(command: String) -> Self {
        Self {
            command,
            total_execution_time: Duration::ZERO,
            command_execution_time: Duration::ZERO,
            overhead_time: Duration::ZERO,
            peak_memory_usage: 0,
            initial_memory_usage: 0,
            exit_code: 0,
            stdout_size: 0,
            stderr_size: 0,
            output_truncated: false,
            process_count: 1,
            cleanup_time: Duration::ZERO,
            timed_out: false,
        }
    }

    /// Calculate overhead percentage
    pub fn overhead_percentage(&self) -> f64 {
        if self.total_execution_time.is_zero() {
            0.0
        } else {
            (self.overhead_time.as_nanos() as f64 / self.total_execution_time.as_nanos() as f64)
                * 100.0
        }
    }

    /// Calculate memory growth (peak - initial)
    pub fn memory_growth(&self) -> u64 {
        self.peak_memory_usage
            .saturating_sub(self.initial_memory_usage)
    }

    /// Check if performance meets target thresholds
    pub fn meets_performance_targets(&self) -> bool {
        // Target: < 100ms overhead for simple commands
        let overhead_ok = self.overhead_time < Duration::from_millis(DEFAULT_OVERHEAD_THRESHOLD_MS);

        // Target: < 50MB memory growth for most commands
        let memory_ok = self.memory_growth() < DEFAULT_MEMORY_GROWTH_THRESHOLD_BYTES;

        // Target: cleanup < 1 second
        let cleanup_ok = self.cleanup_time < Duration::from_secs(DEFAULT_CLEANUP_THRESHOLD_SECS);

        overhead_ok && memory_ok && cleanup_ok
    }
}

/// Performance profiler for shell command execution
#[derive(Debug)]
pub struct ShellPerformanceProfiler {
    /// Current performance metrics being tracked
    current_metrics: Option<ShellPerformanceMetrics>,
    /// Historical metrics for analysis
    historical_metrics: Vec<ShellPerformanceMetrics>,
    /// Start time of current operation
    start_time: Option<Instant>,
    /// Time when command execution actually started
    command_start_time: Option<Instant>,
    /// Performance configuration
    config: PerformanceConfig,
}

/// Configuration for performance monitoring
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Whether to collect memory usage metrics
    pub collect_memory_metrics: bool,
    /// Whether to log performance metrics
    pub log_metrics: bool,
    /// Maximum number of historical metrics to keep
    pub max_historical_metrics: usize,
    /// Whether to warn on performance threshold violations
    pub warn_on_threshold_violations: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            collect_memory_metrics: true,
            log_metrics: true,
            max_historical_metrics: 1000,
            warn_on_threshold_violations: true,
        }
    }
}

impl ShellPerformanceProfiler {
    /// Create a new performance profiler
    pub fn new() -> Self {
        Self::with_config(PerformanceConfig::default())
    }

    /// Create a new performance profiler with custom configuration
    pub fn with_config(config: PerformanceConfig) -> Self {
        Self {
            current_metrics: None,
            historical_metrics: Vec::new(),
            start_time: None,
            command_start_time: None,
            config,
        }
    }

    /// Start profiling a command execution
    pub fn start_profiling(&mut self, command: String) {
        let mut metrics = ShellPerformanceMetrics::new(command);

        if self.config.collect_memory_metrics {
            metrics.initial_memory_usage = self.get_memory_usage();
        }

        self.current_metrics = Some(metrics);
        self.start_time = Some(Instant::now());

        debug!("Started profiling shell command");
    }

    /// Mark the start of actual command execution (after setup)
    pub fn mark_command_start(&mut self) {
        self.command_start_time = Some(Instant::now());
        debug!("Marked command execution start");
    }

    /// Mark the end of command execution (before cleanup)
    pub fn mark_command_end(&mut self, exit_code: i32) {
        if let Some(command_start) = self.command_start_time {
            let current_memory = if self.config.collect_memory_metrics {
                self.get_memory_usage()
            } else {
                0
            };

            if let Some(metrics) = &mut self.current_metrics {
                metrics.command_execution_time = command_start.elapsed();
                metrics.exit_code = exit_code;

                if self.config.collect_memory_metrics {
                    metrics.peak_memory_usage = metrics.peak_memory_usage.max(current_memory);
                }
            }

            debug!("Marked command execution end, exit code: {}", exit_code);
        }
    }

    /// Set output information
    pub fn set_output_info(&mut self, stdout_size: usize, stderr_size: usize, truncated: bool) {
        if let Some(metrics) = &mut self.current_metrics {
            metrics.stdout_size = stdout_size;
            metrics.stderr_size = stderr_size;
            metrics.output_truncated = truncated;
        }
    }

    /// Set timeout information
    pub fn set_timeout_info(&mut self, timed_out: bool) {
        if let Some(metrics) = &mut self.current_metrics {
            metrics.timed_out = timed_out;
        }
    }

    /// Record cleanup time
    pub fn record_cleanup_time(&mut self, cleanup_time: Duration) {
        if let Some(metrics) = &mut self.current_metrics {
            metrics.cleanup_time = cleanup_time;
        }
    }

    /// Finish profiling and return the metrics
    pub fn finish_profiling(&mut self) -> Option<ShellPerformanceMetrics> {
        if let (Some(mut metrics), Some(start_time)) =
            (self.current_metrics.take(), self.start_time.take())
        {
            // Calculate total execution time
            metrics.total_execution_time = start_time.elapsed();

            // Calculate overhead time
            metrics.overhead_time = metrics
                .total_execution_time
                .saturating_sub(metrics.command_execution_time);

            // Update peak memory usage one final time
            if self.config.collect_memory_metrics {
                let current_memory = self.get_memory_usage();
                metrics.peak_memory_usage = metrics.peak_memory_usage.max(current_memory);
            }

            // Log metrics if configured
            if self.config.log_metrics {
                self.log_metrics(&metrics);
            }

            // Check performance thresholds
            if self.config.warn_on_threshold_violations && !metrics.meets_performance_targets() {
                self.warn_performance_issues(&metrics);
            }

            // Store in historical metrics
            self.add_to_historical_metrics(metrics.clone());

            self.command_start_time = None;

            Some(metrics)
        } else {
            warn!("Attempted to finish profiling without starting");
            None
        }
    }

    /// Get performance statistics from historical data
    pub fn get_performance_statistics(&self) -> PerformanceStatistics {
        PerformanceStatistics::from_metrics(&self.historical_metrics)
    }

    /// Clear historical metrics
    pub fn clear_historical_metrics(&mut self) {
        self.historical_metrics.clear();
    }

    /// Get memory usage (platform-specific implementation)
    fn get_memory_usage(&self) -> u64 {
        #[cfg(target_os = "linux")]
        {
            self.get_memory_usage_linux()
        }

        #[cfg(target_os = "macos")]
        {
            self.get_memory_usage_macos()
        }

        #[cfg(target_os = "windows")]
        {
            self.get_memory_usage_windows()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Fallback for unsupported platforms
            0
        }
    }

    #[cfg(target_os = "linux")]
    fn get_memory_usage_linux(&self) -> u64 {
        // Read from /proc/self/status for VmRSS (resident set size)
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb * 1024; // Convert KB to bytes
                        }
                    }
                }
            }
        }
        0
    }

    #[cfg(target_os = "macos")]
    fn get_memory_usage_macos(&self) -> u64 {
        // Use task_info to get memory usage on macOS
        // This is a simplified implementation
        // In production, would use proper macOS system calls
        0
    }

    #[cfg(target_os = "windows")]
    fn get_memory_usage_windows(&self) -> u64 {
        // Use Windows API to get memory usage
        // This is a simplified implementation
        // In production, would use proper Windows API calls
        0
    }

    /// Log performance metrics
    fn log_metrics(&self, metrics: &ShellPerformanceMetrics) {
        info!(
            "Shell command performance: cmd='{}' total={}ms exec={}ms overhead={}ms mem_growth={}KB exit={}",
            metrics.command,
            metrics.total_execution_time.as_millis(),
            metrics.command_execution_time.as_millis(),
            metrics.overhead_time.as_millis(),
            metrics.memory_growth() / 1024,
            metrics.exit_code
        );
    }

    /// Warn about performance issues
    fn warn_performance_issues(&self, metrics: &ShellPerformanceMetrics) {
        if metrics.overhead_time >= Duration::from_millis(100) {
            warn!(
                "High shell command overhead: {}ms for command '{}'",
                metrics.overhead_time.as_millis(),
                metrics.command
            );
        }

        if metrics.memory_growth() >= DEFAULT_MEMORY_GROWTH_THRESHOLD_BYTES {
            warn!(
                "High memory usage: {}MB growth for command '{}'",
                metrics.memory_growth() / (1024 * 1024),
                metrics.command
            );
        }

        if metrics.cleanup_time >= Duration::from_secs(DEFAULT_CLEANUP_THRESHOLD_SECS) {
            warn!(
                "Slow cleanup: {}ms for command '{}'",
                metrics.cleanup_time.as_millis(),
                metrics.command
            );
        }
    }

    /// Add metrics to historical collection
    fn add_to_historical_metrics(&mut self, metrics: ShellPerformanceMetrics) {
        self.historical_metrics.push(metrics);

        // Limit historical metrics size
        if self.historical_metrics.len() > self.config.max_historical_metrics {
            self.historical_metrics.remove(0);
        }
    }
}

impl Default for ShellPerformanceProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance statistics aggregated from multiple command executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStatistics {
    /// Total number of commands executed
    pub total_commands: usize,
    /// Average execution time
    pub avg_execution_time: Duration,
    /// 95th percentile execution time
    pub p95_execution_time: Duration,
    /// 99th percentile execution time
    pub p99_execution_time: Duration,
    /// Average overhead time
    pub avg_overhead_time: Duration,
    /// Average memory growth
    pub avg_memory_growth: u64,
    /// Maximum memory growth observed
    pub max_memory_growth: u64,
    /// Success rate (commands with exit code 0)
    pub success_rate: f64,
    /// Timeout rate (commands that timed out)
    pub timeout_rate: f64,
    /// Commands that met performance targets
    pub performance_target_rate: f64,
}

impl PerformanceStatistics {
    /// Calculate statistics from a collection of metrics
    pub fn from_metrics(metrics: &[ShellPerformanceMetrics]) -> Self {
        if metrics.is_empty() {
            return Self {
                total_commands: 0,
                avg_execution_time: Duration::ZERO,
                p95_execution_time: Duration::ZERO,
                p99_execution_time: Duration::ZERO,
                avg_overhead_time: Duration::ZERO,
                avg_memory_growth: 0,
                max_memory_growth: 0,
                success_rate: 0.0,
                timeout_rate: 0.0,
                performance_target_rate: 0.0,
            };
        }

        let total_commands = metrics.len();

        // Calculate averages
        let total_exec_time: Duration = metrics.iter().map(|m| m.total_execution_time).sum();
        let avg_execution_time = total_exec_time / total_commands as u32;

        let total_overhead_time: Duration = metrics.iter().map(|m| m.overhead_time).sum();
        let avg_overhead_time = total_overhead_time / total_commands as u32;

        let total_memory_growth: u64 = metrics.iter().map(|m| m.memory_growth()).sum();
        let avg_memory_growth = total_memory_growth / total_commands as u64;

        let max_memory_growth = metrics.iter().map(|m| m.memory_growth()).max().unwrap_or(0);

        // Calculate percentiles
        let mut exec_times: Vec<Duration> =
            metrics.iter().map(|m| m.total_execution_time).collect();
        exec_times.sort();

        let p95_index = (total_commands as f64 * 0.95) as usize;
        let p95_execution_time = exec_times.get(p95_index).copied().unwrap_or(Duration::ZERO);

        let p99_index = (total_commands as f64 * 0.99) as usize;
        let p99_execution_time = exec_times.get(p99_index).copied().unwrap_or(Duration::ZERO);

        // Calculate rates
        let successful_commands = metrics.iter().filter(|m| m.exit_code == 0).count();
        let success_rate = successful_commands as f64 / total_commands as f64;

        let timed_out_commands = metrics.iter().filter(|m| m.timed_out).count();
        let timeout_rate = timed_out_commands as f64 / total_commands as f64;

        let performant_commands = metrics
            .iter()
            .filter(|m| m.meets_performance_targets())
            .count();
        let performance_target_rate = performant_commands as f64 / total_commands as f64;

        Self {
            total_commands,
            avg_execution_time,
            p95_execution_time,
            p99_execution_time,
            avg_overhead_time,
            avg_memory_growth,
            max_memory_growth,
            success_rate,
            timeout_rate,
            performance_target_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_lifecycle() {
        let mut profiler = ShellPerformanceProfiler::new();

        // Start profiling
        profiler.start_profiling("echo test".to_string());

        // Mark command phases
        std::thread::sleep(Duration::from_millis(1)); // Simulate setup time
        profiler.mark_command_start();

        std::thread::sleep(Duration::from_millis(1)); // Simulate execution time
        profiler.mark_command_end(0);

        // Set output info
        profiler.set_output_info(10, 0, false);

        // Finish profiling
        let metrics = profiler.finish_profiling();
        assert!(metrics.is_some());

        let metrics = metrics.unwrap();
        assert_eq!(metrics.command, "echo test");
        assert_eq!(metrics.exit_code, 0);
        assert!(!metrics.total_execution_time.is_zero());
        assert_eq!(metrics.stdout_size, 10);
    }
}
