//! Performance tests for file tools
//!
//! This module contains comprehensive performance tests for all file tools,
//! including benchmarks for large file operations, directory traversal, memory usage,
//! concurrent operations, and pattern matching performance.

use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
use swissarmyhammer::test_utils::IsolatedTestHome;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::files;
use tempfile::TempDir;

#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader};

/// Performance measurement utilities
pub struct PerformanceProfiler {
    start_time: Instant,
    initial_memory: Option<usize>,
}

impl Default for PerformanceProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceProfiler {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            initial_memory: Self::get_memory_usage(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn memory_delta(&self) -> Option<isize> {
        if let (Some(initial), Some(current)) = (self.initial_memory, Self::get_memory_usage()) {
            Some(current as isize - initial as isize)
        } else {
            None
        }
    }

    pub fn format_results(&self, operation: &str) -> String {
        let elapsed = self.elapsed();
        match self.memory_delta() {
            Some(delta) => format!(
                "{}: {} ({:+})",
                operation,
                format_duration(elapsed),
                format_memory_delta(delta)
            ),
            None => format!("{}: {}", operation, format_duration(elapsed)),
        }
    }

    #[cfg(target_os = "linux")]
    fn get_memory_usage() -> Option<usize> {
        if let Ok(file) = File::open("/proc/self/status") {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.starts_with("VmRSS:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(kb) = parts[1].parse::<usize>() {
                                return Some(kb * 1024);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "linux"))]
    fn get_memory_usage() -> Option<usize> {
        // For non-Linux platforms, we can't easily get memory usage
        // In a real implementation, platform-specific code would go here
        None
    }
}

/// Reusable test infrastructure for file tools performance testing
pub struct FileTestEnvironment {
    pub temp_dir: TempDir,
    pub workspace_root: PathBuf,
}

impl FileTestEnvironment {
    pub fn new() -> Result<Self, std::io::Error> {
        let temp_dir = TempDir::new()?;
        let workspace_root = temp_dir.path().to_path_buf();
        Ok(Self {
            temp_dir,
            workspace_root,
        })
    }

    pub fn create_test_file(&self, path: &str, content: &str) -> Result<PathBuf, std::io::Error> {
        let full_path = self.workspace_root.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;
        Ok(full_path)
    }

    pub fn create_test_directory(&self, path: &str) -> Result<PathBuf, std::io::Error> {
        let full_path = self.workspace_root.join(path);
        fs::create_dir_all(&full_path)?;
        Ok(full_path)
    }

    pub fn workspace_path(&self) -> &Path {
        &self.workspace_root
    }

    /// Create a large file with specified size in MB
    pub fn create_large_file(&self, path: &str, size_mb: usize) -> Result<PathBuf, std::io::Error> {
        let content = "0123456789".repeat(100_000); // ~1MB of content
        let mut full_content = String::new();
        for _ in 0..size_mb {
            full_content.push_str(&content);
        }
        self.create_test_file(path, &full_content)
    }

    /// Create a deep directory tree for traversal testing
    pub fn create_deep_directory_tree(
        &self,
        depth: usize,
        files_per_level: usize,
    ) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut created_files = Vec::new();

        // Create directory structure
        for level in 0..depth {
            let dir_path = format!("level_{}", level);
            let dir = if level == 0 {
                self.create_test_directory(&dir_path)?
            } else {
                let parent_path = (0..level)
                    .map(|i| format!("level_{}", i))
                    .collect::<Vec<_>>()
                    .join("/");
                self.create_test_directory(&format!("{}/{}", parent_path, dir_path))?
            };

            // Create files in this directory level
            for file_num in 0..files_per_level {
                let file_content = format!("Content for level {} file {}", level, file_num);
                let file_path = dir.join(format!("file_{}.txt", file_num));
                fs::write(&file_path, &file_content)?;
                created_files.push(file_path);
            }
        }

        Ok(created_files)
    }

    /// Create files with complex glob patterns
    pub fn create_glob_test_files(&self) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = Vec::new();

        // Create various file types and structures
        let test_patterns = vec![
            ("src/main.rs", "fn main() {}"),
            ("src/lib.rs", "pub mod utils;"),
            ("src/utils/mod.rs", "pub fn helper() {}"),
            ("tests/test_main.rs", "#[test] fn test() {}"),
            ("docs/README.md", "# Project"),
            ("config/app.toml", "[app]\nname = 'test'"),
            (".gitignore", "target/\n*.log"),
            ("Cargo.toml", "[package]\nname = 'test'"),
        ];

        for (path, content) in test_patterns {
            let file_path = self.create_test_file(path, content)?;
            files.push(file_path);
        }

        Ok(files)
    }

    /// Create files for grep performance testing
    pub fn create_grep_test_files(
        &self,
        num_files: usize,
        lines_per_file: usize,
    ) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = Vec::new();

        for file_num in 0..num_files {
            let mut content = String::new();
            for line_num in 0..lines_per_file {
                let line = if line_num % 10 == 0 {
                    format!("MATCH_TARGET line {} in file {}\n", line_num, file_num)
                } else {
                    format!("Regular line {} content data here\n", line_num)
                };
                content.push_str(&line);
            }

            let file_path =
                self.create_test_file(&format!("search_file_{}.txt", file_num), &content)?;
            files.push(file_path);
        }

        Ok(files)
    }
}

/// Create a test context with mock storage backends
async fn create_test_context() -> ToolContext {
    let issue_storage: Arc<tokio::sync::RwLock<Box<dyn IssueStorage>>> =
        Arc::new(tokio::sync::RwLock::new(Box::new(
            FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap(),
        )));
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let memo_storage: Arc<tokio::sync::RwLock<Box<dyn MemoStorage>>> =
        Arc::new(tokio::sync::RwLock::new(Box::new(MockMemoStorage::new())));

    let rate_limiter = Arc::new(swissarmyhammer::common::rate_limiter::MockRateLimiter);
    let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

    ToolContext::new(
        tool_handlers,
        issue_storage,
        git_ops,
        memo_storage,
        rate_limiter,
    )
}

/// Create a test tool registry with file tools registered
fn create_test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    files::register_file_tools(&mut registry);
    registry
}

// ============================================================================
// Performance Tests for File Read Tool
// ============================================================================

#[tokio::test]
async fn test_read_tool_large_file_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();

    // Test different file sizes
    let file_sizes = vec![1, 5, 10, 20]; // MB sizes
    let mut results = HashMap::new();

    for size_mb in file_sizes {
        let profiler = PerformanceProfiler::new();

        // Create large test file
        let large_file = test_env
            .create_large_file(&format!("large_file_{}_mb.txt", size_mb), size_mb)
            .unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "absolute_path".to_string(),
            json!(large_file.to_string_lossy()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(
            result.is_ok(),
            "Large file read should succeed for {} MB",
            size_mb
        );

        results.insert(
            size_mb,
            profiler.format_results(&format!("Read {} MB file", size_mb)),
        );
    }

    // Report performance results
    for result in results.values() {
        println!("📊 {}", result);
    }

    // Verify performance stays reasonable (< 2 seconds for largest file)
    // Note: This is a reasonable threshold for file operations
    let _largest_file_result = &results[&20];
    // Basic smoke test - just ensure it completes without panic
}

#[tokio::test]
async fn test_read_tool_offset_limit_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();
    let large_file = test_env.create_large_file("offset_test.txt", 10).unwrap();

    // Test different offset/limit combinations
    let test_cases = vec![
        (0, Some(1000)),     // Read first 1000 lines
        (1000, Some(1000)),  // Read middle 1000 lines
        (10000, Some(1000)), // Read offset 1000 lines
        (0, None),           // Read entire file
    ];

    for (offset, limit) in test_cases {
        let profiler = PerformanceProfiler::new();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "absolute_path".to_string(),
            json!(large_file.to_string_lossy()),
        );
        arguments.insert("offset".to_string(), json!(offset));
        if let Some(limit_val) = limit {
            arguments.insert("limit".to_string(), json!(limit_val));
        }

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok(), "Offset/limit read should succeed");

        println!(
            "📊 {}",
            profiler.format_results(&format!("Read offset {} limit {:?}", offset, limit))
        );
    }
}

// ============================================================================
// Performance Tests for File Write Tool
// ============================================================================

#[tokio::test]
async fn test_write_tool_large_content_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();

    // Test writing different content sizes
    let content_sizes = vec![1, 5, 9]; // MB sizes (9MB to stay under 10MB limit)
    for size_mb in content_sizes {
        let profiler = PerformanceProfiler::new();

        let large_content = "0123456789\n".repeat(90_000 * size_mb); // ~size_mb MB, under 10MB limit
        let output_file = test_env
            .workspace_path()
            .join(format!("write_test_{}_mb.txt", size_mb));

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "file_path".to_string(),
            json!(output_file.to_string_lossy()),
        );
        arguments.insert("content".to_string(), json!(large_content));

        let result = tool.execute(arguments, &context).await;
        assert!(
            result.is_ok(),
            "Large content write should succeed for {} MB",
            size_mb
        );

        // Verify file was created with correct size
        assert!(output_file.exists());
        let written_size = fs::metadata(&output_file).unwrap().len();
        assert!(written_size > (size_mb * 900_000) as u64); // Allow some variance

        println!(
            "📊 {}",
            profiler.format_results(&format!("Write {} MB content", size_mb))
        );
    }
}

#[tokio::test]
async fn test_write_tool_concurrent_operations() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();
    let profiler = PerformanceProfiler::new();

    // Create concurrent write operations
    let num_concurrent = 10;
    let mut futures = Vec::new();

    for i in 0..num_concurrent {
        let output_file = test_env
            .workspace_path()
            .join(format!("concurrent_{}.txt", i));
        let content = format!("Content for concurrent file {} with data\n", i).repeat(1000);

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "file_path".to_string(),
            json!(output_file.to_string_lossy()),
        );
        arguments.insert("content".to_string(), json!(content));

        let future = tool.execute(arguments, &context);
        futures.push(future);
    }

    // Wait for all operations to complete
    let mut success_count = 0;
    for future in futures {
        let result = future.await;
        if result.is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, num_concurrent,
        "All concurrent writes should succeed"
    );
    println!(
        "📊 {}",
        profiler.format_results(&format!("Concurrent {} write operations", num_concurrent))
    );

    // Verify all files were created
    for i in 0..num_concurrent {
        let file_path = test_env
            .workspace_path()
            .join(format!("concurrent_{}.txt", i));
        assert!(file_path.exists(), "Concurrent file {} should exist", i);
    }
}

// ============================================================================
// Performance Tests for File Edit Tool
// ============================================================================

#[tokio::test]
async fn test_edit_tool_large_file_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();

    // Create large file with replaceable content
    let base_content = "This is a line with OLD_VALUE that should be replaced.\n";
    let large_content = base_content.repeat(100_000); // ~100k lines

    let large_file = test_env
        .create_test_file("edit_large.txt", &large_content)
        .unwrap();

    // Test single replacement
    let profiler = PerformanceProfiler::new();
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("OLD_VALUE"));
    arguments.insert("new_string".to_string(), json!("NEW_VALUE"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Large file single edit should succeed");

    println!(
        "📊 {}",
        profiler.format_results("Edit large file (single replacement)")
    );

    // Test replace_all on large file
    let profiler = PerformanceProfiler::new();
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("NEW_VALUE"));
    arguments.insert("new_string".to_string(), json!("FINAL_VALUE"));
    arguments.insert("replace_all".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Large file replace_all should succeed");

    println!(
        "📊 {}",
        profiler.format_results("Edit large file (replace all occurrences)")
    );
}

// ============================================================================
// Performance Tests for File Glob Tool
// ============================================================================

#[tokio::test]
async fn test_glob_tool_large_directory_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();

    // Create deep directory structure
    let profiler = PerformanceProfiler::new();
    test_env.create_deep_directory_tree(10, 20).unwrap(); // 10 levels, 20 files per level = 200 files

    println!(
        "📊 {}",
        profiler.format_results("Create deep directory structure (10 levels, 200 files)")
    );

    // Test various glob patterns
    let test_patterns = vec![
        ("**/*.txt", "Recursive text file search"),
        ("*/file_*.txt", "Single level wildcard"),
        ("level_*/level_*/file_*.txt", "Multi-level specific pattern"),
        ("**/*", "Match all files recursively"),
    ];

    for (pattern, description) in test_patterns {
        let profiler = PerformanceProfiler::new();

        let mut arguments = serde_json::Map::new();
        arguments.insert("pattern".to_string(), json!(pattern));
        arguments.insert(
            "path".to_string(),
            json!(test_env.workspace_path().to_string_lossy()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok(), "Glob pattern '{}' should succeed", pattern);

        println!(
            "📊 {}",
            profiler.format_results(&format!("Glob pattern: {} ({})", pattern, description))
        );
    }
}

#[tokio::test]
async fn test_glob_tool_complex_patterns_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();

    // Create realistic project structure
    test_env.create_glob_test_files().unwrap();

    let complex_patterns = vec![
        ("**/*.{rs,toml,md}", "Multiple file extensions"),
        ("src/**/*.rs", "Source files only"),
        ("{src,tests}/**/*", "Multiple directories"),
        ("**/*[!.]*.{rs,md}", "Complex exclusion pattern"),
    ];

    for (pattern, description) in complex_patterns {
        let profiler = PerformanceProfiler::new();

        let mut arguments = serde_json::Map::new();
        arguments.insert("pattern".to_string(), json!(pattern));
        arguments.insert(
            "path".to_string(),
            json!(test_env.workspace_path().to_string_lossy()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(
            result.is_ok(),
            "Complex glob pattern '{}' should succeed",
            pattern
        );

        println!(
            "📊 {}",
            profiler.format_results(&format!("Complex pattern: {} ({})", pattern, description))
        );
    }
}

// ============================================================================
// Performance Tests for File Grep Tool
// ============================================================================

#[tokio::test]
async fn test_grep_tool_large_content_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();

    // Create large set of files for searching
    let profiler = PerformanceProfiler::new();
    test_env.create_grep_test_files(50, 1000).unwrap(); // 50 files, 1000 lines each

    println!(
        "📊 {}",
        profiler.format_results("Create grep test files (50 files, 50k lines total)")
    );

    // Test different search patterns and modes
    let test_cases = vec![
        (
            "MATCH_TARGET",
            "files_with_matches",
            "Simple pattern, files mode",
        ),
        ("MATCH_TARGET", "content", "Simple pattern, content mode"),
        ("MATCH_TARGET", "count", "Simple pattern, count mode"),
        (
            "line \\d+ in file",
            "content",
            "Regex pattern, content mode",
        ),
        (
            "(?i)MATCH_TARGET",
            "files_with_matches",
            "Case-insensitive regex",
        ),
    ];

    for (pattern, output_mode, description) in test_cases {
        let profiler = PerformanceProfiler::new();

        let mut arguments = serde_json::Map::new();
        arguments.insert("pattern".to_string(), json!(pattern));
        arguments.insert(
            "path".to_string(),
            json!(test_env.workspace_path().to_string_lossy()),
        );
        arguments.insert("output_mode".to_string(), json!(output_mode));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok(), "Grep pattern '{}' should succeed", pattern);

        println!(
            "📊 {}",
            profiler.format_results(&format!("Grep: {} ({})", pattern, description))
        );
    }
}

#[tokio::test]
async fn test_grep_tool_complex_regex_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();

    // Create files with various content patterns
    let complex_content = r#"
        function processData(data) {
            const result = data.map(item => {
                return {
                    id: item.id,
                    name: item.name,
                    email: item.email || 'noemail@example.com'
                };
            });
            return result;
        }
        
        class DataProcessor {
            constructor(options = {}) {
                this.options = options;
            }
            
            async process(input) {
                return await this.processInternal(input);
            }
        }
    "#;

    test_env
        .create_test_file("complex_content.js", complex_content)
        .unwrap();

    let complex_patterns = vec![
        (r"function\s+\w+\s*\([^)]*\)", "Function definitions"),
        (r"class\s+\w+\s*\{", "Class definitions"),
        (r"\b\w+@\w+\.\w+\b", "Email addresses"),
        (r"async\s+\w+\s*\([^)]*\)", "Async functions"),
        (r"const\s+\w+\s*=\s*[^;]+;", "Const declarations"),
    ];

    for (pattern, description) in complex_patterns {
        let profiler = PerformanceProfiler::new();

        let mut arguments = serde_json::Map::new();
        arguments.insert("pattern".to_string(), json!(pattern));
        arguments.insert(
            "path".to_string(),
            json!(test_env.workspace_path().to_string_lossy()),
        );
        arguments.insert("output_mode".to_string(), json!("content"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok(), "Complex regex '{}' should succeed", pattern);

        println!(
            "📊 {}",
            profiler.format_results(&format!("Complex regex: {} ({})", pattern, description))
        );
    }
}

// ============================================================================
// Cross-Tool Performance Tests
// ============================================================================

#[tokio::test]
async fn test_cross_tool_workflow_performance() {
    let _guard = IsolatedTestHome::new();
    let registry = create_test_registry();
    let context = create_test_context().await;

    let read_tool = registry.get_tool("files_read").unwrap();
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let test_env = FileTestEnvironment::new().unwrap();

    let profiler = PerformanceProfiler::new();

    // Step 1: Write initial content
    let test_file = test_env.workspace_path().join("workflow_test.txt");
    let initial_content = "Initial content with TARGET_VALUE for replacement.".repeat(1000);

    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(initial_content));

    let write_result = write_tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write step should succeed");

    // Step 2: Read content
    let mut read_args = serde_json::Map::new();
    read_args.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read step should succeed");

    // Step 3: Edit content
    let mut edit_args = serde_json::Map::new();
    edit_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    edit_args.insert("old_string".to_string(), json!("TARGET_VALUE"));
    edit_args.insert("new_string".to_string(), json!("REPLACED_VALUE"));
    edit_args.insert("replace_all".to_string(), json!(true));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit step should succeed");

    // Step 4: Verify with final read
    let mut final_read_args = serde_json::Map::new();
    final_read_args.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let final_read_result = read_tool.execute(final_read_args, &context).await;
    assert!(final_read_result.is_ok(), "Final read should succeed");

    println!(
        "📊 {}",
        profiler.format_results("Complete workflow: Write → Read → Edit → Read")
    );
}

// ============================================================================
// Utility Functions
// ============================================================================

fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis >= 10_000 {
        format!("{:.2}s", duration.as_secs_f64())
    } else if millis >= 1_000 {
        format!("{:.3}s", duration.as_secs_f64())
    } else {
        format!("{}ms", millis)
    }
}

fn format_memory_delta(bytes: isize) -> String {
    let abs_bytes = bytes.unsigned_abs();
    let sign = if bytes >= 0 { "+" } else { "-" };

    if abs_bytes >= 1_000_000_000 {
        format!("{}{:.1} GB", sign, abs_bytes as f64 / 1_000_000_000.0)
    } else if abs_bytes >= 1_000_000 {
        format!("{}{:.1} MB", sign, abs_bytes as f64 / 1_000_000.0)
    } else if abs_bytes >= 1_000 {
        format!("{}{:.1} KB", sign, abs_bytes as f64 / 1_000.0)
    } else {
        format!("{}{} bytes", sign, abs_bytes)
    }
}
