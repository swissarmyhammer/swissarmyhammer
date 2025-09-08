use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::Duration;

/// Performance metrics collector for issue storage operations
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Counters for different operation types
    pub operation_counts: Arc<OperationCounts>,
    /// Timing statistics for different operation types
    pub timing_stats: Arc<TimingStats>,
}

/// Atomic counters for different operation types
#[derive(Debug)]
pub struct OperationCounts {
    /// Number of create operations performed
    pub create_operations: AtomicU64,
    /// Number of read operations performed
    pub read_operations: AtomicU64,
    /// Number of update operations performed
    pub update_operations: AtomicU64,
    /// Number of delete operations performed
    pub delete_operations: AtomicU64,
    /// Number of list operations performed
    pub list_operations: AtomicU64,
}

/// Atomic timing statistics for different operation types
#[derive(Debug)]
pub struct TimingStats {
    /// Total time spent on create operations (microseconds)
    pub total_create_time: AtomicU64,
    /// Total time spent on read operations (microseconds)
    pub total_read_time: AtomicU64,
    /// Total time spent on update operations (microseconds)
    pub total_update_time: AtomicU64,
    /// Total time spent on delete operations (microseconds)
    pub total_delete_time: AtomicU64,
    /// Total time spent on list operations (microseconds)
    pub total_list_time: AtomicU64,
}

impl PerformanceMetrics {
    /// Create a new performance metrics collector
    pub fn new() -> Self {
        Self {
            operation_counts: Arc::new(OperationCounts {
                create_operations: AtomicU64::new(0),
                read_operations: AtomicU64::new(0),
                update_operations: AtomicU64::new(0),
                delete_operations: AtomicU64::new(0),
                list_operations: AtomicU64::new(0),
            }),
            timing_stats: Arc::new(TimingStats {
                total_create_time: AtomicU64::new(0),
                total_read_time: AtomicU64::new(0),
                total_update_time: AtomicU64::new(0),
                total_delete_time: AtomicU64::new(0),
                total_list_time: AtomicU64::new(0),
            }),
        }
    }

    /// Record a completed operation with its duration
    pub fn record_operation(&self, operation: Operation, duration: Duration) {
        let duration_micros = duration.as_micros() as u64;

        match operation {
            Operation::Create => {
                self.operation_counts
                    .create_operations
                    .fetch_add(1, Ordering::Relaxed);
                self.timing_stats
                    .total_create_time
                    .fetch_add(duration_micros, Ordering::Relaxed);
            }
            Operation::Read => {
                self.operation_counts
                    .read_operations
                    .fetch_add(1, Ordering::Relaxed);
                self.timing_stats
                    .total_read_time
                    .fetch_add(duration_micros, Ordering::Relaxed);
            }
            Operation::Update => {
                self.operation_counts
                    .update_operations
                    .fetch_add(1, Ordering::Relaxed);
                self.timing_stats
                    .total_update_time
                    .fetch_add(duration_micros, Ordering::Relaxed);
            }
            Operation::Delete => {
                self.operation_counts
                    .delete_operations
                    .fetch_add(1, Ordering::Relaxed);
                self.timing_stats
                    .total_delete_time
                    .fetch_add(duration_micros, Ordering::Relaxed);
            }
            Operation::List => {
                self.operation_counts
                    .list_operations
                    .fetch_add(1, Ordering::Relaxed);
                self.timing_stats
                    .total_list_time
                    .fetch_add(duration_micros, Ordering::Relaxed);
            }
        }
    }

    /// Get a snapshot of current metrics
    pub fn get_stats(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            create_ops: self
                .operation_counts
                .create_operations
                .load(Ordering::Relaxed),
            read_ops: self
                .operation_counts
                .read_operations
                .load(Ordering::Relaxed),
            update_ops: self
                .operation_counts
                .update_operations
                .load(Ordering::Relaxed),
            delete_ops: self
                .operation_counts
                .delete_operations
                .load(Ordering::Relaxed),
            list_ops: self
                .operation_counts
                .list_operations
                .load(Ordering::Relaxed),

            avg_create_time: self.calculate_avg_time(
                self.timing_stats.total_create_time.load(Ordering::Relaxed),
                self.operation_counts
                    .create_operations
                    .load(Ordering::Relaxed),
            ),
            avg_read_time: self.calculate_avg_time(
                self.timing_stats.total_read_time.load(Ordering::Relaxed),
                self.operation_counts
                    .read_operations
                    .load(Ordering::Relaxed),
            ),
            avg_update_time: self.calculate_avg_time(
                self.timing_stats.total_update_time.load(Ordering::Relaxed),
                self.operation_counts
                    .update_operations
                    .load(Ordering::Relaxed),
            ),
            avg_delete_time: self.calculate_avg_time(
                self.timing_stats.total_delete_time.load(Ordering::Relaxed),
                self.operation_counts
                    .delete_operations
                    .load(Ordering::Relaxed),
            ),
            avg_list_time: self.calculate_avg_time(
                self.timing_stats.total_list_time.load(Ordering::Relaxed),
                self.operation_counts
                    .list_operations
                    .load(Ordering::Relaxed),
            ),
        }
    }

    fn calculate_avg_time(&self, total_time: u64, count: u64) -> f64 {
        if count == 0 {
            0.0
        } else {
            total_time as f64 / count as f64
        }
    }

    /// Reset all metrics to zero
    pub fn reset(&self) {
        self.operation_counts
            .create_operations
            .store(0, Ordering::Relaxed);
        self.operation_counts
            .read_operations
            .store(0, Ordering::Relaxed);
        self.operation_counts
            .update_operations
            .store(0, Ordering::Relaxed);
        self.operation_counts
            .delete_operations
            .store(0, Ordering::Relaxed);
        self.operation_counts
            .list_operations
            .store(0, Ordering::Relaxed);

        self.timing_stats
            .total_create_time
            .store(0, Ordering::Relaxed);
        self.timing_stats
            .total_read_time
            .store(0, Ordering::Relaxed);
        self.timing_stats
            .total_update_time
            .store(0, Ordering::Relaxed);
        self.timing_stats
            .total_delete_time
            .store(0, Ordering::Relaxed);
        self.timing_stats
            .total_list_time
            .store(0, Ordering::Relaxed);
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Types of operations that can be performed on issues
#[derive(Debug, Clone)]
pub enum Operation {
    /// Create a new issue
    Create,
    /// Read an existing issue
    Read,
    /// Update an existing issue
    Update,
    /// Delete or mark an issue as complete
    Delete,
    /// List all issues
    List,
}

/// A snapshot of performance metrics at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Number of create operations performed
    pub create_ops: u64,
    /// Number of read operations performed
    pub read_ops: u64,
    /// Number of update operations performed
    pub update_ops: u64,
    /// Number of delete operations performed
    pub delete_ops: u64,
    /// Number of list operations performed
    pub list_ops: u64,

    /// Average time per create operation (microseconds)
    pub avg_create_time: f64,
    /// Average time per read operation (microseconds)
    pub avg_read_time: f64,
    /// Average time per update operation (microseconds)
    pub avg_update_time: f64,
    /// Average time per delete operation (microseconds)
    pub avg_delete_time: f64,
    /// Average time per list operation (microseconds)
    pub avg_list_time: f64,
}

impl MetricsSnapshot {
    /// Get the total number of operations performed
    pub fn total_operations(&self) -> u64 {
        self.create_ops + self.read_ops + self.update_ops + self.delete_ops + self.list_ops
    }

    /// Get the overall average time per operation across all operation types
    pub fn overall_avg_time(&self) -> f64 {
        let total_time = (self.create_ops as f64 * self.avg_create_time)
            + (self.read_ops as f64 * self.avg_read_time)
            + (self.update_ops as f64 * self.avg_update_time)
            + (self.delete_ops as f64 * self.avg_delete_time)
            + (self.list_ops as f64 * self.avg_list_time);

        let total_ops = self.total_operations();
        if total_ops == 0 {
            0.0
        } else {
            total_time / total_ops as f64
        }
    }

    /// Calculate operations per second given elapsed time
    pub fn operations_per_second(&self, elapsed_seconds: f64) -> f64 {
        if elapsed_seconds <= 0.0 {
            0.0
        } else {
            self.total_operations() as f64 / elapsed_seconds
        }
    }

    /// Get the fastest operation type (lowest average time)
    pub fn fastest_operation(&self) -> Option<Operation> {
        let mut fastest_time = f64::INFINITY;
        let mut fastest_op = None;

        if self.create_ops > 0 && self.avg_create_time < fastest_time {
            fastest_time = self.avg_create_time;
            fastest_op = Some(Operation::Create);
        }

        if self.read_ops > 0 && self.avg_read_time < fastest_time {
            fastest_time = self.avg_read_time;
            fastest_op = Some(Operation::Read);
        }

        if self.update_ops > 0 && self.avg_update_time < fastest_time {
            fastest_time = self.avg_update_time;
            fastest_op = Some(Operation::Update);
        }

        if self.delete_ops > 0 && self.avg_delete_time < fastest_time {
            fastest_time = self.avg_delete_time;
            fastest_op = Some(Operation::Delete);
        }

        if self.list_ops > 0 && self.avg_list_time < fastest_time {
            fastest_op = Some(Operation::List);
        }

        fastest_op
    }

    /// Get the slowest operation type (highest average time)
    pub fn slowest_operation(&self) -> Option<Operation> {
        let mut slowest_time = 0.0;
        let mut slowest_op = None;

        if self.create_ops > 0 && self.avg_create_time > slowest_time {
            slowest_time = self.avg_create_time;
            slowest_op = Some(Operation::Create);
        }

        if self.read_ops > 0 && self.avg_read_time > slowest_time {
            slowest_time = self.avg_read_time;
            slowest_op = Some(Operation::Read);
        }

        if self.update_ops > 0 && self.avg_update_time > slowest_time {
            slowest_time = self.avg_update_time;
            slowest_op = Some(Operation::Update);
        }

        if self.delete_ops > 0 && self.avg_delete_time > slowest_time {
            slowest_time = self.avg_delete_time;
            slowest_op = Some(Operation::Delete);
        }

        if self.list_ops > 0 && self.avg_list_time > slowest_time {
            slowest_op = Some(Operation::List);
        }

        slowest_op
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_metrics_creation() {
        let metrics = PerformanceMetrics::new();
        let stats = metrics.get_stats();

        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.overall_avg_time(), 0.0);
        assert_eq!(stats.create_ops, 0);
        assert_eq!(stats.read_ops, 0);
        assert_eq!(stats.update_ops, 0);
        assert_eq!(stats.delete_ops, 0);
        assert_eq!(stats.list_ops, 0);
    }

    #[test]
    fn test_record_single_operation() {
        let metrics = PerformanceMetrics::new();
        let duration = Duration::from_micros(1000);

        metrics.record_operation(Operation::Create, duration);

        let stats = metrics.get_stats();
        assert_eq!(stats.create_ops, 1);
        assert_eq!(stats.avg_create_time, 1000.0);
        assert_eq!(stats.total_operations(), 1);
        assert_eq!(stats.overall_avg_time(), 1000.0);
    }

    #[test]
    fn test_record_multiple_operations() {
        let metrics = PerformanceMetrics::new();

        // Record multiple operations of different types
        metrics.record_operation(Operation::Create, Duration::from_micros(1000));
        metrics.record_operation(Operation::Read, Duration::from_micros(500));
        metrics.record_operation(Operation::Update, Duration::from_micros(1500));
        metrics.record_operation(Operation::Delete, Duration::from_micros(750));
        metrics.record_operation(Operation::List, Duration::from_micros(2000));

        let stats = metrics.get_stats();
        assert_eq!(stats.create_ops, 1);
        assert_eq!(stats.read_ops, 1);
        assert_eq!(stats.update_ops, 1);
        assert_eq!(stats.delete_ops, 1);
        assert_eq!(stats.list_ops, 1);

        assert_eq!(stats.avg_create_time, 1000.0);
        assert_eq!(stats.avg_read_time, 500.0);
        assert_eq!(stats.avg_update_time, 1500.0);
        assert_eq!(stats.avg_delete_time, 750.0);
        assert_eq!(stats.avg_list_time, 2000.0);

        assert_eq!(stats.total_operations(), 5);
        assert_eq!(stats.overall_avg_time(), 1150.0); // (1000 + 500 + 1500 + 750 + 2000) / 5
    }

    #[test]
    fn test_record_same_operation_multiple_times() {
        let metrics = PerformanceMetrics::new();

        // Record multiple read operations
        metrics.record_operation(Operation::Read, Duration::from_micros(1000));
        metrics.record_operation(Operation::Read, Duration::from_micros(2000));
        metrics.record_operation(Operation::Read, Duration::from_micros(3000));

        let stats = metrics.get_stats();
        assert_eq!(stats.read_ops, 3);
        assert_eq!(stats.avg_read_time, 2000.0); // (1000 + 2000 + 3000) / 3
        assert_eq!(stats.total_operations(), 3);
        assert_eq!(stats.overall_avg_time(), 2000.0);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = PerformanceMetrics::new();

        // Record some operations
        metrics.record_operation(Operation::Create, Duration::from_micros(1000));
        metrics.record_operation(Operation::Read, Duration::from_micros(500));

        let stats = metrics.get_stats();
        assert_eq!(stats.total_operations(), 2);

        // Reset metrics
        metrics.reset();

        let stats = metrics.get_stats();
        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.avg_create_time, 0.0);
        assert_eq!(stats.avg_read_time, 0.0);
    }

    #[test]
    fn test_concurrent_metrics_recording() {
        let metrics = Arc::new(PerformanceMetrics::new());
        let mut handles = vec![];

        // Spawn multiple threads recording operations
        for i in 0..10 {
            let metrics_clone = metrics.clone();
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    metrics_clone
                        .record_operation(Operation::Read, Duration::from_micros(100 + i * 10));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Check that all operations were recorded
        let stats = metrics.get_stats();
        assert_eq!(stats.read_ops, 100); // 10 threads * 10 operations each
        assert_eq!(stats.total_operations(), 100);
    }

    #[test]
    fn test_metrics_snapshot_calculations() {
        let metrics = PerformanceMetrics::new();

        // Record operations with known durations
        metrics.record_operation(Operation::Create, Duration::from_micros(1000));
        metrics.record_operation(Operation::Read, Duration::from_micros(500));
        metrics.record_operation(Operation::Update, Duration::from_micros(1500));

        let stats = metrics.get_stats();

        // Test total operations
        assert_eq!(stats.total_operations(), 3);

        // Test overall average time
        assert_eq!(stats.overall_avg_time(), 1000.0); // (1000 + 500 + 1500) / 3

        // Test operations per second
        assert_eq!(stats.operations_per_second(1.0), 3.0);
        assert_eq!(stats.operations_per_second(0.5), 6.0);
        assert_eq!(stats.operations_per_second(0.0), 0.0);

        // Test fastest operation
        let fastest = stats.fastest_operation();
        assert!(matches!(fastest, Some(Operation::Read)));

        // Test slowest operation
        let slowest = stats.slowest_operation();
        assert!(matches!(slowest, Some(Operation::Update)));
    }

    #[test]
    fn test_empty_metrics_snapshot() {
        let metrics = PerformanceMetrics::new();
        let stats = metrics.get_stats();

        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.overall_avg_time(), 0.0);
        assert_eq!(stats.operations_per_second(1.0), 0.0);
        assert!(stats.fastest_operation().is_none());
        assert!(stats.slowest_operation().is_none());
    }

    #[test]
    fn test_metrics_with_zero_duration() {
        let metrics = PerformanceMetrics::new();

        // Record operation with zero duration
        metrics.record_operation(Operation::Create, Duration::from_micros(0));

        let stats = metrics.get_stats();
        assert_eq!(stats.create_ops, 1);
        assert_eq!(stats.avg_create_time, 0.0);
        assert_eq!(stats.total_operations(), 1);
        assert_eq!(stats.overall_avg_time(), 0.0);
    }

    #[test]
    fn test_metrics_with_very_large_duration() {
        let metrics = PerformanceMetrics::new();

        // Record operation with very large duration
        let large_duration = Duration::from_secs(1); // 1 second = 1,000,000 microseconds
        metrics.record_operation(Operation::Create, large_duration);

        let stats = metrics.get_stats();
        assert_eq!(stats.create_ops, 1);
        assert_eq!(stats.avg_create_time, 1_000_000.0);
        assert_eq!(stats.total_operations(), 1);
        assert_eq!(stats.overall_avg_time(), 1_000_000.0);
    }

    #[test]
    fn test_mixed_operation_performance_analysis() {
        let metrics = PerformanceMetrics::new();

        // Record a realistic mix of operations
        for _ in 0..100 {
            metrics.record_operation(Operation::Read, Duration::from_micros(50));
        }

        for _ in 0..50 {
            metrics.record_operation(Operation::Create, Duration::from_micros(200));
        }

        for _ in 0..25 {
            metrics.record_operation(Operation::Update, Duration::from_micros(150));
        }

        for _ in 0..10 {
            metrics.record_operation(Operation::Delete, Duration::from_micros(100));
        }

        for _ in 0..5 {
            metrics.record_operation(Operation::List, Duration::from_micros(500));
        }

        let stats = metrics.get_stats();

        // Verify operation counts
        assert_eq!(stats.read_ops, 100);
        assert_eq!(stats.create_ops, 50);
        assert_eq!(stats.update_ops, 25);
        assert_eq!(stats.delete_ops, 10);
        assert_eq!(stats.list_ops, 5);
        assert_eq!(stats.total_operations(), 190);

        // Verify average times
        assert_eq!(stats.avg_read_time, 50.0);
        assert_eq!(stats.avg_create_time, 200.0);
        assert_eq!(stats.avg_update_time, 150.0);
        assert_eq!(stats.avg_delete_time, 100.0);
        assert_eq!(stats.avg_list_time, 500.0);

        // Verify performance analysis
        let fastest = stats.fastest_operation();
        assert!(matches!(fastest, Some(Operation::Read)));

        let slowest = stats.slowest_operation();
        assert!(matches!(slowest, Some(Operation::List)));

        // Calculate expected overall average
        let expected_avg =
            ((100.0 * 50.0) + (50.0 * 200.0) + (25.0 * 150.0) + (10.0 * 100.0) + (5.0 * 500.0))
                / 190.0;
        assert_eq!(stats.overall_avg_time(), expected_avg);
    }

    #[test]
    fn test_default_implementation() {
        let metrics = PerformanceMetrics::default();
        let stats = metrics.get_stats();

        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.overall_avg_time(), 0.0);
    }
}
