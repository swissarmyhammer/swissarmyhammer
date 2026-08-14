//! Many operations against one registry at the same time.
//!
//! The stress harness this module carries spawns reads, writes, edits and
//! globs together, and the tests then assert that every one of them
//! completes and that the shared file stays whole.

use super::*;

/// Write operations the safety test runs against one shared file.
const SAFETY_WRITE_OPERATIONS: usize = 5;

/// Read operations the safety test runs against one shared file.
const SAFETY_READ_OPERATIONS: usize = 5;

/// Write operations that share one content value, so the safety test also
/// covers two tasks racing on identical bytes.
const SAFETY_WRITERS_PER_CONTENT: usize = 2;

/// Operations the high concurrency stress test runs at the same time.
const STRESS_TEST_CONCURRENCY: usize = 100;

/// Operations of [`STRESS_TEST_CONCURRENCY`] that have to succeed.
const STRESS_TEST_MIN_SUCCESSES: usize = 90;

/// Files the stress test has to leave on disk.
const STRESS_TEST_MIN_FILES: usize = 90;

/// Time the whole stress test may take.
const STRESS_TEST_TIME_BUDGET: std::time::Duration = std::time::Duration::from_secs(120);

/// Memory [`STRESS_TEST_CONCURRENCY`] operations may add together.
const STRESS_TEST_MEMORY_BUDGET: usize = 200_000_000;

/// Times the base line repeats in the smallest file the stress test writes.
const STRESS_BASE_LINE_REPEATS: usize = 1000;

/// File sizes the stress test cycles through.
const STRESS_FILE_SIZE_VARIANTS: usize = 10;

/// Line repeats each successive stress file size adds.
const STRESS_FILE_SIZE_STEP: usize = 500;

/// Files the mixed operation test creates up front for its readers and
/// editors to share.
const MIXED_BASE_FILES: usize = 20;

/// Times the base line repeats in each file the mixed operation test creates
/// up front.
const MIXED_BASE_FILE_LINE_REPEATS: usize = 100;

/// Write operations the mixed operation test spawns.
const MIXED_WRITE_OPERATIONS: usize = 30;

/// Read operations the mixed operation test spawns.
const MIXED_READ_OPERATIONS: usize = 30;

/// Edit operations the mixed operation test spawns.
const MIXED_EDIT_OPERATIONS: usize = 30;

/// Glob operations the mixed operation test spawns.
const MIXED_GLOB_OPERATIONS: usize = 20;

/// Operations the mixed operation test spawns across every kind.
const MIXED_TOTAL_OPERATIONS: usize =
    MIXED_WRITE_OPERATIONS + MIXED_READ_OPERATIONS + MIXED_EDIT_OPERATIONS + MIXED_GLOB_OPERATIONS;

/// Operations of [`MIXED_TOTAL_OPERATIONS`] that may fail.
const MIXED_MAX_ERRORS: usize = 10;

/// Time the whole mixed operation test may take.
const MIXED_TIME_BUDGET: std::time::Duration = std::time::Duration::from_secs(60);

/// Times the base line repeats in the smallest file a mixed write operation
/// creates.
const MIXED_NEW_FILE_MIN_LINE_REPEATS: usize = 50;

/// File sizes a mixed write operation cycles through.
const MIXED_NEW_FILE_SIZE_VARIANTS: usize = 50;

/// Glob patterns the mixed operation test cycles through.
const GLOB_PATTERNS: &[&str] = &["*.txt", "base_*.txt", "new_file_*.txt", "**/*.txt"];

/// Read operations the shared access test spawns.
const ACCESS_READ_OPERATIONS: usize = 50;

/// Write operations the shared access test spawns.
const ACCESS_WRITE_OPERATIONS: usize = 25;

/// Grep operations the shared access test spawns.
const ACCESS_GREP_OPERATIONS: usize = 25;

/// Operations the shared access test spawns across every kind.
const ACCESS_TOTAL_OPERATIONS: usize =
    ACCESS_READ_OPERATIONS + ACCESS_WRITE_OPERATIONS + ACCESS_GREP_OPERATIONS;

/// Lines the file every shared access operation touches starts with.
const ACCESS_SHARED_FILE_LINES: usize = 1000;

/// Times the base line repeats in each file a shared access write creates.
const ACCESS_NEW_FILE_LINE_REPEATS: usize = 100;

/// Time the whole shared access test may take.
const ACCESS_TIME_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

/// One shared file read in this many asks for a window instead of the whole
/// file.
const WINDOWED_READ_EVERY: usize = 3;

/// Lines each successive windowed read moves its window down the file.
const WINDOWED_READ_OFFSET_STEP: usize = 100;

/// Lines a windowed read asks for.
const WINDOWED_READ_LIMIT: usize = 500;

/// Grep patterns the shared access test cycles through.
const GREP_PATTERNS: &[&str] = &["SHARED_FILE_CONTENT", "initial data"];

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
            let content_size =
                STRESS_BASE_LINE_REPEATS + (i % STRESS_FILE_SIZE_VARIANTS) * STRESS_FILE_SIZE_STEP;
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
        success_count >= STRESS_TEST_MIN_SUCCESSES,
        "At least {} of {} operations should succeed, got {}",
        STRESS_TEST_MIN_SUCCESSES,
        STRESS_TEST_CONCURRENCY,
        success_count
    );
    assert!(
        total_duration < STRESS_TEST_TIME_BUDGET,
        "High concurrency test should complete within {:?}",
        STRESS_TEST_TIME_BUDGET
    );

    let files_created = std::fs::read_dir(temp_dir_path).unwrap().count();
    assert!(
        files_created >= STRESS_TEST_MIN_FILES,
        "Should create at least {} files, created {}",
        STRESS_TEST_MIN_FILES,
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
            let content = format!("New file content {}\n", i)
                .repeat(MIXED_NEW_FILE_MIN_LINE_REPEATS + i % MIXED_NEW_FILE_SIZE_VARIANTS);

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

            let pattern = GLOB_PATTERNS[i % GLOB_PATTERNS.len()];

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
        success_count >= MIXED_TOTAL_OPERATIONS - MIXED_MAX_ERRORS,
        "At least {} of {} operations should succeed, got {}",
        MIXED_TOTAL_OPERATIONS - MIXED_MAX_ERRORS,
        MIXED_TOTAL_OPERATIONS,
        success_count
    );
    assert!(
        error_count <= MIXED_MAX_ERRORS,
        "Should have at most {} errors, got {}",
        MIXED_MAX_ERRORS,
        error_count
    );
    assert!(
        total_duration < MIXED_TIME_BUDGET,
        "Mixed operations should complete within {:?}",
        MIXED_TIME_BUDGET
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

            if i % WINDOWED_READ_EVERY == 0 {
                read_args.insert("offset".to_string(), json!(i * WINDOWED_READ_OFFSET_STEP));
                read_args.insert("limit".to_string(), json!(WINDOWED_READ_LIMIT));
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
            let content =
                format!("Concurrent write operation {}\n", i).repeat(ACCESS_NEW_FILE_LINE_REPEATS);

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

            let pattern = GREP_PATTERNS[i % GREP_PATTERNS.len()];

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
                &format!("content from task {}", i / SAFETY_WRITERS_PER_CONTENT),
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

    let (write_success, write_error) = run_concurrent_test(
        registry.clone(),
        context.clone(),
        SAFETY_WRITE_OPERATIONS,
        write_op,
    )
    .await;
    let (read_success, read_error) =
        run_concurrent_test(registry, context, SAFETY_READ_OPERATIONS, read_op).await;

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
        success_count + error_count == SAFETY_WRITE_OPERATIONS + SAFETY_READ_OPERATIONS,
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

    println!(
        "Running high concurrency stress test with {} simultaneous operations...",
        STRESS_TEST_CONCURRENCY
    );

    let operation = create_stress_test_operation(temp_dir_arc);

    let start_time = std::time::Instant::now();
    let ((success_count, error_count), delta) = profile_memory(|| {
        run_concurrent_test(registry, context, STRESS_TEST_CONCURRENCY, operation)
    })
    .await;
    let total_duration = start_time.elapsed();

    if let Some(delta) = delta {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for {} concurrent operations: {} ({})",
            STRESS_TEST_CONCURRENCY,
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let max_expected_memory = STRESS_TEST_MEMORY_BUDGET;

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

    for i in 0..MIXED_BASE_FILES {
        let file_path = &temp_dir.join(format!("base_file_{}.txt", i));
        let content = format!("Base content for file {} that can be edited\n", i)
            .repeat(MIXED_BASE_FILE_LINE_REPEATS);
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
        MIXED_WRITE_OPERATIONS,
    );
    spawn_read_operations(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        MIXED_READ_OPERATIONS,
        MIXED_BASE_FILES,
    );
    spawn_edit_operations(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        MIXED_EDIT_OPERATIONS,
        MIXED_BASE_FILES,
    );
    spawn_glob_operations(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        MIXED_GLOB_OPERATIONS,
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

    let initial_content = "SHARED_FILE_CONTENT: initial data\n".repeat(ACCESS_SHARED_FILE_LINES);
    std::fs::write(shared_file, &initial_content).unwrap();

    let start_time = std::time::Instant::now();
    let mut join_set = tokio::task::JoinSet::new();

    let temp_dir_path = &temp_dir.to_path_buf();
    spawn_concurrent_reads(
        &mut join_set,
        registry.clone(),
        context.clone(),
        shared_file.clone(),
        ACCESS_READ_OPERATIONS,
    );
    spawn_concurrent_writes(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        ACCESS_WRITE_OPERATIONS,
    );
    spawn_concurrent_greps(
        &mut join_set,
        registry.clone(),
        context.clone(),
        temp_dir_path.clone(),
        ACCESS_GREP_OPERATIONS,
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
        success_count, ACCESS_TOTAL_OPERATIONS,
        "All {} concurrent operations should succeed",
        ACCESS_TOTAL_OPERATIONS
    );
    assert_eq!(error_count, 0, "Should have no errors");
    assert!(
        total_duration < ACCESS_TIME_BUDGET,
        "Concurrent access should complete within {:?}",
        ACCESS_TIME_BUDGET
    );

    assert!(shared_file.exists());
    let final_content = std::fs::read_to_string(shared_file).unwrap();
    assert!(
        !final_content.is_empty(),
        "Shared file should still have content"
    );
}
