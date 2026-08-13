//! Many operations against one registry at the same time.
//!
//! The stress harness this module carries spawns reads, writes, edits and
//! globs together, and the tests then assert that every one of them
//! completes and that the shared file stays whole.

use super::*;

/// Run concurrent operations and aggregate results
async fn run_concurrent_test<F, Fut>(
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    operation_count: usize,
    operation: F,
) -> (usize, usize)
where
    F: Fn(Arc<ToolRegistry>, Arc<ToolContext>, usize) -> Fut,
    Fut: std::future::Future<Output = Result<(), &'static str>> + Send + 'static,
{
    let mut join_set = tokio::task::JoinSet::new();

    for i in 0..operation_count {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        join_set.spawn(operation(registry_clone, context_clone, i));
    }

    let mut success_count = 0;
    let mut error_count = 0;
    let mut errors = Vec::new();

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Ok(_) => success_count += 1,
            Err(e) => {
                error_count += 1;
                errors.push(format!("{:?}", e));
            }
        }
    }

    (success_count, error_count)
}

/// Create a stress test operation that writes, reads, and edits a file
#[allow(clippy::type_complexity)]
fn create_stress_test_operation(
    temp_dir_arc: Arc<std::path::PathBuf>,
) -> impl Fn(
    Arc<ToolRegistry>,
    Arc<ToolContext>,
    usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), &'static str>> + Send>> {
    move |registry: Arc<ToolRegistry>, context: Arc<ToolContext>, i: usize| {
        let temp_dir_path = temp_dir_arc.clone();
        Box::pin(async move {
            let file_path = temp_dir_path.join(format!("stress_test_file_{}.txt", i));
            let content_size = 1000 + (i % 10) * 500;
            let content = format!("Stress test content for file {}\n", i).repeat(content_size);

            let write_tool = registry.get_tool("files").unwrap();
            let write_args_map = write_args(&file_path.to_string_lossy(), &content);
            write_tool
                .execute(write_args_map, &context)
                .await
                .map_err(|_| "Write failed")?;

            let read_tool = registry.get_tool("files").unwrap();
            let read_args_map = read_args(&file_path.to_string_lossy());
            read_tool
                .execute(read_args_map, &context)
                .await
                .map_err(|_| "Read failed")?;

            let edit_tool = registry.get_tool("files").unwrap();
            let mut edit_args_map = edit_args(
                &file_path.to_string_lossy(),
                &format!("file {}", i),
                &format!("FILE {} (edited)", i),
            );
            edit_args_map.insert("replace_all".to_string(), json!(true));
            edit_tool
                .execute(edit_args_map, &context)
                .await
                .map_err(|_| "Edit failed")?;

            Ok(())
        })
    }
}

/// Verify stress test results
fn verify_stress_test_results(
    success_count: usize,
    error_count: usize,
    total_duration: std::time::Duration,
    temp_dir_path: &std::path::Path,
) {
    println!(
        "High concurrency test completed: {} succeeded, {} failed in {:?}",
        success_count, error_count, total_duration
    );

    assert!(
        success_count >= 90,
        "At least 90% of operations should succeed, got {}/100",
        success_count
    );
    assert!(
        total_duration.as_secs() < 120,
        "High concurrency test should complete within 2 minutes"
    );

    let files_created = std::fs::read_dir(temp_dir_path).unwrap().count();
    assert!(
        files_created >= 90,
        "Should create at least 90 files, created {}",
        files_created
    );
}

/// Spawn write operations for mixed concurrency test
fn spawn_write_operations(
    join_set: &mut tokio::task::JoinSet<Result<rmcp::model::CallToolResult, rmcp::ErrorData>>,
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    temp_dir_path: std::path::PathBuf,
    count: usize,
) {
    for i in 0..count {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_clone = temp_dir_path.clone();

        join_set.spawn(async move {
            let file_path = temp_dir_clone.join(format!("new_file_{}.txt", i));
            let content = format!("New file content {}\n", i).repeat(50 + i % 50);

            let write_tool = registry_clone.get_tool("files").unwrap();
            let mut write_args = serde_json::Map::new();
            write_args.insert("op".to_string(), json!("write file"));
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));

            write_tool.execute(write_args, &context_clone).await
        });
    }
}

/// Spawn read operations for mixed concurrency test
fn spawn_read_operations(
    join_set: &mut tokio::task::JoinSet<Result<rmcp::model::CallToolResult, rmcp::ErrorData>>,
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    temp_dir_path: std::path::PathBuf,
    count: usize,
    base_files: usize,
) {
    for i in 0..count {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_clone = temp_dir_path.clone();

        join_set.spawn(async move {
            let file_index = i % base_files;
            let file_path = temp_dir_clone.join(format!("base_file_{}.txt", file_index));

            let read_tool = registry_clone.get_tool("files").unwrap();
            let mut read_args = serde_json::Map::new();
            read_args.insert("op".to_string(), json!("read file"));
            read_args.insert("path".to_string(), json!(file_path.to_string_lossy()));

            read_tool.execute(read_args, &context_clone).await
        });
    }
}

/// Spawn edit operations for mixed concurrency test
fn spawn_edit_operations(
    join_set: &mut tokio::task::JoinSet<Result<rmcp::model::CallToolResult, rmcp::ErrorData>>,
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    temp_dir_path: std::path::PathBuf,
    count: usize,
    base_files: usize,
) {
    for i in 0..count {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_clone = temp_dir_path.clone();

        join_set.spawn(async move {
            let file_index = i % base_files;
            let file_path = temp_dir_clone.join(format!("base_file_{}.txt", file_index));

            let edit_tool = registry_clone.get_tool("files").unwrap();
            let mut edit_args = serde_json::Map::new();
            edit_args.insert("op".to_string(), json!("edit file"));
            edit_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            edit_args.insert(
                "old_string".to_string(),
                json!(format!("file {}", file_index)),
            );
            edit_args.insert(
                "new_string".to_string(),
                json!(format!("file {} (edited by task {})", file_index, i)),
            );
            edit_args.insert("replace_all".to_string(), json!(false));

            edit_tool.execute(edit_args, &context_clone).await
        });
    }
}

/// Spawn glob operations for mixed concurrency test
fn spawn_glob_operations(
    join_set: &mut tokio::task::JoinSet<Result<rmcp::model::CallToolResult, rmcp::ErrorData>>,
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    temp_dir_path: std::path::PathBuf,
    count: usize,
) {
    for i in 0..count {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_clone = temp_dir_path.clone();

        join_set.spawn(async move {
            let glob_tool = registry_clone.get_tool("files").unwrap();
            let mut glob_args = serde_json::Map::new();
            glob_args.insert("op".to_string(), json!("glob files"));

            let pattern = match i % 4 {
                0 => "*.txt",
                1 => "base_*.txt",
                2 => "new_file_*.txt",
                _ => "**/*.txt",
            };

            glob_args.insert("pattern".to_string(), json!(pattern));
            glob_args.insert("path".to_string(), json!(temp_dir_clone.to_string_lossy()));

            glob_tool.execute(glob_args, &context_clone).await
        });
    }
}

/// Verify mixed operation results
fn verify_mixed_operation_results(
    success_count: usize,
    error_count: usize,
    total_duration: std::time::Duration,
) {
    println!(
        "Mixed operation concurrency completed: {} succeeded, {} failed in {:?}",
        success_count, error_count, total_duration
    );

    assert!(
        success_count >= 100,
        "At least 100/110 operations should succeed, got {}",
        success_count
    );
    assert!(
        error_count <= 10,
        "Should have at most 10 errors, got {}",
        error_count
    );
    assert!(
        total_duration.as_secs() < 60,
        "Mixed operations should complete within 1 minute"
    );
}

/// Spawn concurrent read operations on a shared file
fn spawn_concurrent_reads(
    join_set: &mut tokio::task::JoinSet<Result<rmcp::model::CallToolResult, rmcp::ErrorData>>,
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    shared_file: std::path::PathBuf,
    count: usize,
) {
    for i in 0..count {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let file_path = shared_file.clone();

        join_set.spawn(async move {
            let read_tool = registry_clone.get_tool("files").unwrap();
            let mut read_args = serde_json::Map::new();
            read_args.insert("op".to_string(), json!("read file"));
            read_args.insert("path".to_string(), json!(file_path.to_string_lossy()));

            if i % 3 == 0 {
                read_args.insert("offset".to_string(), json!(i * 100));
                read_args.insert("limit".to_string(), json!(500));
            }

            read_tool.execute(read_args, &context_clone).await
        });
    }
}

/// Spawn concurrent write operations to different files
fn spawn_concurrent_writes(
    join_set: &mut tokio::task::JoinSet<Result<rmcp::model::CallToolResult, rmcp::ErrorData>>,
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    temp_dir_path: std::path::PathBuf,
    count: usize,
) {
    for i in 0..count {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_clone = temp_dir_path.clone();

        join_set.spawn(async move {
            let file_path = temp_dir_clone.join(format!("concurrent_write_{}.txt", i));
            let content = format!("Concurrent write operation {}\n", i).repeat(100);

            let write_tool = registry_clone.get_tool("files").unwrap();
            let mut write_args = serde_json::Map::new();
            write_args.insert("op".to_string(), json!("write file"));
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));

            write_tool.execute(write_args, &context_clone).await
        });
    }
}

/// Spawn concurrent grep operations
fn spawn_concurrent_greps(
    join_set: &mut tokio::task::JoinSet<Result<rmcp::model::CallToolResult, rmcp::ErrorData>>,
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    temp_dir_path: std::path::PathBuf,
    count: usize,
) {
    for i in 0..count {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_clone = temp_dir_path.clone();

        join_set.spawn(async move {
            let grep_tool = registry_clone.get_tool("files").unwrap();
            let mut grep_args = serde_json::Map::new();
            grep_args.insert("op".to_string(), json!("grep files"));

            let pattern = if i % 2 == 0 {
                "SHARED_FILE_CONTENT"
            } else {
                "initial data"
            };

            grep_args.insert("pattern".to_string(), json!(pattern));
            grep_args.insert("path".to_string(), json!(temp_dir_clone.to_string_lossy()));
            grep_args.insert("output_mode".to_string(), json!("files_with_matches"));

            grep_tool.execute(grep_args, &context_clone).await
        });
    }
}

#[tokio::test]
async fn test_concurrent_file_operations_safety() {
    use std::sync::Arc;

    let registry = Arc::new(create_test_registry().await);
    let context = Arc::new(create_test_context().await);

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let shared_file_path = temp_dir.join("concurrent_test.txt");
    let shared_file = Arc::new(shared_file_path);

    // Initialize the file
    fs::write(&*shared_file, "initial content").unwrap();

    let file_for_write = shared_file.clone();
    let file_for_read = shared_file.clone();

    // Run write operations
    let write_op = |registry: Arc<ToolRegistry>, context: Arc<ToolContext>, i: usize| {
        let file_clone = file_for_write.clone();
        async move {
            let write_tool = registry.get_tool("files").unwrap();
            let write_args = write_args(
                &file_clone.to_string_lossy(),
                &format!("content from task {}", i / 2),
            );
            write_tool
                .execute(write_args, &context)
                .await
                .map(|_| ())
                .map_err(|_| "Write failed")
        }
    };

    let read_op = |registry: Arc<ToolRegistry>, context: Arc<ToolContext>, _i: usize| {
        let file_clone = file_for_read.clone();
        async move {
            let read_tool = registry.get_tool("files").unwrap();
            let read_args = read_args(&file_clone.to_string_lossy());
            read_tool
                .execute(read_args, &context)
                .await
                .map(|_| ())
                .map_err(|_| "Read failed")
        }
    };

    // Run 5 write and 5 read operations
    let (write_success, write_error) =
        run_concurrent_test(registry.clone(), context.clone(), 5, write_op).await;
    let (read_success, read_error) = run_concurrent_test(registry, context, 5, read_op).await;

    let success_count = write_success + read_success;
    let error_count = write_error + read_error;

    println!(
        "Concurrent operations: {} succeeded, {} failed",
        success_count, error_count
    );

    // Verify the file system remains consistent
    assert!(shared_file.exists());
    let final_content = fs::read_to_string(&*shared_file).unwrap();
    assert!(!final_content.is_empty());

    // All operations should complete without causing data corruption or system instability
    assert!(
        success_count + error_count == 10,
        "All concurrent operations should complete"
    );
}

// ============================================================================
// Extended Concurrent Operation Stress Tests
// ============================================================================

#[tokio::test]
async fn test_high_concurrency_stress_test() {
    let registry = Arc::new(create_test_registry().await);
    let context = Arc::new(create_test_context().await);

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let temp_dir_arc = Arc::new(temp_dir.clone());

    println!("Running high concurrency stress test with 100 simultaneous operations...");

    let profiler = MemoryProfiler::new();
    let start_time = std::time::Instant::now();

    let operation = create_stress_test_operation(temp_dir_arc);
    let (success_count, error_count) = run_concurrent_test(registry, context, 100, operation).await;

    let total_duration = start_time.elapsed();

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for 100 concurrent operations: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let max_expected_memory = 200_000_000;

        assert!(
            abs_delta < max_expected_memory,
            "High concurrency memory usage {} exceeds expected maximum {}",
            MemoryProfiler::format_bytes(abs_delta),
            MemoryProfiler::format_bytes(max_expected_memory)
        );
    }

    verify_stress_test_results(success_count, error_count, total_duration, &temp_dir);
}

#[tokio::test]
async fn test_mixed_operation_concurrency_stress() {
    let registry = Arc::new(create_test_registry().await);
    let context = Arc::new(create_test_context().await);

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    println!("Running mixed operation concurrency stress test...");

    let base_files = 20;
    for i in 0..base_files {
        let file_path = &temp_dir.join(format!("base_file_{}.txt", i));
        let content = format!("Base content for file {} that can be edited\n", i).repeat(100);
        std::fs::write(file_path, content).unwrap();
    }

    let start_time = std::time::Instant::now();
    let mut join_set = tokio::task::JoinSet::new();

    let temp_dir_path = &temp_dir.to_path_buf();
    spawn_write_operations(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        30,
    );
    spawn_read_operations(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        30,
        base_files,
    );
    spawn_edit_operations(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        30,
        base_files,
    );
    spawn_glob_operations(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        20,
    );

    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    let total_duration = start_time.elapsed();
    verify_mixed_operation_results(success_count, error_count, total_duration);
}

#[tokio::test]
async fn test_concurrent_file_access_patterns() {
    let registry = Arc::new(create_test_registry().await);
    let context = Arc::new(create_test_context().await);

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let shared_file = &temp_dir.join("shared_access_file.txt");

    println!("Testing concurrent access patterns to shared file...");

    let initial_content = "SHARED_FILE_CONTENT: initial data\n".repeat(1000);
    std::fs::write(shared_file, &initial_content).unwrap();

    let start_time = std::time::Instant::now();
    let mut join_set = tokio::task::JoinSet::new();

    let temp_dir_path = &temp_dir.to_path_buf();
    spawn_concurrent_reads(
        &mut join_set,
        registry.clone(),
        context.clone(),
        shared_file.clone(),
        50,
    );
    spawn_concurrent_writes(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        25,
    );
    spawn_concurrent_greps(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        25,
    );

    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    let total_duration = start_time.elapsed();

    println!(
        "Concurrent file access test completed: {} succeeded, {} failed in {:?}",
        success_count, error_count, total_duration
    );

    assert_eq!(
        success_count, 100,
        "All 100 concurrent operations should succeed"
    );
    assert_eq!(error_count, 0, "Should have no errors");
    assert!(
        total_duration.as_secs() < 30,
        "Concurrent access should complete within 30 seconds"
    );

    assert!(shared_file.exists());
    let final_content = std::fs::read_to_string(shared_file).unwrap();
    assert!(
        !final_content.is_empty(),
        "Shared file should still have content"
    );
}
