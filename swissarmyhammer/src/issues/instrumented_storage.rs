use super::filesystem::{Issue, IssueInfo, IssueStorage};
use super::metrics::{MetricsSnapshot, Operation, PerformanceMetrics};
use super::IssueName;
use crate::error::Result;
use async_trait::async_trait;
use tokio::time::Instant;

/// A storage wrapper that collects performance metrics for all operations
pub struct InstrumentedIssueStorage {
    storage: Box<dyn IssueStorage>,
    metrics: PerformanceMetrics,
}

impl InstrumentedIssueStorage {
    /// Create a new instrumented storage wrapper
    pub fn new(storage: Box<dyn IssueStorage>) -> Self {
        Self {
            storage,
            metrics: PerformanceMetrics::new(),
        }
    }

    /// Get access to the performance metrics collector
    pub fn metrics(&self) -> &PerformanceMetrics {
        &self.metrics
    }

    /// Get a snapshot of current performance metrics
    pub fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics.get_stats()
    }

    /// Reset all performance metrics to zero
    pub fn reset_metrics(&self) {
        self.metrics.reset();
    }
}

#[async_trait]
impl IssueStorage for InstrumentedIssueStorage {
    async fn create_issue(&self, name: String, content: String) -> Result<Issue> {
        let start = Instant::now();
        let result = self.storage.create_issue(name, content).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Create, duration);
        result
    }


    async fn get_issue(&self, name: &str) -> Result<Issue> {
        let start = Instant::now();
        let result = self.storage.get_issue(name).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Read, duration);
        result
    }

    async fn update_issue(&self, name: &str, content: String) -> Result<Issue> {
        let start = Instant::now();
        let result = self.storage.update_issue(name, content).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Update, duration);
        result
    }

    async fn mark_complete(&self, name: &str) -> Result<Issue> {
        let start = Instant::now();
        let result = self.storage.mark_complete(name).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Delete, duration);
        result
    }

    async fn list_issues(&self) -> Result<Vec<Issue>> {
        let start = Instant::now();
        let result = self.storage.list_issues().await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::List, duration);
        result
    }

    async fn list_issues_info(&self) -> Result<Vec<IssueInfo>> {
        let start = Instant::now();
        let result = self.storage.list_issues_info().await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::List, duration);
        result
    }

    async fn get_issue_info(&self, name: &str) -> Result<IssueInfo> {
        let start = Instant::now();
        let result = self.storage.get_issue_info(name).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Read, duration);
        result
    }

    async fn create_issues_batch(&self, issues: Vec<(String, String)>) -> Result<Vec<Issue>> {
        let start = Instant::now();
        let result = self.storage.create_issues_batch(issues).await;
        let duration = start.elapsed();

        // Record each create operation in the batch with per-operation time
        if let Ok(ref created_issues) = result {
            if !created_issues.is_empty() {
                let per_operation_duration = duration / created_issues.len() as u32;
                for _ in created_issues {
                    self.metrics
                        .record_operation(Operation::Create, per_operation_duration);
                }
            }
        }

        result
    }

    async fn get_issues_batch(&self, names: Vec<&str>) -> Result<Vec<Issue>> {
        let start = Instant::now();
        let result = self.storage.get_issues_batch(names).await;
        let duration = start.elapsed();

        // Record each read operation in the batch with per-operation time
        if let Ok(ref issues) = result {
            if !issues.is_empty() {
                let per_operation_duration = duration / issues.len() as u32;
                for _ in issues {
                    self.metrics
                        .record_operation(Operation::Read, per_operation_duration);
                }
            }
        }

        result
    }

    async fn update_issues_batch(&self, updates: Vec<(&str, String)>) -> Result<Vec<Issue>> {
        let start = Instant::now();
        let result = self.storage.update_issues_batch(updates).await;
        let duration = start.elapsed();

        // Record each update operation in the batch with per-operation time
        if let Ok(ref updated_issues) = result {
            if !updated_issues.is_empty() {
                let per_operation_duration = duration / updated_issues.len() as u32;
                for _ in updated_issues {
                    self.metrics
                        .record_operation(Operation::Update, per_operation_duration);
                }
            }
        }

        result
    }

    async fn mark_complete_batch(&self, names: Vec<&str>) -> Result<Vec<Issue>> {
        let start = Instant::now();
        let result = self.storage.mark_complete_batch(names).await;
        let duration = start.elapsed();

        // Record each delete operation in the batch with per-operation time
        if let Ok(ref completed_issues) = result {
            if !completed_issues.is_empty() {
                let per_operation_duration = duration / completed_issues.len() as u32;
                for _ in completed_issues {
                    self.metrics
                        .record_operation(Operation::Delete, per_operation_duration);
                }
            }
        }

        result
    }

    async fn get_next_issue(&self) -> Result<Option<Issue>> {
        let start = Instant::now();
        let result = self.storage.get_next_issue().await;
        let duration = start.elapsed();

        // Record as a read operation
        self.metrics.record_operation(Operation::Read, duration);

        result
    }

    // Type-safe implementations using IssueName

    async fn get_issue_by_name(&self, name: &IssueName) -> Result<Issue> {
        let start = Instant::now();
        let result = self.storage.get_issue_by_name(name).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Read, duration);
        result
    }

    async fn create_issue_with_name(&self, name: IssueName, content: String) -> Result<Issue> {
        let start = Instant::now();
        let result = self.storage.create_issue_with_name(name, content).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Create, duration);
        result
    }

    async fn update_issue_by_name(&self, name: &IssueName, content: String) -> Result<Issue> {
        let start = Instant::now();
        let result = self.storage.update_issue_by_name(name, content).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Update, duration);
        result
    }

    async fn mark_complete_by_name(&self, name: &IssueName) -> Result<Issue> {
        let start = Instant::now();
        let result = self.storage.mark_complete_by_name(name).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Update, duration);
        result
    }

    async fn get_issues_batch_by_name(&self, names: Vec<&IssueName>) -> Result<Vec<Issue>> {
        let start = Instant::now();
        let result = self.storage.get_issues_batch_by_name(names).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Read, duration);
        result
    }

    async fn update_issues_batch_by_name(
        &self,
        updates: Vec<(&IssueName, String)>,
    ) -> Result<Vec<Issue>> {
        let start = Instant::now();
        let result = self.storage.update_issues_batch_by_name(updates).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Update, duration);
        result
    }

    async fn mark_complete_batch_by_name(&self, names: Vec<&IssueName>) -> Result<Vec<Issue>> {
        let start = Instant::now();
        let result = self.storage.mark_complete_batch_by_name(names).await;
        let duration = start.elapsed();

        self.metrics.record_operation(Operation::Update, duration);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issues::filesystem::FileSystemIssueStorage;
    use tempfile::TempDir;

    fn create_test_storage() -> (InstrumentedIssueStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().join("issues");

        let fs_storage = FileSystemIssueStorage::new(issues_dir).unwrap();
        let instrumented_storage = InstrumentedIssueStorage::new(Box::new(fs_storage));

        (instrumented_storage, temp_dir)
    }

    #[tokio::test]
    async fn test_instrumented_storage_creation() {
        let (storage, _temp) = create_test_storage();

        // Check initial metrics
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.total_operations(), 0);
        assert_eq!(snapshot.create_ops, 0);
        assert_eq!(snapshot.read_ops, 0);
        assert_eq!(snapshot.update_ops, 0);
        assert_eq!(snapshot.delete_ops, 0);
        assert_eq!(snapshot.list_ops, 0);
    }

    #[tokio::test]
    async fn test_create_issue_records_metrics() {
        let (storage, _temp) = create_test_storage();

        // Create an issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        // Check metrics were recorded
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.create_ops, 1);
        assert_eq!(snapshot.total_operations(), 1);
        assert!(snapshot.avg_create_time > 0.0);

        // Verify the issue was actually created
        assert_eq!(issue.name.as_str(), "test_issue");
        assert_eq!(issue.content, "Test content");
    }

    #[tokio::test]
    async fn test_get_issue_records_metrics() {
        let (storage, _temp) = create_test_storage();

        // Create an issue first
        let issue = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        // Reset metrics to focus on get operation
        storage.reset_metrics();

        // Get the issue
        let retrieved_issue = storage.get_issue(issue.name.as_str()).await.unwrap();

        // Check metrics were recorded
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.read_ops, 1);
        assert_eq!(snapshot.total_operations(), 1);
        assert!(snapshot.avg_read_time > 0.0);

        // Verify the issue was actually retrieved
        assert_eq!(retrieved_issue.name, issue.name);
        assert_eq!(retrieved_issue.content, issue.content);
    }

    #[tokio::test]
    async fn test_update_issue_records_metrics() {
        let (storage, _temp) = create_test_storage();

        // Create an issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Original content".to_string())
            .await
            .unwrap();

        // Reset metrics to focus on update operation
        storage.reset_metrics();

        // Update the issue
        let updated_issue = storage
            .update_issue(issue.name.as_str(), "Updated content".to_string())
            .await
            .unwrap();

        // Check metrics were recorded
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.update_ops, 1);
        assert_eq!(snapshot.total_operations(), 1);
        assert!(snapshot.avg_update_time > 0.0);

        // Verify the issue was actually updated
        assert_eq!(updated_issue.name, issue.name);
        assert_eq!(updated_issue.content, "Updated content");
    }

    #[tokio::test]
    async fn test_mark_complete_records_metrics() {
        let (storage, _temp) = create_test_storage();

        // Create an issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        // Reset metrics to focus on mark_complete operation
        storage.reset_metrics();

        // Mark as complete
        let completed_issue = storage.mark_complete(issue.name.as_str()).await.unwrap();

        // Check metrics were recorded (mark_complete is tracked as Delete operation)
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.delete_ops, 1);
        assert_eq!(snapshot.total_operations(), 1);
        assert!(snapshot.avg_delete_time > 0.0);

        // Verify the issue was actually marked complete
        assert_eq!(completed_issue.name, issue.name);
        assert!(completed_issue.completed);
    }

    #[tokio::test]
    async fn test_list_issues_records_metrics() {
        let (storage, _temp) = create_test_storage();

        // Create multiple issues
        storage
            .create_issue("issue1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        storage
            .create_issue("issue2".to_string(), "Content 2".to_string())
            .await
            .unwrap();

        // Reset metrics to focus on list operation
        storage.reset_metrics();

        // List issues
        let issues = storage.list_issues().await.unwrap();

        // Check metrics were recorded
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.list_ops, 1);
        assert_eq!(snapshot.total_operations(), 1);
        assert!(snapshot.avg_list_time > 0.0);

        // Verify the issues were actually listed
        assert_eq!(issues.len(), 2);
    }

    #[tokio::test]
    async fn test_multiple_operations_metrics() {
        let (storage, _temp) = create_test_storage();

        // Perform multiple operations
        let issue1 = storage
            .create_issue("issue1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        let issue2 = storage
            .create_issue("issue2".to_string(), "Content 2".to_string())
            .await
            .unwrap();

        storage.get_issue(issue1.name.as_str()).await.unwrap();
        storage.get_issue(issue2.name.as_str()).await.unwrap();

        storage
            .update_issue(issue1.name.as_str(), "Updated content".to_string())
            .await
            .unwrap();

        storage.mark_complete(issue2.name.as_str()).await.unwrap();

        storage.list_issues().await.unwrap();

        // Check aggregated metrics
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.create_ops, 2);
        assert_eq!(snapshot.read_ops, 2);
        assert_eq!(snapshot.update_ops, 1);
        assert_eq!(snapshot.delete_ops, 1);
        assert_eq!(snapshot.list_ops, 1);
        assert_eq!(snapshot.total_operations(), 7);

        // All average times should be positive
        assert!(snapshot.avg_create_time > 0.0);
        assert!(snapshot.avg_read_time > 0.0);
        assert!(snapshot.avg_update_time > 0.0);
        assert!(snapshot.avg_delete_time > 0.0);
        assert!(snapshot.avg_list_time > 0.0);
        assert!(snapshot.overall_avg_time() > 0.0);
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        let (storage, _temp) = create_test_storage();

        // Perform some operations
        storage
            .create_issue("issue1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        storage.list_issues().await.unwrap();

        // Verify metrics were recorded
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.total_operations(), 2);

        // Reset metrics
        storage.reset_metrics();

        // Verify metrics are reset
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.total_operations(), 0);
        assert_eq!(snapshot.create_ops, 0);
        assert_eq!(snapshot.list_ops, 0);
        assert_eq!(snapshot.avg_create_time, 0.0);
        assert_eq!(snapshot.avg_list_time, 0.0);
    }

    #[tokio::test]
    async fn test_performance_analysis() {
        let (storage, _temp) = create_test_storage();

        // Create several issues to get more realistic timing data
        for i in 1..=5 {
            storage
                .create_issue(format!("issue_{i}"), format!("Content {i}"))
                .await
                .unwrap();
        }

        // Perform multiple read operations (should be faster than creates)
        for i in 1..=5 {
            storage.get_issue(&format!("issue_{i}")).await.unwrap();
        }

        // Perform a list operation (potentially slower)
        storage.list_issues().await.unwrap();

        let snapshot = storage.get_metrics_snapshot();

        // Verify operation counts
        assert_eq!(snapshot.create_ops, 5);
        assert_eq!(snapshot.read_ops, 5);
        assert_eq!(snapshot.list_ops, 1);
        assert_eq!(snapshot.total_operations(), 11);

        // Test operations per second calculation
        let ops_per_second = snapshot.operations_per_second(1.0);
        assert_eq!(ops_per_second, 11.0);

        // Test fastest/slowest operation analysis
        let fastest = snapshot.fastest_operation();
        let slowest = snapshot.slowest_operation();

        // Should have some operation as fastest and slowest
        assert!(fastest.is_some());
        assert!(slowest.is_some());

        // The fastest and slowest should be different unless all operations take the same time
        // (which is unlikely in practice)
        println!(
            "Fastest operation: {:?} ({}μs)",
            fastest,
            match fastest {
                Some(Operation::Create) => snapshot.avg_create_time,
                Some(Operation::Read) => snapshot.avg_read_time,
                Some(Operation::Update) => snapshot.avg_update_time,
                Some(Operation::Delete) => snapshot.avg_delete_time,
                Some(Operation::List) => snapshot.avg_list_time,
                None => 0.0,
            }
        );

        println!(
            "Slowest operation: {:?} ({}μs)",
            slowest,
            match slowest {
                Some(Operation::Create) => snapshot.avg_create_time,
                Some(Operation::Read) => snapshot.avg_read_time,
                Some(Operation::Update) => snapshot.avg_update_time,
                Some(Operation::Delete) => snapshot.avg_delete_time,
                Some(Operation::List) => snapshot.avg_list_time,
                None => 0.0,
            }
        );
    }

    #[tokio::test]
    async fn test_concurrent_operations_metrics() {
        let (storage, _temp) = create_test_storage();

        // Create an issue first
        let issue = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        // Reset metrics
        storage.reset_metrics();

        // Simulate concurrent reads using tokio::spawn
        let storage = std::sync::Arc::new(storage);
        let mut handles = vec![];

        for _ in 0..10 {
            let storage_clone = storage.clone();
            let issue_name = issue.name.clone();
            let handle = tokio::spawn(async move {
                storage_clone.get_issue(&issue_name).await.unwrap();
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Check metrics
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.read_ops, 10);
        assert_eq!(snapshot.total_operations(), 10);
        assert!(snapshot.avg_read_time > 0.0);
    }

    #[tokio::test]
    async fn test_error_handling_still_records_metrics() {
        let (storage, _temp) = create_test_storage();

        // Try to get a non-existent issue
        let result = storage.get_issue("nonexistent_issue").await;
        assert!(result.is_err());

        // Verify metrics were still recorded even though operation failed
        let snapshot = storage.get_metrics_snapshot();
        assert_eq!(snapshot.read_ops, 1);
        assert_eq!(snapshot.total_operations(), 1);
        assert!(snapshot.avg_read_time > 0.0);
    }
}
