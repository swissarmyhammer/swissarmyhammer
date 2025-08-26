use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::runtime::Runtime;
use tokio::time::Duration;

use swissarmyhammer::issues::metrics::{Operation, PerformanceMetrics};
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};

fn setup_fs_storage() -> (FileSystemIssueStorage, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let issues_dir = temp_dir.path().join("issues");
    let storage = FileSystemIssueStorage::new(issues_dir).unwrap();
    (storage, temp_dir)
}

fn benchmark_sequential_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_operations");
    let rt = Runtime::new().unwrap();

    let batch_sizes = vec![5, 10, 20, 50];

    for batch_size in batch_sizes {
        // Compare individual operations vs sequential operations
        group.bench_with_input(
            BenchmarkId::new("individual_creates", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let (storage, _temp) = setup_fs_storage();

                        for i in 1..=batch_size {
                            let _issue = storage
                                .create_issue(format!("test_{i}"), format!("Content {i}"))
                                .await
                                .unwrap();
                        }
                    });
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("sequential_creates", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let (storage, _temp) = setup_fs_storage();

                        for i in 1..=batch_size {
                            let _issue = storage
                                .create_issue(format!("test_{i}"), format!("Content {i}"))
                                .await
                                .unwrap();
                        }
                    });
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("individual_gets", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let (storage, _temp) = setup_fs_storage();

                        // Pre-create issues
                        for i in 1..=batch_size {
                            let _issue = storage
                                .create_issue(format!("test_{i}"), format!("Content {i}"))
                                .await
                                .unwrap();
                        }

                        // Individual gets
                        for i in 1..=batch_size {
                            let issue_name = format!("test_{i}");
                            let _issue = storage.get_issue(&issue_name).await.unwrap();
                        }
                    });
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("sequential_gets", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let (storage, _temp) = setup_fs_storage();

                        // Pre-create issues
                        for i in 1..=batch_size {
                            let _issue = storage
                                .create_issue(format!("test_{i}"), format!("Content {i}"))
                                .await
                                .unwrap();
                        }

                        // Sequential gets
                        for i in 1..=batch_size {
                            let issue_name = format!("test_{i}");
                            let _issue = storage.get_issue(&issue_name).await.unwrap();
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

fn benchmark_metrics_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_collection");

    let operation_counts = vec![100, 1000, 10000];

    for count in operation_counts {
        group.bench_with_input(
            BenchmarkId::new("metrics_recording", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let metrics = PerformanceMetrics::new();

                    for i in 0..count {
                        let operation = match i % 5 {
                            0 => Operation::Create,
                            1 => Operation::Read,
                            2 => Operation::Update,
                            3 => Operation::Delete,
                            _ => Operation::List,
                        };

                        metrics.record_operation(
                            black_box(operation),
                            black_box(Duration::from_micros(100 + (i % 1000) as u64)),
                        );
                    }

                    let _stats = metrics.get_stats();
                });
            },
        );
    }

    group.finish();
}

fn benchmark_filesystem_storage(c: &mut Criterion) {
    let mut group = c.benchmark_group("filesystem_storage");
    let rt = Runtime::new().unwrap();

    let operation_counts = vec![10, 50, 100];

    for count in operation_counts {
        group.bench_with_input(
            BenchmarkId::new("filesystem_operations", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let (storage, _temp) = setup_fs_storage();

                        // Create issues
                        for i in 1..=count {
                            let _issue = storage
                                .create_issue(format!("test_{i}"), format!("Content {i}"))
                                .await
                                .unwrap();
                        }

                        // Read issues
                        for i in 1..=count {
                            let issue_name = format!("test_{i}");
                            let _issue = storage.get_issue(&issue_name).await.unwrap();
                        }

                        // Update issues
                        for i in 1..=count {
                            let issue_name = format!("test_{i}");
                            let _issue = storage
                                .update_issue(&issue_name, format!("Updated content {i}"))
                                .await
                                .unwrap();
                        }

                        // List issues
                        let _issues = storage.list_issues().await.unwrap();
                    });
                });
            },
        );
    }

    group.finish();
}

fn benchmark_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_operations");
    let rt = Runtime::new().unwrap();

    let concurrent_counts = vec![2, 5, 10];

    for concurrent in concurrent_counts {
        group.bench_with_input(
            BenchmarkId::new("concurrent_storage_access", concurrent),
            &concurrent,
            |b, &concurrent| {
                b.iter(|| {
                    rt.block_on(async {
                        let (storage, _temp) = setup_fs_storage();

                        // Pre-create issues
                        for i in 1..=100 {
                            let _issue = storage
                                .create_issue(format!("test_issue_{i}"), format!("Content {i}"))
                                .await
                                .unwrap();
                        }

                        // Concurrent access
                        let storage = Arc::new(storage);
                        let mut handles = Vec::new();
                        for _ in 0..concurrent {
                            let storage_clone = storage.clone();
                            let handle = tokio::spawn(async move {
                                for i in 1..=100 {
                                    let _issue = storage_clone
                                        .get_issue(&format!("test_issue_{i}"))
                                        .await
                                        .unwrap();
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    });
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("concurrent_metrics_recording", concurrent),
            &concurrent,
            |b, &concurrent| {
                b.iter(|| {
                    rt.block_on(async {
                        let metrics = Arc::new(PerformanceMetrics::new());

                        let mut handles = Vec::new();
                        for _ in 0..concurrent {
                            let metrics_clone = metrics.clone();
                            let handle = tokio::spawn(async move {
                                for i in 0..100 {
                                    let operation = match i % 5 {
                                        0 => Operation::Create,
                                        1 => Operation::Read,
                                        2 => Operation::Update,
                                        3 => Operation::Delete,
                                        _ => Operation::List,
                                    };

                                    metrics_clone.record_operation(
                                        operation,
                                        Duration::from_micros(100 + (i % 1000) as u64),
                                    );
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }

                        let _stats = metrics.get_stats();
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    issue_performance_benches,
    benchmark_sequential_operations,
    benchmark_metrics_collection,
    benchmark_filesystem_storage,
    benchmark_concurrent_operations
);

criterion_main!(issue_performance_benches);
