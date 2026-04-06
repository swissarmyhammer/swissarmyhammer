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

        let p95_index = ((total_commands as f64 * 0.95).ceil() as usize).saturating_sub(1);
        let p95_execution_time = exec_times.get(p95_index).copied().unwrap_or(Duration::ZERO);

        let p99_index = ((total_commands as f64 * 0.99).ceil() as usize).saturating_sub(1);
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
    use tracing_test::traced_test;

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

    #[test]
    fn test_set_timeout_info() {
        let mut profiler = ShellPerformanceProfiler::new();

        profiler.start_profiling("sleep 999".to_string());
        profiler.mark_command_start();
        profiler.set_timeout_info(true);
        profiler.mark_command_end(124);

        let metrics = profiler
            .finish_profiling()
            .expect("metrics should be present");
        assert!(
            metrics.timed_out,
            "timed_out should be true after set_timeout_info(true)"
        );
    }

    #[test]
    fn test_record_cleanup_time() {
        let mut profiler = ShellPerformanceProfiler::new();
        let cleanup_duration = Duration::from_millis(42);

        profiler.start_profiling("echo cleanup".to_string());
        profiler.mark_command_start();
        profiler.mark_command_end(0);
        profiler.record_cleanup_time(cleanup_duration);

        let metrics = profiler
            .finish_profiling()
            .expect("metrics should be present");
        assert_eq!(
            metrics.cleanup_time, cleanup_duration,
            "cleanup_time should match the recorded duration"
        );
    }

    /// Helper to build a metric with specific field overrides for testing.
    fn make_metric(
        total_time_ms: u64,
        overhead_ms: u64,
        initial_mem: u64,
        peak_mem: u64,
        exit_code: i32,
        timed_out: bool,
    ) -> ShellPerformanceMetrics {
        ShellPerformanceMetrics {
            command: "test".to_string(),
            total_execution_time: Duration::from_millis(total_time_ms),
            command_execution_time: Duration::from_millis(
                total_time_ms.saturating_sub(overhead_ms),
            ),
            overhead_time: Duration::from_millis(overhead_ms),
            peak_memory_usage: peak_mem,
            initial_memory_usage: initial_mem,
            exit_code,
            stdout_size: 0,
            stderr_size: 0,
            output_truncated: false,
            process_count: 1,
            cleanup_time: Duration::ZERO,
            timed_out,
        }
    }

    #[test]
    fn test_from_metrics_empty() {
        let stats = PerformanceStatistics::from_metrics(&[]);

        assert_eq!(stats.total_commands, 0);
        assert_eq!(stats.avg_execution_time, Duration::ZERO);
        assert_eq!(stats.p95_execution_time, Duration::ZERO);
        assert_eq!(stats.p99_execution_time, Duration::ZERO);
        assert_eq!(stats.avg_overhead_time, Duration::ZERO);
        assert_eq!(stats.avg_memory_growth, 0);
        assert_eq!(stats.max_memory_growth, 0);
        assert_eq!(stats.success_rate, 0.0);
        assert_eq!(stats.timeout_rate, 0.0);
        assert_eq!(stats.performance_target_rate, 0.0);
    }

    #[test]
    fn test_from_metrics_single() {
        let m = make_metric(
            200,   // total 200ms
            10,    // overhead 10ms
            1000,  // initial mem
            5000,  // peak mem (growth = 4000)
            0,     // success
            false, // no timeout
        );
        let stats = PerformanceStatistics::from_metrics(&[m]);

        assert_eq!(stats.total_commands, 1);
        assert_eq!(stats.avg_execution_time, Duration::from_millis(200));
        assert_eq!(stats.avg_overhead_time, Duration::from_millis(10));
        assert_eq!(stats.avg_memory_growth, 4000);
        assert_eq!(stats.max_memory_growth, 4000);
        assert_eq!(stats.success_rate, 1.0);
        assert_eq!(stats.timeout_rate, 0.0);
        assert_eq!(stats.performance_target_rate, 1.0);
        // With a single element, p95 index = (1 * 0.95) as usize = 0
        assert_eq!(stats.p95_execution_time, Duration::from_millis(200));
        // p99 index = (1 * 0.99) as usize = 0
        assert_eq!(stats.p99_execution_time, Duration::from_millis(200));
    }

    #[test]
    fn test_from_metrics_multiple_varied() {
        // Build 4 metrics with different characteristics:
        //  0: 100ms, exit 0, no timeout, mem growth 1000
        //  1: 200ms, exit 1, no timeout, mem growth 2000
        //  2: 300ms, exit 0, timed out,  mem growth 3000
        //  3: 400ms, exit 0, no timeout, mem growth 4000
        let metrics = vec![
            make_metric(100, 5, 0, 1000, 0, false),
            make_metric(200, 15, 0, 2000, 1, false),
            make_metric(300, 25, 0, 3000, 0, true),
            make_metric(400, 35, 0, 4000, 0, false),
        ];
        let stats = PerformanceStatistics::from_metrics(&metrics);

        assert_eq!(stats.total_commands, 4);

        // avg execution = (100+200+300+400)/4 = 250ms
        assert_eq!(stats.avg_execution_time, Duration::from_millis(250));

        // avg overhead = (5+15+25+35)/4 = 20ms
        assert_eq!(stats.avg_overhead_time, Duration::from_millis(20));

        // avg memory growth = (1000+2000+3000+4000)/4 = 2500
        assert_eq!(stats.avg_memory_growth, 2500);

        // max memory growth = 4000
        assert_eq!(stats.max_memory_growth, 4000);

        // Success: metrics 0, 2, 3 have exit_code 0 => 3/4 = 0.75
        assert!((stats.success_rate - 0.75).abs() < f64::EPSILON);

        // Timeout: only metric 2 => 1/4 = 0.25
        assert!((stats.timeout_rate - 0.25).abs() < f64::EPSILON);

        // All four meet performance targets (overhead < 100ms, mem growth < 50MB, cleanup < 1s)
        assert!((stats.performance_target_rate - 1.0).abs() < f64::EPSILON);

        // Sorted exec times: [100, 200, 300, 400]
        // p95 index = (4 * 0.95) as usize = 3 => 400ms
        assert_eq!(stats.p95_execution_time, Duration::from_millis(400));
        // p99 index = (4 * 0.99) as usize = 3 => 400ms
        assert_eq!(stats.p99_execution_time, Duration::from_millis(400));
    }

    #[test]
    fn test_from_metrics_percentile_ordering() {
        // 10 metrics with execution times 10..100ms to verify percentile index selection
        let metrics: Vec<ShellPerformanceMetrics> = (1..=10)
            .map(|i| make_metric(i * 10, 0, 0, 0, 0, false))
            .collect();
        let stats = PerformanceStatistics::from_metrics(&metrics);

        assert_eq!(stats.total_commands, 10);
        // Sorted: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
        // p95 index = (10 * 0.95) as usize = 9 => 100ms
        assert_eq!(stats.p95_execution_time, Duration::from_millis(100));
        // p99 index = (10 * 0.99) as usize = 9 => 100ms
        assert_eq!(stats.p99_execution_time, Duration::from_millis(100));
    }

    #[test]
    fn test_from_metrics_all_failures_and_timeouts() {
        let metrics = vec![
            make_metric(50, 0, 0, 0, 1, true),
            make_metric(60, 0, 0, 0, 127, true),
        ];
        let stats = PerformanceStatistics::from_metrics(&metrics);

        assert_eq!(stats.total_commands, 2);
        assert_eq!(stats.success_rate, 0.0);
        assert_eq!(stats.timeout_rate, 1.0);
    }

    /// Helper: create a profiler with pre-injected metrics for threshold testing.
    /// Disables memory collection so the profiler won't overwrite injected values.
    fn profiler_with_injected_metrics(
        metrics: ShellPerformanceMetrics,
    ) -> ShellPerformanceProfiler {
        let config = PerformanceConfig {
            collect_memory_metrics: false,
            log_metrics: true,
            max_historical_metrics: 100,
            warn_on_threshold_violations: true,
        };
        let mut profiler = ShellPerformanceProfiler::with_config(config);
        profiler.current_metrics = Some(metrics);
        profiler.start_time = Some(Instant::now());
        profiler.command_start_time = Some(Instant::now());
        profiler
    }

    #[test]
    #[traced_test]
    fn test_warn_high_overhead() {
        // Exercises warn_performance_issues overhead path (lines 341-347)
        // and log_metrics (lines 327-337).
        // Sets start_time 150ms in the past so finish_profiling() computes
        // overhead >= 100ms without any thread::sleep.
        let mut metrics = ShellPerformanceMetrics::new("slow-setup".to_string());
        metrics.command_execution_time = Duration::ZERO;

        let config = PerformanceConfig {
            collect_memory_metrics: false,
            log_metrics: true,
            max_historical_metrics: 100,
            warn_on_threshold_violations: true,
        };
        let mut profiler = ShellPerformanceProfiler::with_config(config);
        profiler.current_metrics = Some(metrics);
        // Place start_time 150ms in the past so elapsed() returns ~150ms
        profiler.start_time = Instant::now().checked_sub(Duration::from_millis(150));
        profiler.command_start_time = Some(Instant::now());

        let result = profiler.finish_profiling();
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(
            m.overhead_time >= Duration::from_millis(100),
            "Expected overhead >= 100ms, got {}ms",
            m.overhead_time.as_millis()
        );
        assert!(
            !m.meets_performance_targets(),
            "Should NOT meet performance targets with high overhead"
        );

        // Verify the warn log was actually emitted
        assert!(logs_contain("High shell command overhead"));
        assert!(logs_contain("slow-setup"));
    }

    #[test]
    #[traced_test]
    fn test_warn_high_memory_growth() {
        // Exercises warn_performance_issues memory-growth path (lines 349-355)
        // and log_metrics (lines 327-337).
        // Inject peak_memory_usage = 60MB with initial = 0 so growth >= 50MB.
        let mut metrics = ShellPerformanceMetrics::new("memory-hog".to_string());
        metrics.initial_memory_usage = 0;
        metrics.peak_memory_usage = 60 * 1024 * 1024; // 60MB
                                                      // Keep command_execution_time near total so overhead stays low
        metrics.command_execution_time = Duration::from_millis(1);

        let mut profiler = profiler_with_injected_metrics(metrics);

        let result = profiler.finish_profiling();
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(
            m.memory_growth() >= DEFAULT_MEMORY_GROWTH_THRESHOLD_BYTES,
            "Expected memory growth >= 50MB, got {}MB",
            m.memory_growth() / (1024 * 1024)
        );
        assert!(
            !m.meets_performance_targets(),
            "Should NOT meet performance targets with high memory growth"
        );

        // Verify the warn log was actually emitted
        assert!(logs_contain("High memory usage"));
        assert!(logs_contain("memory-hog"));
    }

    #[test]
    #[traced_test]
    fn test_warn_slow_cleanup() {
        // Exercises warn_performance_issues cleanup path (lines 357-363)
        // and log_metrics (lines 327-337).
        // Inject cleanup_time = 2s which exceeds the 1s threshold.
        let mut metrics = ShellPerformanceMetrics::new("slow-cleanup".to_string());
        metrics.cleanup_time = Duration::from_secs(2);
        // Keep command_execution_time near total so overhead stays low
        metrics.command_execution_time = Duration::from_millis(1);

        let mut profiler = profiler_with_injected_metrics(metrics);

        let result = profiler.finish_profiling();
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(
            m.cleanup_time >= Duration::from_secs(DEFAULT_CLEANUP_THRESHOLD_SECS),
            "Expected cleanup >= 1s, got {}ms",
            m.cleanup_time.as_millis()
        );
        assert!(
            !m.meets_performance_targets(),
            "Should NOT meet performance targets with slow cleanup"
        );

        // Verify the warn log was actually emitted
        assert!(logs_contain("Slow cleanup"));
        assert!(logs_contain("slow-cleanup"));
    }

    #[test]
    fn test_overhead_percentage_zero_total_time() {
        // When total_execution_time is zero, overhead_percentage should return 0.0
        // to avoid division by zero (early return at line 68-69).
        let metrics = ShellPerformanceMetrics::new("test".to_string());
        assert_eq!(metrics.overhead_percentage(), 0.0);
    }

    #[test]
    fn test_overhead_percentage_nonzero_values() {
        // total=1000ms, overhead=250ms → 25.0%
        let mut metrics = ShellPerformanceMetrics::new("test".to_string());
        metrics.total_execution_time = Duration::from_millis(1000);
        metrics.overhead_time = Duration::from_millis(250);
        let pct = metrics.overhead_percentage();
        assert!(
            (pct - 25.0).abs() < f64::EPSILON,
            "Expected 25.0%, got {pct}%"
        );
    }

    #[test]
    fn test_overhead_percentage_all_overhead() {
        // When total == overhead, the result should be 100.0%
        let mut metrics = ShellPerformanceMetrics::new("test".to_string());
        metrics.total_execution_time = Duration::from_millis(500);
        metrics.overhead_time = Duration::from_millis(500);
        let pct = metrics.overhead_percentage();
        assert!(
            (pct - 100.0).abs() < f64::EPSILON,
            "Expected 100.0%, got {pct}%"
        );
    }

    #[test]
    #[traced_test]
    fn test_log_metrics_on_healthy_command() {
        // Exercises log_metrics (lines 327-337) via finish_profiling on a command
        // that meets all performance targets (no warnings fired).
        let config = PerformanceConfig {
            collect_memory_metrics: false,
            log_metrics: true,
            max_historical_metrics: 100,
            warn_on_threshold_violations: false,
        };
        let mut profiler = ShellPerformanceProfiler::with_config(config);
        profiler.start_profiling("echo hello".to_string());
        profiler.mark_command_start();
        profiler.mark_command_end(0);

        let result = profiler.finish_profiling();
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(
            m.meets_performance_targets(),
            "Healthy command should meet all performance targets"
        );
        // Verify it was stored in historical metrics
        let stats = profiler.get_performance_statistics();
        assert_eq!(stats.total_commands, 1);

        // Verify the info-level performance log was emitted
        assert!(logs_contain("Shell command performance"));
        assert!(logs_contain("echo hello"));
    }

    #[test]
    fn test_profiler_default_impl() {
        // Exercises Default impl for ShellPerformanceProfiler (lines 378-380)
        let profiler: ShellPerformanceProfiler = Default::default();
        assert!(profiler.current_metrics.is_none());
        assert!(profiler.historical_metrics.is_empty());
    }

    #[test]
    fn test_finish_profiling_without_starting() {
        // Exercises finish_profiling when no profiling was started (lines 254-255)
        let mut profiler = ShellPerformanceProfiler::new();
        let result = profiler.finish_profiling();
        assert!(result.is_none());
    }

    #[test]
    fn test_mark_command_end_without_start() {
        // Exercises mark_command_end when command_start_time is None (line 189)
        let mut profiler = ShellPerformanceProfiler::new();
        profiler.start_profiling("test".to_string());
        // Don't call mark_command_start — command_start_time is None
        profiler.mark_command_end(0);
        // Should not panic, metrics.command_execution_time stays at ZERO
        let metrics = profiler.finish_profiling().unwrap();
        assert_eq!(metrics.command_execution_time, Duration::ZERO);
    }

    #[test]
    fn test_set_output_info_without_metrics() {
        // Exercises set_output_info when current_metrics is None (line 192 guard)
        let mut profiler = ShellPerformanceProfiler::new();
        // Don't start profiling — no current_metrics
        profiler.set_output_info(100, 50, true);
        // Should not panic
    }

    #[test]
    fn test_set_timeout_info_without_metrics() {
        // Exercises set_timeout_info when current_metrics is None
        let mut profiler = ShellPerformanceProfiler::new();
        profiler.set_timeout_info(true);
        // Should not panic
    }

    #[test]
    fn test_record_cleanup_time_without_metrics() {
        // Exercises record_cleanup_time when current_metrics is None
        let mut profiler = ShellPerformanceProfiler::new();
        profiler.record_cleanup_time(Duration::from_millis(100));
        // Should not panic
    }

    #[test]
    fn test_clear_historical_metrics() {
        // Exercises clear_historical_metrics (lines 265-267)
        let mut profiler = ShellPerformanceProfiler::new();

        // Add some metrics
        profiler.start_profiling("cmd1".to_string());
        profiler.mark_command_start();
        profiler.mark_command_end(0);
        profiler.finish_profiling();

        let stats = profiler.get_performance_statistics();
        assert_eq!(stats.total_commands, 1);

        profiler.clear_historical_metrics();
        let stats = profiler.get_performance_statistics();
        assert_eq!(stats.total_commands, 0);
    }

    #[test]
    fn test_historical_metrics_overflow_trim() {
        // Exercises add_to_historical_metrics overflow (line 372)
        let config = PerformanceConfig {
            collect_memory_metrics: false,
            log_metrics: false,
            max_historical_metrics: 3,
            warn_on_threshold_violations: false,
        };
        let mut profiler = ShellPerformanceProfiler::with_config(config);

        // Add 5 metrics — should trim to max of 3
        for i in 0..5 {
            profiler.start_profiling(format!("cmd{}", i));
            profiler.mark_command_start();
            profiler.mark_command_end(0);
            profiler.finish_profiling();
        }

        let stats = profiler.get_performance_statistics();
        assert_eq!(
            stats.total_commands, 3,
            "Should trim to max_historical_metrics"
        );
    }

    #[test]
    fn test_memory_growth_saturating() {
        // Exercises memory_growth() saturating subtraction (line 79)
        let mut metrics = ShellPerformanceMetrics::new("test".to_string());
        metrics.initial_memory_usage = 1000;
        metrics.peak_memory_usage = 500; // Peak less than initial
        assert_eq!(metrics.memory_growth(), 0, "Should saturate at 0");
    }

    #[test]
    fn test_meets_performance_targets_all_passing() {
        // Exercises meets_performance_targets happy path (lines 83-94)
        let mut metrics = ShellPerformanceMetrics::new("test".to_string());
        metrics.overhead_time = Duration::from_millis(10); // < 100ms
        metrics.initial_memory_usage = 0;
        metrics.peak_memory_usage = 1024; // < 50MB growth
        metrics.cleanup_time = Duration::from_millis(100); // < 1s
        assert!(metrics.meets_performance_targets());
    }

    #[test]
    fn test_meets_performance_targets_high_overhead() {
        let mut metrics = ShellPerformanceMetrics::new("test".to_string());
        metrics.overhead_time = Duration::from_millis(200); // >= 100ms
        metrics.cleanup_time = Duration::ZERO;
        assert!(!metrics.meets_performance_targets());
    }

    #[test]
    fn test_meets_performance_targets_high_memory() {
        let mut metrics = ShellPerformanceMetrics::new("test".to_string());
        metrics.overhead_time = Duration::from_millis(10);
        metrics.initial_memory_usage = 0;
        metrics.peak_memory_usage = 60 * 1024 * 1024; // 60MB >= 50MB threshold
        metrics.cleanup_time = Duration::ZERO;
        assert!(!metrics.meets_performance_targets());
    }

    #[test]
    fn test_meets_performance_targets_slow_cleanup() {
        let mut metrics = ShellPerformanceMetrics::new("test".to_string());
        metrics.overhead_time = Duration::from_millis(10);
        metrics.cleanup_time = Duration::from_secs(2); // >= 1s
        assert!(!metrics.meets_performance_targets());
    }

    #[test]
    fn test_log_metrics_disabled() {
        // Exercises the path where log_metrics is false
        let config = PerformanceConfig {
            collect_memory_metrics: false,
            log_metrics: false,
            max_historical_metrics: 100,
            warn_on_threshold_violations: false,
        };
        let mut profiler = ShellPerformanceProfiler::with_config(config);
        profiler.start_profiling("quiet".to_string());
        profiler.mark_command_start();
        profiler.mark_command_end(0);

        let metrics = profiler.finish_profiling();
        assert!(metrics.is_some());
    }

    #[test]
    fn test_profiler_with_memory_metrics_disabled() {
        // Exercises paths where collect_memory_metrics is false
        let config = PerformanceConfig {
            collect_memory_metrics: false,
            log_metrics: false,
            max_historical_metrics: 100,
            warn_on_threshold_violations: false,
        };
        let mut profiler = ShellPerformanceProfiler::with_config(config);
        profiler.start_profiling("no-mem".to_string());
        profiler.mark_command_start();
        profiler.mark_command_end(0);

        let metrics = profiler.finish_profiling().unwrap();
        assert_eq!(metrics.initial_memory_usage, 0);
        assert_eq!(metrics.peak_memory_usage, 0);
    }
}
