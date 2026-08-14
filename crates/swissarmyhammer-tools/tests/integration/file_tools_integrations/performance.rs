//! The memory a file operation costs.
//!
//! A full read, a windowed read, a large write and a large edit, each
//! measured with the memory profiler the parent module carries.

use super::*;

// ============================================================================
// Performance Benchmarking Tests
// ============================================================================

/// Times the base sentence repeats inside one block of the read fixture.
const READ_FIXTURE_SENTENCES_PER_BLOCK: usize = 20;

/// Blocks the read fixture holds.
const READ_FIXTURE_BLOCKS: usize = 1000;

/// Memory a full read may add, as a multiple of the file it reads.
const FULL_READ_MEMORY_BUDGET_MULTIPLE: usize = 3;

/// First line the windowed read asks for.
const WINDOWED_READ_OFFSET: usize = 500;

/// Lines the windowed read asks for.
const WINDOWED_READ_LIMIT: usize = 100;

/// Bytes one line is assumed to cost when the windowed read budgets memory.
const ESTIMATED_BYTES_PER_LINE: usize = 100;

/// Memory a windowed read may add, as a multiple of the window it reads.
const WINDOWED_READ_MEMORY_BUDGET_MULTIPLE: usize = 10;

/// Times the base sentence repeats inside one section of the write fixture.
const WRITE_FIXTURE_SENTENCES_PER_SECTION: usize = 100;

/// Sections the write fixture holds.
const WRITE_FIXTURE_SECTIONS: usize = 1000;

/// Memory a large write may add, as a multiple of the content it writes.
const WRITE_MEMORY_BUDGET_MULTIPLE: usize = 2;

/// Lines one block of the edit fixture holds.
const EDIT_FIXTURE_LINES_PER_BLOCK: usize = 5000;

/// Blocks the edit fixture holds. The product with
/// [`EDIT_FIXTURE_LINES_PER_BLOCK`] stays under the 10MB write limit.
const EDIT_FIXTURE_BLOCKS: usize = 40;

/// Memory one edit may add, as a multiple of the file it edits.
const SINGLE_EDIT_MEMORY_BUDGET_MULTIPLE: usize = 2;

/// Memory a replace-all edit may add, as a multiple of the file it edits.
const REPLACE_ALL_MEMORY_BUDGET_MULTIPLE: usize = 3;

/// File operations the concurrent memory test runs at the same time.
const CONCURRENT_FILE_OPERATIONS: usize = 20;

/// Times the base line repeats in each file the concurrent memory test writes.
const CONCURRENT_FILE_LINE_REPEATS: usize = 1000;

/// Memory `CONCURRENT_FILE_OPERATIONS` operations may add together.
const CONCURRENT_OPERATIONS_MEMORY_BUDGET: usize = 50_000_000;

/// Build the large file the two read memory tests profile against.
fn read_memory_fixture() -> String {
    let chunk = "Memory usage test content with realistic data patterns. "
        .repeat(READ_FIXTURE_SENTENCES_PER_BLOCK);
    let mut content = String::new();
    for i in 0..READ_FIXTURE_BLOCKS {
        content.push_str(&format!("Block {}: {}", i, chunk));
    }
    content
}

#[tokio::test]
async fn test_full_file_read_memory_usage() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let large_file = &temp_dir.join("memory_test_file.txt");

    let content = read_memory_fixture();

    println!(
        "Creating {} file for memory profiling...",
        MemoryProfiler::format_bytes(content.len())
    );
    let write_result = fs::write(large_file, &content);
    if let Err(ref e) = write_result {
        println!("fs::write error: {:?}", e);
    }
    write_result.unwrap();

    println!("File exists: {}", large_file.exists());
    println!("File path: {}", large_file.to_string_lossy());

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("read file"));
    arguments.insert("path".to_string(), json!(large_file.to_string_lossy()));

    println!("Reading file with memory profiling...");
    let (result, delta) = profile_memory(|| read_tool.execute(arguments, &context)).await;

    match &result {
        Ok(r) => println!(
            "Read tool success: response has {} content items",
            r.content.len()
        ),
        Err(e) => panic!("Read tool error: {}", e),
    }

    if let Some(delta) = delta {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta during read: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let file_size = content.len();
        let max_expected_memory = file_size * FULL_READ_MEMORY_BUDGET_MULTIPLE;

        assert!(
            abs_delta < max_expected_memory,
            "Memory usage {} exceeds expected maximum {}",
            MemoryProfiler::format_bytes(abs_delta),
            MemoryProfiler::format_bytes(max_expected_memory)
        );
    } else {
        println!("Memory profiling not available on this platform");
    }
}

#[tokio::test]
async fn test_offset_limit_read_memory_usage() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let large_file = &temp_dir.join("memory_test_file.txt");

    let content = read_memory_fixture();

    fs::write(large_file, &content).unwrap();

    let mut offset_args = serde_json::Map::new();
    offset_args.insert("op".to_string(), json!("read file"));
    offset_args.insert("path".to_string(), json!(large_file.to_string_lossy()));
    offset_args.insert("offset".to_string(), json!(WINDOWED_READ_OFFSET));
    offset_args.insert("limit".to_string(), json!(WINDOWED_READ_LIMIT));

    let (result, delta) = profile_memory(|| read_tool.execute(offset_args, &context)).await;
    assert!(result.is_ok());

    if let Some(delta) = delta {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for offset/limit read: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let window_size = WINDOWED_READ_LIMIT * ESTIMATED_BYTES_PER_LINE;
        let max_expected_memory = window_size * WINDOWED_READ_MEMORY_BUDGET_MULTIPLE;

        assert!(
            abs_delta < max_expected_memory,
            "Offset/limit memory usage {} exceeds expected maximum {}",
            MemoryProfiler::format_bytes(abs_delta),
            MemoryProfiler::format_bytes(max_expected_memory)
        );
    }
}

#[tokio::test]
async fn test_large_file_write_memory_usage() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let large_file = &temp_dir.join("memory_write_test.txt");

    // Generate content for memory testing (under 10MB limit)
    let chunk = "Memory profiling write test content with varied patterns. "
        .repeat(WRITE_FIXTURE_SENTENCES_PER_SECTION);
    let mut content = String::new();
    for i in 0..WRITE_FIXTURE_SECTIONS {
        content.push_str(&format!("Section {}: {}", i, chunk));
    }

    println!(
        "Testing write memory usage for {} file...",
        MemoryProfiler::format_bytes(content.len())
    );

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let (result, delta) = profile_memory(|| write_tool.execute(arguments, &context)).await;
    assert!(result.is_ok());

    if let Some(delta) = delta {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta during write: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let content_size = content.len();
        let max_expected_memory = content_size * WRITE_MEMORY_BUDGET_MULTIPLE;

        assert!(
            abs_delta < max_expected_memory,
            "Write memory usage {} exceeds expected maximum {}",
            MemoryProfiler::format_bytes(abs_delta),
            MemoryProfiler::format_bytes(max_expected_memory)
        );
    } else {
        println!("Memory profiling not available on this platform");
    }

    // Verify file was written correctly
    assert!(large_file.exists());
    let written_size = fs::metadata(large_file).unwrap().len() as usize;
    assert!(
        written_size >= content.len(),
        "Written file should match content size"
    );
}

#[tokio::test]
async fn test_large_file_edit_memory_usage() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();
    let edit_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let large_file = &temp_dir.join("memory_edit_test.txt");

    // Create file with repeated patterns for editing
    let base_pattern =
        "MEMORY_TEST_PATTERN: original_content_here\n".repeat(EDIT_FIXTURE_LINES_PER_BLOCK);
    let content = base_pattern.repeat(EDIT_FIXTURE_BLOCKS);

    println!(
        "Creating file with {} lines for edit memory testing...",
        content.lines().count()
    );

    // Write the large file
    let mut write_args = serde_json::Map::new();
    write_args.insert("op".to_string(), json!("write file"));
    write_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));
    write_tool.execute(write_args, &context).await.unwrap();

    // Test single edit memory usage
    let mut edit_args = serde_json::Map::new();
    edit_args.insert("op".to_string(), json!("edit file"));
    edit_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    edit_args.insert(
        "old_string".to_string(),
        json!("MEMORY_TEST_PATTERN: original_content_here"),
    );
    edit_args.insert(
        "new_string".to_string(),
        json!("MEMORY_TEST_PATTERN: modified_content_here"),
    );
    edit_args.insert("replace_all".to_string(), json!(false));

    let (result, delta) = profile_memory(|| edit_tool.execute(edit_args, &context)).await;
    assert!(result.is_ok());

    if let Some(delta) = delta {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for single edit: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let file_size = fs::metadata(large_file).unwrap().len() as usize;
        let max_expected_memory = file_size * SINGLE_EDIT_MEMORY_BUDGET_MULTIPLE;

        assert!(
            abs_delta < max_expected_memory,
            "Single edit memory usage {} exceeds expected maximum {}",
            MemoryProfiler::format_bytes(abs_delta),
            MemoryProfiler::format_bytes(max_expected_memory)
        );
    }

    // Test replace_all memory usage
    let mut edit_all_args = serde_json::Map::new();
    edit_all_args.insert("op".to_string(), json!("edit file"));
    edit_all_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    edit_all_args.insert("old_string".to_string(), json!("original_content_here"));
    edit_all_args.insert(
        "new_string".to_string(),
        json!("completely_new_content_here"),
    );
    edit_all_args.insert("replace_all".to_string(), json!(true));

    let (result, delta) = profile_memory(|| edit_tool.execute(edit_all_args, &context)).await;
    assert!(result.is_ok());

    if let Some(delta) = delta {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for replace_all: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let file_size = fs::metadata(large_file).unwrap().len() as usize;
        let max_expected_memory = file_size * REPLACE_ALL_MEMORY_BUDGET_MULTIPLE;

        assert!(
            abs_delta < max_expected_memory,
            "Replace_all memory usage {} exceeds expected maximum {}",
            MemoryProfiler::format_bytes(abs_delta),
            MemoryProfiler::format_bytes(max_expected_memory)
        );
    } else {
        println!("Memory profiling not available on this platform");
    }
}

#[tokio::test]
async fn test_concurrent_operations_memory_usage() {
    let registry = Arc::new(create_test_registry().await);
    let context = Arc::new(create_test_context().await);

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    println!("Testing memory usage during concurrent file operations...");

    let (success_count, delta) = profile_memory(|| async {
        // Create multiple files for concurrent operations
        let mut join_set = tokio::task::JoinSet::new();

        for i in 0..CONCURRENT_FILE_OPERATIONS {
            let registry_clone = registry.clone();
            let context_clone = context.clone();
            let temp_dir_path = temp_dir.clone();

            join_set.spawn(async move {
                let file_path = temp_dir_path.join(format!("concurrent_file_{}.txt", i));

                // Generate content for each file
                let content = format!("Concurrent test content for file {}\n", i)
                    .repeat(CONCURRENT_FILE_LINE_REPEATS);

                // Write file
                let write_tool = registry_clone.get_tool("files").unwrap();
                let mut write_args = serde_json::Map::new();
                write_args.insert("op".to_string(), json!("write file"));
                write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
                write_args.insert("content".to_string(), json!(content));

                let write_result = write_tool.execute(write_args, &context_clone).await;

                // Read file back
                let read_tool = registry_clone.get_tool("files").unwrap();
                let mut read_args = serde_json::Map::new();
                read_args.insert("op".to_string(), json!("read file"));
                read_args.insert("path".to_string(), json!(file_path.to_string_lossy()));

                let read_result = read_tool.execute(read_args, &context_clone).await;

                (write_result, read_result)
            });
        }

        // Wait for all operations to complete
        let mut success_count = 0;
        while let Some(result) = join_set.join_next().await {
            if let (Ok(_), Ok(_)) = result.unwrap() {
                success_count += 1;
            }
        }
        success_count
    })
    .await;

    if let Some(delta) = delta {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for {} concurrent operations: {} ({})",
            success_count,
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        // Concurrent operations should not cause excessive memory usage
        // Allow reasonable overhead for tokio runtime and file handles
        let max_expected_memory = CONCURRENT_OPERATIONS_MEMORY_BUDGET;

        assert!(
            abs_delta < max_expected_memory,
            "Concurrent operations memory usage {} exceeds expected maximum {}",
            MemoryProfiler::format_bytes(abs_delta),
            MemoryProfiler::format_bytes(max_expected_memory)
        );
    } else {
        println!("Memory profiling not available on this platform");
    }

    assert_eq!(
        success_count, CONCURRENT_FILE_OPERATIONS,
        "All concurrent operations should succeed"
    );
}
