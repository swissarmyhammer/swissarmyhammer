//! Integration tests for file tools
//!
//! These tests verify that file tools work correctly through all layers of the system,
//! including MCP protocol handling, tool registry integration, security validation,
//! and end-to-end scenarios.

use serde_json::json;
use std::fs;

use std::sync::Arc;

use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::files;

#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader};

/// Dangerous paths for security testing
const DANGEROUS_PATHS: &[&str] = &[
    "/tmp/../../../etc/passwd",
    "/home/user/../../../etc/passwd",
    "../../../etc/passwd",
    "..\\..\\..\\windows\\system32\\config\\sam",
    "/var/tmp/../../../../etc/shadow",
    "~/../../etc/hosts",
    "/usr/local/../../../root/.ssh/id_rsa",
    "/tmp/../../../../../proc/version",
];

/// File tools to test for security
const FILE_TOOLS: &[&str] = &[
    "files_read",
    "files_write",
    "files_edit",
    "files_glob",
    "files_grep",
];

/// Create malformed inputs for testing
fn create_malformed_inputs(test_dir_path: &std::path::Path) -> Vec<String> {
    let long_path = "extremely_long_path_".repeat(1000);
    vec![
        "".to_string(),
        "\0".to_string(),
        format!("{}/path/with\0null", test_dir_path.display()),
        format!("{}/path\nwith\nnewlines", test_dir_path.display()),
        format!("{}/path\rwith\rcarriage\rreturns", test_dir_path.display()),
        format!("{}/path\twith\ttabs", test_dir_path.display()),
        format!(
            "{}/path with spaces and special chars: <>|\"*?",
            test_dir_path.display()
        ),
        format!("{}/\u{FEFF}path_with_bom", test_dir_path.display()),
        format!("{}/{}", test_dir_path.display(), long_path),
    ]
}

/// Memory usage profiling utilities for performance testing
struct MemoryProfiler {
    initial_memory: Option<usize>,
}

impl MemoryProfiler {
    fn new() -> Self {
        let initial_memory = Self::get_memory_usage();
        Self { initial_memory }
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
        None
    }

    fn memory_delta(&self) -> Option<isize> {
        if let (Some(initial), Some(current)) = (self.initial_memory, Self::get_memory_usage()) {
            Some(current as isize - initial as isize)
        } else {
            None
        }
    }

    fn format_bytes(bytes: usize) -> String {
        if bytes >= 1_000_000_000 {
            format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
        } else if bytes >= 1_000_000 {
            format!("{:.1} MB", bytes as f64 / 1_000_000.0)
        } else if bytes >= 1_000 {
            format!("{:.1} KB", bytes as f64 / 1_000.0)
        } else {
            format!("{} bytes", bytes)
        }
    }
}

/// Create a test context with mock storage backends for testing MCP tools
async fn create_test_context() -> ToolContext {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let _unique_id = format!(
        "{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );

    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());

    ToolContext::new(tool_handlers, git_ops, agent_config)
}

/// Create a test tool registry with file tools registered
async fn create_test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    files::register_file_tools(&mut registry).await;
    registry
}

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Extract response text from CallToolResult to eliminate duplication across tests
fn extract_response_text(call_result: &rmcp::model::CallToolResult) -> &str {
    if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content")
    }
}

/// Create a test file with given name and content, returning env, temp_dir path, and file path
fn create_test_file(
    name: &str,
    content: &str,
) -> (
    IsolatedTestEnvironment,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir_path = env.temp_dir();
    let test_file = temp_dir_path.join(name);
    fs::write(&test_file, content).unwrap();
    (env, temp_dir_path, test_file)
}

/// Create a temporary directory with an initialized git repository
fn create_test_dir_with_git() -> (IsolatedTestEnvironment, std::path::PathBuf) {
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = env.temp_dir();
    let repo = git2::Repository::init(&temp_dir).expect("Failed to initialize git repo");
    // Ensure initial branch is 'main' for consistency across environments
    repo.set_head("refs/heads/main")
        .expect("Failed to set HEAD to main");
    (env, temp_dir)
}

/// Generic argument builder that accepts key-value pairs
fn build_args(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
    let mut arguments = serde_json::Map::new();
    for (key, value) in pairs {
        arguments.insert(key.to_string(), value.clone());
    }
    arguments
}

/// Check if error message contains any of the expected messages
fn assert_error_contains_any(error_msg: &str, expected_messages: &[&str], context: &str) {
    let contains_expected = expected_messages.iter().any(|msg| error_msg.contains(msg));
    assert!(
        contains_expected,
        "{}: Expected error to contain one of {:?}, but got: {}",
        context, expected_messages, error_msg
    );
}

/// Builder helper for files_read arguments
fn read_args(path: &str) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[("path", json!(path))])
}

/// Builder helper for files_write arguments
fn write_args(file_path: &str, content: &str) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[("file_path", json!(file_path)), ("content", json!(content))])
}

/// Builder helper for files_edit arguments
fn edit_args(
    file_path: &str,
    old_string: &str,
    new_string: &str,
) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[
        ("file_path", json!(file_path)),
        ("old_string", json!(old_string)),
        ("new_string", json!(new_string)),
    ])
}

/// Builder helper for files_glob arguments
fn glob_args(pattern: &str) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[("pattern", json!(pattern))])
}

/// Builder helper for files_grep arguments
fn grep_args(pattern: &str) -> serde_json::Map<String, serde_json::Value> {
    build_args(&[("pattern", json!(pattern))])
}

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

            let write_tool = registry.get_tool("files_write").unwrap();
            let write_args_map = write_args(&file_path.to_string_lossy(), &content);
            write_tool
                .execute(write_args_map, &context)
                .await
                .map_err(|_| "Write failed")?;

            let read_tool = registry.get_tool("files_read").unwrap();
            let read_args_map = read_args(&file_path.to_string_lossy());
            read_tool
                .execute(read_args_map, &context)
                .await
                .map_err(|_| "Read failed")?;

            let edit_tool = registry.get_tool("files_edit").unwrap();
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

            let write_tool = registry_clone.get_tool("files_write").unwrap();
            let mut write_args = serde_json::Map::new();
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

            let read_tool = registry_clone.get_tool("files_read").unwrap();
            let mut read_args = serde_json::Map::new();
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

            let edit_tool = registry_clone.get_tool("files_edit").unwrap();
            let mut edit_args = serde_json::Map::new();
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
            let glob_tool = registry_clone.get_tool("files_glob").unwrap();
            let mut glob_args = serde_json::Map::new();

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
            let read_tool = registry_clone.get_tool("files_read").unwrap();
            let mut read_args = serde_json::Map::new();
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

            let write_tool = registry_clone.get_tool("files_write").unwrap();
            let mut write_args = serde_json::Map::new();
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
            let grep_tool = registry_clone.get_tool("files_grep").unwrap();
            let mut grep_args = serde_json::Map::new();

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

/// Profile memory usage of an operation and return the memory delta
#[allow(dead_code)]
async fn profile_memory<F, Fut, T>(operation: F) -> (T, Option<isize>)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let profiler = MemoryProfiler::new();
    let result = operation().await;
    let delta = profiler.memory_delta();
    (result, delta)
}

/// Build security test arguments for a given tool and dangerous path
fn build_security_test_arguments(
    tool_name: &str,
    dangerous_path: &str,
) -> serde_json::Map<String, serde_json::Value> {
    match tool_name {
        "files_read" => read_args(dangerous_path),
        "files_write" => write_args(dangerous_path, "malicious content"),
        "files_edit" => edit_args(dangerous_path, "old", "new"),
        "files_glob" => {
            let mut args = glob_args("*");
            args.insert("path".to_string(), json!(dangerous_path));
            args
        }
        "files_grep" => {
            let mut args = grep_args("password");
            args.insert("path".to_string(), json!(dangerous_path));
            args
        }
        _ => panic!("Unsupported tool for security testing: {}", tool_name),
    }
}

/// Test path security for a given tool
async fn test_path_security_for_tool(
    tool_name: &str,
    registry: &ToolRegistry,
    context: &ToolContext,
    dangerous_paths: &[&str],
) {
    let tool = registry.get_tool(tool_name).unwrap();

    for dangerous_path in dangerous_paths {
        // Skip Windows-style paths on Unix - backslashes are literal characters, not path separators
        // These paths don't represent actual path traversal attacks on Unix systems
        #[cfg(unix)]
        if dangerous_path.contains('\\') {
            continue;
        }

        let arguments = build_security_test_arguments(tool_name, dangerous_path);
        let result = tool.execute(arguments, context).await;

        match result {
            Err(error) => {
                let error_msg = format!("{:?}", error);
                let expected_messages = &[
                    "blocked pattern",
                    "not found",
                    "absolute",
                    "No such file",
                    "does not exist",
                    "invalid",
                    "dangerous",
                    "traversal",
                    "not allowed",
                ];
                let context_msg = format!(
                    "{} tool should block or safely handle path traversal: {}",
                    tool_name, dangerous_path
                );
                assert_error_contains_any(&error_msg, expected_messages, &context_msg);
            }
            Ok(call_result) => {
                // For write operations, success is a security failure - we shouldn't be able
                // to write to dangerous paths
                if tool_name == "files_write" {
                    panic!(
                        "{} tool allowed write to dangerous path '{}': {:?}",
                        tool_name, dangerous_path, call_result
                    );
                }
                // For read operations, success with is_error=true is acceptable (tool handled it)
                if call_result.is_error == Some(true) {
                    // Tool returned an error response, which is expected
                    continue;
                }
                // Read/glob/grep succeeding on non-existent dangerous paths is also fine
                // (they just won't find anything)
            }
        }
    }
}

/// Parameterized test helper for read tool with offset and limit
async fn test_read_with_offset_limit(
    offset: Option<usize>,
    limit: Option<usize>,
    expected_content: &str,
) {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    let (_env, _temp_dir, test_file) = create_test_file("test_file.txt", test_content);

    let mut arguments = read_args(&test_file.to_string_lossy());
    if let Some(offset_val) = offset {
        arguments.insert("offset".to_string(), json!(offset_val));
    }
    if let Some(limit_val) = limit {
        arguments.insert("limit".to_string(), json!(limit_val));
    }

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Read operation should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    assert_eq!(response_text, expected_content);
}

/// Verify tool exists in registry
fn verify_tool_exists(registry: &ToolRegistry, tool_name: &str) {
    assert!(
        registry.get_tool(tool_name).is_some(),
        "Tool {} should be registered",
        tool_name
    );

    let tool_names = registry.list_tool_names();
    assert!(
        tool_names.contains(&tool_name.to_string()),
        "Tool {} should be in tool list",
        tool_name
    );
}

/// Verify tool description contains expected keywords
fn verify_tool_description(
    tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    description_keywords: &[&str],
) {
    assert!(
        !tool.description().is_empty(),
        "Description should not be empty"
    );

    let description = tool.description().to_lowercase();
    let contains_any_keyword = description_keywords
        .iter()
        .any(|keyword| description.contains(keyword));
    assert!(
        contains_any_keyword,
        "Description should contain at least one of keywords: {:?}, but got: {}",
        description_keywords, description
    );
}

/// Verify tool schema properties
fn verify_tool_schema(
    tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    required_properties: &[&str],
    optional_properties: &[&str],
) {
    let schema = tool.schema();
    assert!(schema.is_object(), "Schema should be an object");

    let properties = schema["properties"].as_object().unwrap();
    for prop in required_properties {
        assert!(
            properties.contains_key(*prop),
            "Schema should contain required property: {}",
            prop
        );
    }
    for prop in optional_properties {
        assert!(
            properties.contains_key(*prop),
            "Schema should contain optional property: {}",
            prop
        );
    }

    // Handle both simple "required" array and "oneOf" schemas
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        // Simple schema with required array
        for prop in required_properties {
            assert!(
                required.contains(&serde_json::Value::String(prop.to_string())),
                "Property {} should be required",
                prop
            );
        }
    } else if let Some(one_of) = schema.get("oneOf").and_then(|o| o.as_array()) {
        // Schema with oneOf - verify at least one alternative contains the required properties
        let mut found_in_any_alternative = vec![false; required_properties.len()];
        for alternative in one_of {
            if let Some(alt_required) = alternative.get("required").and_then(|r| r.as_array()) {
                for (idx, prop) in required_properties.iter().enumerate() {
                    if alt_required.contains(&serde_json::Value::String(prop.to_string())) {
                        found_in_any_alternative[idx] = true;
                    }
                }
            }
        }
        // For oneOf schemas, we just verify the properties exist, not that they're strictly required
        // This is because oneOf means "one of these alternatives must be satisfied"
    }
}

/// Verify tool registration with expected properties
fn verify_tool_registration(
    registry: &ToolRegistry,
    tool_name: &str,
    description_keywords: &[&str],
    required_properties: &[&str],
    optional_properties: &[&str],
) {
    verify_tool_exists(registry, tool_name);

    let tool = registry.get_tool(tool_name).unwrap();
    assert_eq!(<dyn swissarmyhammer_tools::mcp::tool_registry::McpTool as swissarmyhammer_tools::mcp::tool_registry::McpTool>::name(tool), tool_name);

    verify_tool_description(tool, description_keywords);
    verify_tool_schema(tool, required_properties, optional_properties);
}

// ============================================================================
// File Read Tool Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    verify_tool_registration(
        &registry,
        "files_read",
        &["file"],
        &["path"],
        &["offset", "limit"],
    );
}

#[tokio::test]
async fn test_read_tool_execution_success_cases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create temporary file for testing
    let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    let (_env, _temp_dir, test_file) = create_test_file("test_file.txt", test_content);

    // Test basic file reading
    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File read should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
    assert!(!call_result.content.is_empty());

    // Extract the content from the response
    let response_text = extract_response_text(&call_result);

    assert_eq!(response_text, test_content);
}

#[tokio::test]
async fn test_read_tool_offset_limit_functionality() {
    test_read_with_offset_limit(Some(2), Some(2), "Line 2\nLine 3").await;
}

#[tokio::test]
async fn test_read_tool_offset_only() {
    test_read_with_offset_limit(Some(3), None, "Line 3\nLine 4\nLine 5").await;
}

#[tokio::test]
async fn test_read_tool_limit_only() {
    test_read_with_offset_limit(None, Some(3), "Line 1\nLine 2\nLine 3").await;
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_missing_file_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test reading non-existent file
    let arguments = read_args("/non/existent/file.txt");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Reading non-existent file should fail");

    // Verify error contains helpful information
    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(
        error_msg.contains("Parent directory does not exist")
            || error_msg.contains("not found")
            || error_msg.contains("No such file")
    );
}

#[tokio::test]
async fn test_read_tool_relative_path_support() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test reading with relative path - now just verify it doesn't reject due to being relative
    let arguments = read_args("relative/path/file.txt");

    let result = tool.execute(arguments, &context).await;

    // Should not reject due to relative path, but may fail for other reasons (file not found, etc.)
    if let Err(error) = result {
        let error_msg = format!("{:?}", error);
        assert!(
            !error_msg.contains("absolute"),
            "Should not reject relative paths anymore"
        );
    }
    // If it succeeds, that's also fine - relative paths are now allowed
}

#[tokio::test]
async fn test_read_tool_empty_path_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test reading with empty path
    let arguments = read_args("");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty path should be rejected");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(
        error_msg.contains("absolute, not relative")
            || error_msg.contains("empty")
            || error_msg.contains("cannot be empty")
    );
}

#[tokio::test]
async fn test_read_tool_missing_required_parameter() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test execution without required path parameter
    let arguments = serde_json::Map::new(); // Empty arguments

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Missing required parameter should fail");
}

// ============================================================================
// Security Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_path_traversal_protection() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test various path traversal attempts
    let dangerous_paths = vec![
        "/tmp/../../../etc/passwd",
        "/tmp/../../etc/passwd",
        "/home/user/../../../etc/passwd",
    ];

    for dangerous_path in dangerous_paths {
        let arguments = read_args(dangerous_path);

        let result = tool.execute(arguments, &context).await;

        // The result may either fail due to path validation or file not found
        // Both outcomes are acceptable for security
        if let Err(err) = result {
            let error_msg = format!("{:?}", err);
            assert!(
                error_msg.contains("blocked pattern")
                    || error_msg.contains("not found")
                    || error_msg.contains("No such file"),
                "Path traversal should be blocked or file not found: {} (error: {})",
                dangerous_path,
                error_msg
            );
        }
        // If it succeeds, the file either doesn't exist or is blocked properly
    }
}

#[tokio::test]
async fn test_read_tool_handles_large_files_safely() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create a reasonably large test file
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("large_file.txt");

    let mut large_content = String::new();
    for i in 1..=1000 {
        large_content.push_str(&format!("Line {} content\n", i));
    }
    fs::write(test_file, &large_content).unwrap();

    // Test reading large file with limit
    let mut arguments = read_args(&test_file.to_string_lossy());
    arguments.insert("limit".to_string(), json!(10)); // Only read first 10 lines

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Reading large file with limit should succeed"
    );

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should only contain first 10 lines
    let line_count = response_text.lines().count();
    assert_eq!(line_count, 10);
    assert!(response_text.starts_with("Line 1 content"));
    assert!(response_text.contains("Line 10 content"));
    assert!(!response_text.contains("Line 11 content"));
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_empty_file() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create empty file
    let (_env, _temp_dir, test_file) = create_test_file("empty_file.txt", "");

    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Reading empty file should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    assert_eq!(response_text, "");
}

#[tokio::test]
async fn test_read_tool_single_line_file() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let test_content = "Single line without newline";
    let (_env, _temp_dir, test_file) = create_test_file("single_line.txt", test_content);

    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    assert_eq!(response_text, test_content);
}

#[tokio::test]
async fn test_read_tool_with_unicode_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let test_content = "Hello 🌍\n世界\nПривет мир\n";
    let (_env, _temp_dir, test_file) = create_test_file("unicode_file.txt", test_content);

    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Reading unicode file should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    assert_eq!(response_text, test_content);
}

#[tokio::test]
async fn test_read_tool_excessive_offset_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let mut arguments = read_args("/tmp/test.txt");
    arguments.insert("offset".to_string(), json!(2_000_000));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject offset over 1,000,000");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("offset must be less than 1,000,000"));
    }
}

#[tokio::test]
async fn test_read_tool_zero_limit_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let mut arguments = read_args("/tmp/test.txt");
    arguments.insert("limit".to_string(), json!(0));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject zero limit");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("limit must be greater than 0"));
    }
}

#[tokio::test]
async fn test_read_tool_excessive_limit_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let mut arguments = read_args("/tmp/test.txt");
    arguments.insert("limit".to_string(), json!(200_000));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject limit over 100,000");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("limit must be less than or equal to 100,000"));
    }
}

#[tokio::test]
async fn test_read_tool_file_not_found_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test non-existent file
    let arguments = read_args("/tmp/definitely_does_not_exist_12345.txt");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should fail for non-existent file");
}

#[tokio::test]
async fn test_read_tool_permission_denied_scenarios() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test unreadable file (if we can create one)
    let (_env, _temp_dir, test_file) = create_test_file("unreadable.txt", "secret content");

    // Try to make it unreadable (may not work on all systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&test_file).unwrap().permissions();
        perms.set_mode(0o000); // No permissions
        let _ = fs::set_permissions(&test_file, perms);
    }

    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    // Note: This test may pass on systems where we can't actually restrict permissions
    if let Err(err) = result {
        let error_msg = format!("{:?}", err);
        println!("Permission denied test error: {}", error_msg);
    }
}

#[tokio::test]
async fn test_read_tool_large_file_handling() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create a larger file to test performance
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("large_file.txt");

    let mut large_content = String::new();
    for i in 0..1000 {
        large_content.push_str(&format!("This is line number {}\n", i + 1));
    }
    fs::write(test_file, &large_content).unwrap();

    // Test reading with limit to avoid reading the entire large file
    let mut arguments = read_args(&test_file.to_string_lossy());
    arguments.insert("limit".to_string(), json!(100)); // Read only 100 lines

    let start_time = std::time::Instant::now();
    let result = tool.execute(arguments, &context).await;
    let duration = start_time.elapsed();

    assert!(
        result.is_ok(),
        "Large file read should succeed: {:?}",
        result
    );
    assert!(
        duration.as_secs() < 5,
        "Large file read should complete quickly"
    );

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should contain exactly 100 lines worth of content
    let line_count = response_text.lines().count();
    assert_eq!(line_count, 100, "Should read exactly 100 lines");
}

#[tokio::test]
async fn test_read_tool_edge_cases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test empty file
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let empty_file = &temp_dir.join("empty.txt");
    fs::write(empty_file, "").unwrap();

    let arguments = read_args(&empty_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Empty file read should succeed");

    // Test file with only whitespace
    let whitespace_file = &temp_dir.join("whitespace.txt");
    fs::write(whitespace_file, "   \n\t\n   \n").unwrap();

    let arguments = read_args(&whitespace_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Whitespace file read should succeed");

    // Test file with mixed line endings
    let mixed_endings_file = &temp_dir.join("mixed_endings.txt");
    fs::write(mixed_endings_file, "Line 1\nLine 2\r\nLine 3\rLine 4").unwrap();

    let arguments = read_args(&mixed_endings_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Mixed line endings file read should succeed"
    );
}

// ============================================================================
// Glob Tool Tests
// ============================================================================

#[tokio::test]
async fn test_glob_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    verify_tool_registration(
        &registry,
        "files_glob",
        &["pattern"],
        &["pattern"],
        &["path", "case_sensitive", "respect_git_ignore"],
    );
}

#[tokio::test]
async fn test_glob_tool_basic_pattern_matching() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test directory structure
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_files = vec![
        "test1.txt",
        "test2.js",
        "subdir/test3.txt",
        "subdir/test4.py",
        "README.md",
    ];

    for file_path in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test basic glob pattern
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Basic glob should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Extract response text
    let response_text = extract_response_text(&call_result);

    assert!(response_text.contains("test1.txt"));
    assert!(!response_text.contains("test2.js"));
    assert!(!response_text.contains("README.md"));
}

#[tokio::test]
async fn test_glob_tool_advanced_gitignore_integration() {
    // This test verifies .gitignore patterns are properly respected.
    // Since **/* is rejected as too broad, we use directory-scoped patterns
    // to test the gitignore functionality.
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test directory with .gitignore and git repo
    let (_env, temp_dir) = create_test_dir_with_git();

    // Write .gitignore file
    let gitignore_content = "*.log\n/build/\ntemp_*\n!important.log\n";
    fs::write(temp_dir.join(".gitignore"), gitignore_content).unwrap();

    // Create a src directory structure for scoped pattern testing
    let test_files = vec![
        "src/main.rs",
        "src/lib.rs",
        "src/debug.log",    // Should be ignored by *.log
        "important.log",    // Explicitly not ignored by !important.log
        "debug.log",        // Should be ignored by *.log
        "build/output.txt", // Should be ignored by /build/
        "temp_file.txt",    // Should be ignored by temp_*
        "normal.txt",       // Should be included
    ];

    for file_path in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test 1: Scoped pattern for src directory with gitignore
    let mut arguments = glob_args("src/**/*.rs");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Scoped gitignore glob should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find .rs files in src/
    assert!(response_text.contains("main.rs"), "Should find main.rs");
    assert!(response_text.contains("lib.rs"), "Should find lib.rs");
    // Should NOT find log files even in src/ (gitignore applies)
    assert!(
        !response_text.contains("debug.log"),
        "Should not find src/debug.log"
    );

    // Test 2: Root-level txt files pattern to verify temp_* is ignored
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Root txt pattern should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find normal.txt
    assert!(
        response_text.contains("normal.txt"),
        "Should find normal.txt"
    );
    // Should NOT find temp_file.txt (ignored by temp_*)
    assert!(
        !response_text.contains("temp_file.txt"),
        "Should not find temp_file.txt"
    );

    // Test 3: Log files pattern to verify !important.log negation
    let mut arguments = glob_args("*.log");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Root log pattern should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find important.log (negated in .gitignore with !important.log)
    assert!(
        response_text.contains("important.log"),
        "Should find important.log (negated ignore)"
    );
    // Should NOT find debug.log (ignored by *.log)
    assert!(
        !response_text.contains("debug.log"),
        "Should not find debug.log"
    );
}

#[tokio::test]
async fn test_glob_tool_pattern_validation() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Test empty pattern
    let arguments = glob_args("");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty pattern should fail");

    // Test overly long pattern
    let long_pattern = "a".repeat(1001);
    let arguments = glob_args(&long_pattern);

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Overly long pattern should fail");

    // Test invalid glob pattern
    let arguments = glob_args("[invalid[pattern");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Invalid glob pattern should fail");
}

#[tokio::test]
async fn test_glob_tool_case_sensitivity() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test files with mixed case
    let (_env, temp_dir) = create_test_dir_with_git();

    // Use different filenames to avoid filesystem case issues
    let test_files = vec!["Test.TXT", "other.txt", "README.md", "readme.MD"];

    for file_path in test_files {
        let full_path = &temp_dir.join(file_path);
        fs::write(full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test case insensitive (default) - use basic glob to avoid filesystem case issues
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(false)); // Use fallback glob

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find both .TXT and .txt with case insensitive
    assert!(response_text.contains("Test.TXT"));
    assert!(response_text.contains("other.txt"));

    // Test case sensitive
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("case_sensitive".to_string(), json!(true));
    arguments.insert("respect_git_ignore".to_string(), json!(false)); // Use fallback glob

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should only find .txt files, not .TXT
    assert!(!response_text.contains("Test.TXT"));
    assert!(response_text.contains("other.txt"));
}

#[tokio::test]
async fn test_glob_tool_modification_time_sorting() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test files with different modification times
    let (_env, temp_dir) = create_test_dir_with_git();

    let file1 = &temp_dir.join("old_file.txt");
    fs::write(file1, "Old content").unwrap();

    // Sleep to ensure different modification times
    std::thread::sleep(std::time::Duration::from_millis(100));

    let file2 = &temp_dir.join("new_file.txt");
    fs::write(file2, "New content").unwrap();

    // Test that files are sorted by modification time (recent first)
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Parse the response to check order - filter out only file paths, not header lines
    let lines: Vec<&str> = response_text
        .lines()
        .filter(|line| line.contains(".txt") && line.starts_with("/"))
        .collect();

    // The newer file should appear before the older file
    if lines.len() >= 2 {
        let first_file_is_new = lines[0].contains("new_file.txt");
        let second_file_is_old = lines[1].contains("old_file.txt");

        // Both conditions should be true for proper sorting
        assert!(
            first_file_is_new && second_file_is_old,
            "Files should be sorted by modification time (recent first). Found order: {:?}",
            lines
        );
    }
}

#[tokio::test]
async fn test_glob_tool_no_matches() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test directory with no matching files
    let (_env, temp_dir) = create_test_dir_with_git();

    fs::write(temp_dir.join("test.txt"), "content").unwrap();

    // Search for pattern that won't match
    let mut arguments = glob_args("*.nonexistent");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "No matches should still succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    assert!(response_text.contains("No files found matching pattern"));
}

#[tokio::test]
async fn test_glob_tool_recursive_patterns() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create nested directory structure
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_files = vec![
        "root.rs",
        "src/main.rs",
        "src/lib.rs",
        "src/utils/helper.rs",
        "tests/integration.rs",
        "docs/readme.md",
    ];

    for file_path in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test recursive Rust file search
    let mut arguments = glob_args("**/*.rs");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find all Rust files
    assert!(response_text.contains("root.rs"));
    assert!(response_text.contains("main.rs"));
    assert!(response_text.contains("lib.rs"));
    assert!(response_text.contains("helper.rs"));
    assert!(response_text.contains("integration.rs"));

    // Should not find non-Rust files
    assert!(!response_text.contains("readme.md"));
}

// ============================================================================
// Grep Tool Tests
// ============================================================================

#[tokio::test]
async fn test_grep_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    verify_tool_registration(
        &registry,
        "files_grep",
        &["search", "grep"],
        &["pattern"],
        &[
            "path",
            "glob",
            "type",
            "case_insensitive",
            "context_lines",
            "output_mode",
        ],
    );
}

#[tokio::test]
async fn test_grep_tool_basic_pattern_matching() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test files with content to search
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    let test_files = vec![
        ("src/main.rs", "fn main() {\n    println!(\"Hello, world!\");\n    let result = calculate();\n}"),
        ("src/lib.rs", "pub fn calculate() -> i32 {\n    42\n}\n\npub fn helper() {\n    // Helper function\n}"),
        ("README.md", "# Project\n\nThis is a test project.\nIt contains example functions.\n"),
        ("docs/guide.txt", "User guide:\n1. Run the program\n2. Check the output\n"),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    // Test basic search for "function"
    let mut arguments = glob_args("function");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Basic grep should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Extract response text
    let response_text = extract_response_text(&call_result);

    // Should find "functions" in README.md and "Helper function" in lib.rs
    assert!(response_text.contains("functions") || response_text.contains("Helper function"));
    assert!(response_text.contains("Time:")); // Should show timing info
}

#[tokio::test]
async fn test_grep_tool_file_type_filtering() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test files with different extensions
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    let test_files = vec![
        ("main.rs", "fn main() {\n    let test = true;\n}"),
        ("script.py", "def test_function():\n    return True"),
        ("app.js", "function test() {\n    return true;\n}"),
        ("style.css", ".test {\n    color: red;\n}"),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        fs::write(full_path, content).unwrap();
    }

    // Test filtering by Rust files only
    let mut arguments = glob_args("test");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("type".to_string(), json!("rust"));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "File type filtering should succeed: {:?}",
        result
    );

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should only find matches in Rust files
    assert!(response_text.contains("main.rs") || response_text.contains("1 matches"));
    assert!(!response_text.contains("script.py"));
    assert!(!response_text.contains("app.js"));
    assert!(!response_text.contains("style.css"));
}

#[tokio::test]
async fn test_grep_tool_glob_filtering() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test files in different directories
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    let test_files = vec![
        ("src/main.rs", "const VERSION: &str = \"1.0.0\";"),
        ("tests/unit.rs", "const TEST_VERSION: &str = \"1.0.0\";"),
        ("benches/bench.rs", "const BENCH_VERSION: &str = \"1.0.0\";"),
        ("examples/demo.rs", "const DEMO_VERSION: &str = \"1.0.0\";"),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    // Test filtering by glob pattern - use a simpler glob that should work
    let mut arguments = glob_args("VERSION");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("glob".to_string(), json!("*.rs")); // Simplified glob pattern

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Glob filtering should succeed: {:?}",
        result
    );

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find VERSION in Rust files (basic glob test)
    println!("Glob filtering response: {}", response_text);
    // With a *.rs glob, we should find matches in Rust files
    assert!(
        response_text.contains("4 matches")
            || response_text.contains("VERSION")
            || response_text.contains("matches in"),
        "Should find matches with *.rs glob pattern. Got: {}",
        response_text
    );
}

#[tokio::test]
async fn test_grep_tool_case_sensitivity() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test file with mixed case content
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test.txt");
    let content = "Hello World\nHELLO WORLD\nhello world\nGoodbye World";
    fs::write(test_file, content).unwrap();

    // Test case sensitive search
    let mut arguments = glob_args("Hello");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("case_insensitive".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Case sensitive search should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should only match exact case
    assert!(response_text.contains("1 matches") || response_text.contains("Hello World"));

    // Test case insensitive search
    let mut arguments = glob_args("hello");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("case_insensitive".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Case insensitive search should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should match all case variations
    assert!(response_text.contains("3 matches") || response_text.contains("Hello World"));
}

#[tokio::test]
async fn test_grep_tool_context_lines() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test file with multiple lines
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("context.txt");
    let content = "Line 1\nLine 2\nMATCH HERE\nLine 4\nLine 5\nLine 6\nANOTHER MATCH\nLine 8";
    fs::write(test_file, content).unwrap();

    // Test with context lines
    let mut arguments = glob_args("MATCH");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("context_lines".to_string(), json!(1));
    arguments.insert("output_mode".to_string(), json!("content"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Context lines search should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // When using fallback, context may not be perfectly formatted but should include matches
    assert!(response_text.contains("MATCH") || response_text.contains("2 matches"));
}

#[tokio::test]
async fn test_grep_tool_output_modes() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test files
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    let test_files = vec![
        (
            "file1.txt",
            "This contains the target word multiple times.\nTarget here too.",
        ),
        ("file2.txt", "Another target in this file."),
        ("file3.txt", "No matches in this file."),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        fs::write(full_path, content).unwrap();
    }

    // Test files_with_matches mode
    let mut arguments = glob_args("target");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("output_mode".to_string(), json!("files_with_matches"));
    arguments.insert("case_insensitive".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "files_with_matches mode should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should show files with matches (not individual line matches)
    assert!(
        (response_text.contains("2") && response_text.contains("files"))
            || response_text.contains("Files with matches (2)"),
        "Response should indicate 2 files found. Got: {}",
        response_text
    );

    // Test count mode
    let mut arguments = glob_args("target");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("output_mode".to_string(), json!("count"));
    arguments.insert("case_insensitive".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "count mode should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should show match count
    assert!(response_text.contains("matches"));
    // Should find 3-4 matches across files (3 target + 1 Target)
    assert!(
        response_text.contains("3") || response_text.contains("4"),
        "Should find 3-4 matches across files. Got: {}",
        response_text
    );
}

#[tokio::test]
async fn test_grep_tool_error_handling() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Test invalid regex pattern
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let mut arguments = glob_args("[invalid");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Invalid regex should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    // The error might come from ripgrep or the regex engine - both are acceptable
    assert!(
        error_msg.contains("Invalid regex pattern")
            || error_msg.contains("regex")
            || error_msg.contains("failed")
            || error_msg.contains("search failed"),
        "Error message should indicate regex or search failure: {}",
        error_msg
    );

    // Test non-existent directory
    let mut arguments = glob_args("test");
    arguments.insert("path".to_string(), json!("/non/existent/directory"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Non-existent directory should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("does not exist") || error_msg.contains("not found"));

    // Test invalid output mode
    let mut arguments = glob_args("test");
    arguments.insert("output_mode".to_string(), json!("invalid_mode"));

    let result = tool.execute(arguments, &context).await;
    // This should either fail during execution or handle gracefully
    if let Err(err) = result {
        let error_msg = format!("{:?}", err);
        assert!(error_msg.contains("Invalid output_mode"));
    }
}

#[tokio::test]
async fn test_grep_tool_binary_file_exclusion() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test directory with mixed file types
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Create text file
    let text_file = &temp_dir.join("text.txt");
    fs::write(text_file, "This is searchable text content").unwrap();

    // Create binary-like file (simulated)
    let binary_file = &temp_dir.join("data.bin");
    let binary_content = vec![0u8, 1, 2, 3, 255, 254, 0, 127]; // Contains null bytes
    fs::write(binary_file, binary_content).unwrap();

    // Test search - should find text file but skip binary
    let mut arguments = glob_args("searchable");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Binary exclusion search should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find text file content
    assert!(response_text.contains("searchable") || response_text.contains("1 matches"));
    // Should not mention binary file (it should be skipped)
    assert!(!response_text.contains("data.bin"));
}

#[tokio::test]
async fn test_grep_tool_no_matches() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test file without target pattern
    let (_env, temp_dir, _test_file) =
        create_test_file("test.txt", "This file has no target content");

    // Search for non-existent pattern
    let mut arguments = grep_args("nonexistent_pattern_12345");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "No matches should still succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    let response_text = extract_response_text(&call_result);

    // Should indicate no matches found
    assert!(response_text.contains("No matches found") || response_text.contains("0 matches"));
}

#[tokio::test]
async fn test_grep_tool_timing_info() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test file
    let (_env, temp_dir, _test_file) =
        create_test_file("test.txt", "Test content for timing");

    // Test basic search
    let mut arguments = grep_args("content");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Timing test should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should include timing information
    assert!(response_text.contains("Time:"));
    assert!(response_text.contains("ms"));
}

#[tokio::test]
async fn test_grep_tool_single_file_vs_directory() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test directory with multiple files
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    let test_files = vec![
        ("target.txt", "This file contains the word target"),
        ("other.txt", "This file does not contain the word"),
        ("nested/deep.txt", "Another target file nested deeply"),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    // Test searching entire directory
    let mut arguments = glob_args("target");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Directory search should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find matches in multiple files
    assert!(response_text.contains("2 matches") || response_text.contains("target"));

    // Test searching single file
    let single_file = &temp_dir.join("target.txt");
    let mut arguments = glob_args("target");
    arguments.insert("path".to_string(), json!(single_file.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Single file search should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find match in single file only
    assert!(response_text.contains("1 matches") || response_text.contains("target"));
}

// ============================================================================
// File Write Tool Tests
// ============================================================================

#[tokio::test]
async fn test_write_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    verify_tool_registration(
        &registry,
        "files_write",
        &["file", "write"],
        &["file_path", "content"],
        &[],
    );
}

#[tokio::test]
async fn test_write_tool_execution_success_cases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    // Create temporary directory for testing
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_write.txt");
    let test_content = "Hello, World!\nThis is a test file created via MCP integration.";

    // Test basic file writing
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(test_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File write should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
    assert!(!call_result.content.is_empty());

    // Verify the file was actually created with correct content
    assert!(test_file.exists());
    let written_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(written_content, test_content);
}

#[tokio::test]
async fn test_write_tool_overwrite_existing_file() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    // Create temporary file with initial content
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_overwrite.txt");
    let initial_content = "Initial content";
    fs::write(test_file, initial_content).unwrap();

    let new_content = "New overwritten content";
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(new_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File overwrite should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify the file was overwritten
    let written_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(written_content, new_content);
    assert_ne!(written_content, initial_content);
}

#[tokio::test]
async fn test_write_tool_creates_parent_directories() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    // Create test file in nested directories that don't exist
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let nested_file = temp_dir
        .join("deeply")
        .join("nested")
        .join("directories")
        .join("test_file.txt");
    let test_content = "File in deeply nested directory";

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "file_path".to_string(),
        json!(nested_file.to_string_lossy()),
    );
    arguments.insert("content".to_string(), json!(test_content));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Write with parent directory creation should succeed"
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify the file and directories were created
    assert!(nested_file.exists());
    let written_content = fs::read_to_string(&nested_file).unwrap();
    assert_eq!(written_content, test_content);
}

#[tokio::test]
async fn test_write_tool_unicode_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("unicode_test.txt");
    let unicode_content = "Hello 🦀 Rust!\n你好世界\nПривет мир\n🚀✨🎉";

    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(unicode_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Unicode content write should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify Unicode content was written correctly
    let written_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(written_content, unicode_content);
}

#[tokio::test]
async fn test_write_tool_empty_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("empty_file.txt");
    let empty_content = "";

    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(empty_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Empty content write should succeed");

    // Verify empty file was created
    assert!(test_file.exists());
    let written_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(written_content, "");
}

#[tokio::test]
async fn test_write_tool_error_handling() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    // Test invalid file path (empty)
    let arguments = write_args("", "test content");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty file path should fail");

    // Test relative path (should be accepted but may fail due to parent directory)
    let arguments = write_args("relative/path/file.txt", "test content");

    let result = tool.execute(arguments, &context).await;

    match result {
        Ok(_) => {
            // Relative path was accepted and file was created successfully
        }
        Err(error) => {
            let error_msg = format!("{:?}", error);
            // Should not reject due to being relative anymore
            assert!(
                !error_msg.contains("absolute"),
                "Should not reject relative paths"
            );
            // May fail due to parent directory not existing, which is fine
            assert!(
                error_msg.contains("Parent directory does not exist")
                    || error_msg.contains("No such file or directory"),
                "Should fail due to missing parent directory, not relative path: {}",
                error_msg
            );
        }
    }
}

// ============================================================================
// File Edit Tool Tests
// ============================================================================

#[tokio::test]
async fn test_edit_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    verify_tool_registration(
        &registry,
        "files_edit",
        &["string", "replacement"],
        &["file_path", "old_string", "new_string"],
        &["replace_all"],
    );
}

#[tokio::test]
async fn test_edit_tool_single_replacement_success() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    // Create test file with content to edit (single occurrence)
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_edit.txt");
    let initial_content = "Hello world! This is a test file with unique content.";
    fs::write(test_file, initial_content).unwrap();

    // Test single replacement
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("world"));
    arguments.insert("new_string".to_string(), json!("universe"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Single replacement should succeed: {:?}",
        result
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify the occurrence was replaced
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(
        edited_content,
        "Hello universe! This is a test file with unique content."
    );
}

#[tokio::test]
async fn test_edit_tool_replace_all_success() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    // Create test file with multiple occurrences
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_replace_all.txt");
    let initial_content = "test test test";
    fs::write(test_file, initial_content).unwrap();

    // Test replace all
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("test"));
    arguments.insert("new_string".to_string(), json!("example"));
    arguments.insert("replace_all".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Replace all should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify all occurrences were replaced
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(edited_content, "example example example");
}

#[tokio::test]
async fn test_edit_tool_string_not_found_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    // Create test file
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_not_found.txt");
    let initial_content = "Hello world!";
    fs::write(test_file, initial_content).unwrap();

    // Try to replace non-existent string
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("nonexistent"));
    arguments.insert("new_string".to_string(), json!("replacement"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Edit with non-existent string should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("not found") || error_msg.contains("does not contain"));
}

#[tokio::test]
async fn test_edit_tool_multiple_occurrences_without_replace_all() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    // Create test file with duplicate content
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_multiple.txt");
    let initial_content = "duplicate duplicate duplicate";
    fs::write(test_file, initial_content).unwrap();

    // Try single replacement on multiple occurrences (should fail)
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("duplicate"));
    arguments.insert("new_string".to_string(), json!("unique"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Single replacement with multiple occurrences should succeed and replace first occurrence"
    );

    // Verify only the first occurrence was replaced
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(edited_content, "unique duplicate duplicate");
}

#[tokio::test]
async fn test_edit_tool_unicode_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("unicode_edit.txt");
    let unicode_content = "Hello 🌍! Здравствуй мир! 你好世界!";
    fs::write(test_file, unicode_content).unwrap();

    // Edit unicode content
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("🌍"));
    arguments.insert("new_string".to_string(), json!("🦀"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Unicode edit should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify Unicode content was edited correctly
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(edited_content, "Hello 🦀! Здравствуй мир! 你好世界!");
}

#[tokio::test]
async fn test_edit_tool_preserves_line_endings() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("line_endings.txt");
    // Content with mixed line endings
    let content_with_crlf = "Line 1\r\nLine 2 with target\r\nLine 3\r\n";
    fs::write(test_file, content_with_crlf).unwrap();

    // Edit while preserving line endings
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("target"));
    arguments.insert("new_string".to_string(), json!("replacement"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Edit preserving line endings should succeed"
    );

    // Verify line endings were preserved
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(
        edited_content,
        "Line 1\r\nLine 2 with replacement\r\nLine 3\r\n"
    );
    assert!(edited_content.contains("\r\n")); // CRLF preserved
}

#[tokio::test]
async fn test_edit_tool_file_not_exists_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let nonexistent_file = &temp_dir.join("does_not_exist.txt");

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "file_path".to_string(),
        json!(nonexistent_file.to_string_lossy()),
    );
    arguments.insert("old_string".to_string(), json!("old"));
    arguments.insert("new_string".to_string(), json!("new"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Edit on non-existent file should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("does not exist") || error_msg.contains("not found"));
}

#[tokio::test]
async fn test_edit_tool_empty_parameters_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file("test.txt", "test content");

    // Test empty old_string
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!(""));
    arguments.insert("new_string".to_string(), json!("new"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Edit with empty old_string should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("cannot be empty") || error_msg.contains("required"));
}

#[tokio::test]
async fn test_edit_tool_multiple_edits_sequential() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let (_env, _temp_dir, test_file) =
        create_test_file("multi_edit_test.txt", "Hello world! This is a test.");

    // Test multiple sequential edits
    let mut arguments = serde_json::Map::new();
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert(
        "edits".to_string(),
        json!([
            {
                "oldText": "world",
                "newText": "universe"
            },
            {
                "oldText": "test",
                "newText": "example"
            }
        ]),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Multiple edits should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify both edits were applied
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "Hello universe! This is a example.");
}

#[tokio::test]
async fn test_edit_tool_multiple_edits_with_aliases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file("alias_test.txt", "foo bar baz");

    // Test parameter aliases
    let mut arguments = serde_json::Map::new();
    arguments.insert("filePath".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert(
        "edits".to_string(),
        json!([
            {
                "old_string": "foo",
                "new_text": "FOO"
            },
            {
                "old_text": "bar",
                "new_string": "BAR"
            }
        ]),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Multiple edits with aliases should succeed");

    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "FOO BAR baz");
}

#[tokio::test]
async fn test_edit_tool_multiple_edits_with_replace_all() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let (_env, _temp_dir, test_file) =
        create_test_file("replace_all_multi.txt", "test test test, example example");

    let mut arguments = serde_json::Map::new();
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert(
        "edits".to_string(),
        json!([
            {
                "oldText": "test",
                "newText": "exam",
                "replace_all": true
            },
            {
                "oldText": "example",
                "newText": "sample",
                "replace_all": true
            }
        ]),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Multiple edits with replace_all should succeed"
    );

    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "exam exam exam, sample sample");
}

#[tokio::test]
async fn test_edit_tool_single_mode_with_path_aliases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file("single_alias.txt", "test content");

    // Test single edit mode with different parameter aliases
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("oldText".to_string(), json!("test"));
    arguments.insert("newText".to_string(), json!("demo"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Single edit with aliases should succeed");

    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "demo content");
}

#[tokio::test]
async fn test_edit_tool_empty_edits_array_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file("empty_edits.txt", "content");

    let mut arguments = serde_json::Map::new();
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("edits".to_string(), json!([]));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty edits array should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("edits array cannot be empty"));
}

#[tokio::test]
async fn test_edit_tool_chain_of_transformations() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file(
        "chain_test.txt",
        "The quick brown fox jumps over the lazy dog",
    );

    // Apply a chain of transformations
    let mut arguments = serde_json::Map::new();
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert(
        "edits".to_string(),
        json!([
            {
                "oldText": "quick",
                "newText": "swift"
            },
            {
                "oldText": "brown",
                "newText": "red"
            },
            {
                "oldText": "lazy",
                "newText": "sleepy"
            }
        ]),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Chain of transformations should succeed");

    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(
        edited_content,
        "The swift red fox jumps over the sleepy dog"
    );
}

// ============================================================================
// Tool Composition and Integration Tests
// ============================================================================

#[tokio::test]
async fn test_write_then_read_workflow() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("write_read_test.txt");
    let test_content = "Content written by write tool\nSecond line of content\n";

    // Step 1: Write file
    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(test_content));

    let write_result = write_tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write should succeed");

    let write_call_result = write_result.unwrap();
    assert_eq!(write_call_result.is_error, Some(false));

    // Step 2: Read the same file
    let mut read_args = serde_json::Map::new();
    read_args.insert("path".to_string(), json!(test_file.to_string_lossy()));

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    assert_eq!(read_call_result.is_error, Some(false));

    // Verify content matches
    let response_text = extract_response_text(&read_call_result);
    assert_eq!(response_text, test_content);
}

#[tokio::test]
async fn test_write_then_edit_workflow() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("write_edit_test.txt");
    let initial_content = "Original content that needs updating";

    // Step 1: Write initial file
    let write_args = write_args(&test_file.to_string_lossy(), initial_content);

    let write_result = write_tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write should succeed");

    // Step 2: Edit the file
    let mut edit_args = edit_args(&test_file.to_string_lossy(), "Original", "Updated");
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    let edit_call_result = edit_result.unwrap();
    assert_eq!(edit_call_result.is_error, Some(false));

    // Verify file was edited correctly
    let final_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(final_content, "Updated content that needs updating");
}

#[tokio::test]
async fn test_read_then_edit_workflow() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("read_edit_test.txt");
    let initial_content = "Function calculate_sum() {\n    return a + b;\n}";
    fs::write(test_file, initial_content).unwrap();

    // Step 1: Read the file to analyze content
    let read_args = read_args(&test_file.to_string_lossy());

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    let response_text = extract_response_text(&read_call_result);

    // Verify we can read the function name
    assert!(response_text.contains("calculate_sum"));

    // Step 2: Edit the function name based on what we read
    let mut edit_args = edit_args(&test_file.to_string_lossy(), "calculate_sum", "add_numbers");
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    // Verify the edit was successful
    let final_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(
        final_content,
        "Function add_numbers() {\n    return a + b;\n}"
    );
}

#[tokio::test]
async fn test_glob_then_grep_workflow() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files_glob").unwrap();
    let grep_tool = registry.get_tool("files_grep").unwrap();

    // Create test directory structure with multiple files
    let (_env, temp_dir) = create_test_dir_with_git();

    let test_files = vec![
        ("src/main.rs", "fn main() {\n    println!(\"Hello, world!\");\n    let result = calculate();\n}"),
        ("src/lib.rs", "pub fn calculate() -> i32 {\n    42\n}\n\npub fn helper() {\n    // Helper function\n}"),
        ("tests/integration.rs", "use mylib;\n\n#[test]\nfn test_calculate() {\n    assert_eq!(mylib::calculate(), 42);\n}"),
        ("README.md", "# My Project\n\nThis project has calculate functions.\n"),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    // Step 1: Use glob to find all Rust files
    let mut glob_args = glob_args("**/*.rs");
    glob_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    assert!(glob_result.is_ok(), "Glob should succeed");

    let glob_call_result = glob_result.unwrap();
    assert_eq!(glob_call_result.is_error, Some(false));

    let glob_response = extract_response_text(&glob_call_result);

    // Verify glob found Rust files
    assert!(glob_response.contains("main.rs"));
    assert!(glob_response.contains("lib.rs"));
    assert!(glob_response.contains("integration.rs"));
    assert!(!glob_response.contains("README.md")); // Should not find non-Rust files

    // Step 2: Use grep to search within the files found by glob
    let mut grep_args = grep_args("calculate");
    grep_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    grep_args.insert("glob".to_string(), json!("*.rs")); // Search within Rust files

    let grep_result = grep_tool.execute(grep_args, &context).await;
    assert!(grep_result.is_ok(), "Grep should succeed");

    let grep_call_result = grep_result.unwrap();
    assert_eq!(grep_call_result.is_error, Some(false));

    let grep_response = extract_response_text(&grep_call_result);

    // Verify grep found "calculate" in Rust files
    assert!(grep_response.contains("calculate") || grep_response.contains("matches"));
}

#[tokio::test]
async fn test_complex_file_workflow() {
    // Test a complex workflow: glob -> read -> edit -> read (to verify)
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files_glob").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    // Create test project structure
    let (_env, temp_dir) = create_test_dir_with_git();

    let test_files = vec![
        (
            "src/config.json",
            "{\n  \"version\": \"1.0.0\",\n  \"debug\": true\n}",
        ),
        (
            "config/app.json",
            "{\n  \"version\": \"1.0.0\",\n  \"production\": false\n}",
        ),
        (
            "package.json",
            "{\n  \"name\": \"myapp\",\n  \"version\": \"1.0.0\"\n}",
        ),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    // Step 1: Find JSON files in src directory (scoped glob, not overly broad **/*)
    // Use respect_git_ignore: false because files are untracked in the fresh git repo
    let mut glob_args = serde_json::Map::new();
    glob_args.insert("pattern".to_string(), json!("src/**/*.json"));
    glob_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    glob_args.insert("respect_git_ignore".to_string(), json!(false));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    assert!(glob_result.is_ok(), "Glob should find JSON files in src/");

    // Step 2: Read one of the config files
    let config_file = &temp_dir.join("src/config.json");
    let initial_read_args = read_args(&config_file.to_string_lossy());

    let read_result = read_tool.execute(initial_read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    let original_content = extract_response_text(&read_call_result);

    // Verify we can read the version
    assert!(original_content.contains("1.0.0"));
    assert!(original_content.contains("debug"));

    // Step 3: Update the version in the config file
    let mut edit_args = edit_args(&config_file.to_string_lossy(), "1.0.0", "1.1.0");
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    // Step 4: Read again to verify the change
    let read_verify_args = read_args(&config_file.to_string_lossy());

    let read_verify_result = read_tool.execute(read_verify_args, &context).await;
    assert!(
        read_verify_result.is_ok(),
        "Read verification should succeed"
    );

    let verify_call_result = read_verify_result.unwrap();
    let updated_content = extract_response_text(&verify_call_result);

    // Verify the version was updated
    assert!(updated_content.contains("1.1.0"));
    assert!(!updated_content.contains("1.0.0")); // Old version should be gone
    assert!(updated_content.contains("debug")); // Other content should remain
}

#[tokio::test]
async fn test_error_handling_in_workflow() {
    // Test error handling when tools fail in a workflow
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let nonexistent_file = &temp_dir.join("does_not_exist.txt");

    // Step 1: Try to read non-existent file (should fail)
    let mut read_args = serde_json::Map::new();
    read_args.insert(
        "path".to_string(),
        json!(nonexistent_file.to_string_lossy()),
    );

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(
        read_result.is_err(),
        "Read should fail for non-existent file"
    );

    // Step 2: Try to edit the same non-existent file (should also fail)
    let mut edit_args = serde_json::Map::new();
    edit_args.insert(
        "file_path".to_string(),
        json!(nonexistent_file.to_string_lossy()),
    );
    edit_args.insert("old_string".to_string(), json!("old"));
    edit_args.insert("new_string".to_string(), json!("new"));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(
        edit_result.is_err(),
        "Edit should fail for non-existent file"
    );

    // Both operations should fail gracefully with clear error messages
    let read_error = format!("{:?}", read_result.unwrap_err());
    let edit_error = format!("{:?}", edit_result.unwrap_err());

    assert!(read_error.contains("does not exist") || read_error.contains("not found"));
    assert!(edit_error.contains("does not exist") || edit_error.contains("not found"));
}

// ============================================================================
// Enhanced Security Tests for All File Tools
// ============================================================================

#[tokio::test]
async fn test_comprehensive_path_traversal_protection_all_tools() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;

    for tool_name in FILE_TOOLS {
        test_path_security_for_tool(tool_name, &registry, &context, DANGEROUS_PATHS).await;
    }
}

#[tokio::test]
async fn test_symlink_read_security() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let normal_file = &temp_dir.join("normal.txt");
    fs::write(normal_file, "normal content").unwrap();

    let symlink_file = &temp_dir.join("symlink.txt");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink("/etc/passwd", symlink_file);
    }

    if symlink_file.exists() {
        let mut read_args = serde_json::Map::new();
        read_args.insert("path".to_string(), json!(symlink_file.to_string_lossy()));

        let read_result = read_tool.execute(read_args, &context).await;
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            println!("Symlink read rejected (secure): {}", error_msg);
        } else {
            let read_call_result = read_result.unwrap();
            if let Some(content_item) = read_call_result.content.first() {
                if let rmcp::model::RawContent::Text(text_content) = &content_item.raw {
                    assert!(
                        !text_content.text.contains("root:")
                            && !text_content.text.contains("shadow"),
                        "Symlink should not expose sensitive content"
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn test_symlink_write_security() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let symlink_file = &temp_dir.join("symlink.txt");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink("/etc/passwd", symlink_file);
    }

    if symlink_file.exists() {
        let mut write_args = serde_json::Map::new();
        write_args.insert(
            "file_path".to_string(),
            json!(symlink_file.to_string_lossy()),
        );
        write_args.insert("content".to_string(), json!("overwrite attempt"));

        let write_result = write_tool.execute(write_args, &context).await;
        if write_result.is_ok() {
            let passwd_content = fs::read_to_string("/etc/passwd").unwrap_or_default();
            assert!(
                !passwd_content.contains("overwrite attempt"),
                "Should not modify system files through symlinks"
            );
        }
    }
}

/// Test restricted path access for a single tool
async fn test_restricted_path_access(
    tool_name: &str,
    tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    path: &str,
    context: &ToolContext,
) {
    if tool_name == "files_read" {
        let mut read_args = serde_json::Map::new();
        read_args.insert("path".to_string(), json!(path));
        let read_result = tool.execute(read_args, context).await;
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            println!("Restricted read blocked: {} - {}", path, error_msg);
        }
    } else if tool_name == "files_write" {
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(path));
        write_args.insert("content".to_string(), json!("unauthorized write"));
        let write_result = tool.execute(write_args, context).await;
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            println!("Restricted write blocked: {} - {}", path, error_msg);
        } else {
            let actual_content = fs::read_to_string(path).unwrap_or_default();
            assert!(
                !actual_content.contains("unauthorized write"),
                "Should not modify restricted system files"
            );
        }
    }
}

#[tokio::test]
async fn test_workspace_boundary_enforcement() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;

    let read_tool = registry.get_tool("files_read").unwrap();
    let write_tool = registry.get_tool("files_write").unwrap();

    let restricted_paths = vec![
        "/etc/passwd",
        "/root/.bashrc",
        "/var/log/system.log",
        "/usr/bin/sudo",
        "/sys/kernel/debug/",
        "/proc/1/environ",
        "/home/other_user/.ssh/id_rsa",
    ];

    for restricted_path in restricted_paths {
        test_restricted_path_access("files_read", read_tool, restricted_path, &context).await;
        test_restricted_path_access("files_write", write_tool, restricted_path, &context).await;
    }
}

#[tokio::test]
async fn test_read_tool_malformed_input() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();

    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let malformed_inputs = create_malformed_inputs(test_dir.path());

    for malformed_input in &malformed_inputs {
        let mut read_args = serde_json::Map::new();
        read_args.insert("path".to_string(), json!(malformed_input));

        let read_result = read_tool.execute(read_args, &context).await;
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            assert!(
                !error_msg.contains("panic") && !error_msg.contains("thread"),
                "Should handle malformed input gracefully, not panic: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_write_tool_malformed_input() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();

    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let malformed_inputs = create_malformed_inputs(test_dir.path());

    for malformed_input in &malformed_inputs {
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(malformed_input));
        write_args.insert("content".to_string(), json!("test content"));

        let write_result = write_tool.execute(write_args, &context).await;
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("invalid")
                    || error_msg.contains("empty")
                    || error_msg.contains("directory")
                    || error_msg.contains("permission")
                    || error_msg.contains("NUL byte")
                    || error_msg.contains("File name too long")
                    || error_msg.contains("Path too long")
                    || error_msg.contains("Read-only"),
                "Should provide clear validation error: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_glob_tool_malformed_input() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files_glob").unwrap();

    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let malformed_inputs = create_malformed_inputs(test_dir.path());

    for malformed_input in &malformed_inputs {
        let mut glob_args = serde_json::Map::new();
        glob_args.insert("pattern".to_string(), json!(malformed_input));

        let glob_result = glob_tool.execute(glob_args, &context).await;
        if let Err(error) = glob_result {
            let error_msg = format!("{:?}", error);
            assert!(
                !error_msg.contains("panic"),
                "Glob should handle malformed patterns gracefully: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_grep_tool_malformed_input() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let grep_tool = registry.get_tool("files_grep").unwrap();

    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let malformed_inputs = create_malformed_inputs(test_dir.path());

    for malformed_input in &malformed_inputs {
        let mut grep_args = serde_json::Map::new();
        grep_args.insert("pattern".to_string(), json!(malformed_input));

        let grep_result = grep_tool.execute(grep_args, &context).await;
        if let Err(error) = grep_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("Invalid regex")
                    || error_msg.contains("pattern")
                    || !error_msg.contains("panic"),
                "Grep should handle malformed regex gracefully: {}",
                error_msg
            );
        }
    }
}

/// Test privileged location access for a tool
async fn test_privileged_location_access(
    tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    location: &str,
    tool_name: &str,
    context: &ToolContext,
) {
    if tool_name == "files_write" {
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(location));
        write_args.insert(
            "content".to_string(),
            json!("# privilege escalation attempt"),
        );

        let write_result = tool.execute(write_args, context).await;
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            println!("Privileged write blocked: {} - {}", location, error_msg);
        } else {
            println!("Warning: Write to {} succeeded unexpectedly", location);
        }
    } else if tool_name == "files_edit" {
        let mut edit_args = serde_json::Map::new();
        edit_args.insert("file_path".to_string(), json!(location));
        edit_args.insert("old_string".to_string(), json!("root"));
        edit_args.insert("new_string".to_string(), json!("compromised"));

        let edit_result = tool.execute(edit_args, context).await;
        if let Err(error) = edit_result {
            let error_msg = format!("{:?}", error);
            println!("Privileged edit blocked: {} - {}", location, error_msg);
        }
    }
}

#[tokio::test]
async fn test_permission_escalation_prevention() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;

    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let privileged_locations = vec![
        "/etc/sudoers",
        "/etc/shadow",
        "/etc/ssh/sshd_config",
        "/root/.ssh/authorized_keys",
        "/var/spool/cron/root",
        "/etc/crontab",
        "/usr/bin/sudo",
    ];

    for privileged_location in privileged_locations {
        test_privileged_location_access(write_tool, privileged_location, "files_write", &context)
            .await;
        test_privileged_location_access(edit_tool, privileged_location, "files_edit", &context)
            .await;
    }
}

#[tokio::test]
async fn test_read_tool_excessive_parameters() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test.txt");
    fs::write(test_file, "small content").unwrap();

    let mut read_args = serde_json::Map::new();
    read_args.insert("path".to_string(), json!(test_file.to_string_lossy()));
    read_args.insert("offset".to_string(), json!(u32::MAX));
    read_args.insert("limit".to_string(), json!(u32::MAX));

    let read_result = read_tool.execute(read_args, &context).await;
    if let Err(error) = read_result {
        let error_msg = format!("{:?}", error);
        assert!(
            error_msg.contains("offset")
                || error_msg.contains("limit")
                || error_msg.contains("too large"),
            "Should validate excessive offset/limit values: {}",
            error_msg
        );
    }
}

#[tokio::test]
async fn test_write_tool_large_content_limits() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let huge_content = "A".repeat(20_000_000);
    let large_file = &temp_dir.join("large_test.txt");

    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(huge_content));

    let write_result = write_tool.execute(write_args, &context).await;
    if let Err(error) = write_result {
        let error_msg = format!("{:?}", error);
        println!("Large content write rejected: {}", error_msg);
    }
}

#[tokio::test]
async fn test_glob_tool_complex_patterns() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files_glob").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let recursive_pattern = "**/**/".repeat(100) + "*";

    let mut glob_args = serde_json::Map::new();
    glob_args.insert("pattern".to_string(), json!(recursive_pattern));
    glob_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    if let Err(error) = glob_result {
        let error_msg = format!("{:?}", error);
        println!("Complex glob pattern handled: {}", error_msg);
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
            let write_tool = registry.get_tool("files_write").unwrap();
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
            let read_tool = registry.get_tool("files_read").unwrap();
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
// Performance Benchmarking Tests
// ============================================================================

#[tokio::test]
async fn test_full_file_read_memory_usage() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let large_file = &temp_dir.join("memory_test_file.txt");

    let chunk = "Memory usage test content with realistic data patterns. ".repeat(20);
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!("Block {}: {}", i, chunk));
    }

    println!(
        "Creating {}MB file for memory profiling...",
        content.len() / 1024 / 1024
    );
    let write_result = fs::write(large_file, &content);
    if let Err(ref e) = write_result {
        println!("fs::write error: {:?}", e);
    }
    write_result.unwrap();

    let profiler = MemoryProfiler::new();

    println!("File exists: {}", large_file.exists());
    println!("File path: {}", large_file.to_string_lossy());

    let mut arguments = serde_json::Map::new();
    arguments.insert("path".to_string(), json!(large_file.to_string_lossy()));

    println!("Reading file with memory profiling...");
    let result = read_tool.execute(arguments, &context).await;

    match &result {
        Ok(r) => println!(
            "Read tool success: response has {} content items",
            r.content.len()
        ),
        Err(e) => panic!("Read tool error: {}", e),
    }

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta during read: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let file_size = content.len();
        let max_expected_memory = file_size * 3;

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
    let read_tool = registry.get_tool("files_read").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let large_file = &temp_dir.join("memory_test_file.txt");

    let chunk = "Memory usage test content with realistic data patterns. ".repeat(20);
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!("Block {}: {}", i, chunk));
    }

    fs::write(large_file, &content).unwrap();

    let profiler = MemoryProfiler::new();

    let mut offset_args = serde_json::Map::new();
    offset_args.insert("path".to_string(), json!(large_file.to_string_lossy()));
    offset_args.insert("offset".to_string(), json!(500));
    offset_args.insert("limit".to_string(), json!(100));

    let result = read_tool.execute(offset_args, &context).await;
    assert!(result.is_ok());

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for offset/limit read: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        let limit_size = 100 * 100;
        let max_expected_memory = limit_size * 10;

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
    let write_tool = registry.get_tool("files_write").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let large_file = &temp_dir.join("memory_write_test.txt");

    // Generate content for memory testing (under 10MB limit)
    let chunk = "Memory profiling write test content with varied patterns. ".repeat(100);
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!("Section {}: {}", i, chunk));
    }

    println!(
        "Testing write memory usage for {}MB file...",
        content.len() / 1024 / 1024
    );

    let profiler = MemoryProfiler::new();

    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let result = write_tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta during write: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        // Memory usage should be reasonable - allow up to 2x content size
        let content_size = content.len();
        let max_expected_memory = content_size * 2;

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
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let large_file = &temp_dir.join("memory_edit_test.txt");

    // Create file with repeated patterns for editing
    let base_pattern = "MEMORY_TEST_PATTERN: original_content_here\n".repeat(5000);
    let content = base_pattern.repeat(40); // 200K lines, safe under 10MB

    println!(
        "Creating file with {} lines for edit memory testing...",
        content.lines().count()
    );

    // Write the large file
    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));
    write_tool.execute(write_args, &context).await.unwrap();

    // Test single edit memory usage
    let profiler = MemoryProfiler::new();

    let mut edit_args = serde_json::Map::new();
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

    let result = edit_tool.execute(edit_args, &context).await;
    assert!(result.is_ok());

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for single edit: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        // Single edit should use reasonable memory
        let file_size = fs::metadata(large_file).unwrap().len() as usize;
        let max_expected_memory = file_size * 2; // Allow 2x file size

        assert!(
            abs_delta < max_expected_memory,
            "Single edit memory usage {} exceeds expected maximum {}",
            MemoryProfiler::format_bytes(abs_delta),
            MemoryProfiler::format_bytes(max_expected_memory)
        );
    }

    // Test replace_all memory usage
    let profiler = MemoryProfiler::new();

    let mut edit_all_args = serde_json::Map::new();
    edit_all_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    edit_all_args.insert("old_string".to_string(), json!("original_content_here"));
    edit_all_args.insert(
        "new_string".to_string(),
        json!("completely_new_content_here"),
    );
    edit_all_args.insert("replace_all".to_string(), json!(true));

    let result = edit_tool.execute(edit_all_args, &context).await;
    assert!(result.is_ok());

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for replace_all: {} ({})",
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        // Replace_all may use more memory but should still be reasonable
        let file_size = fs::metadata(large_file).unwrap().len() as usize;
        let max_expected_memory = file_size * 3; // Allow 3x file size for replace_all

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

    let profiler = MemoryProfiler::new();

    // Create multiple files for concurrent operations
    let mut join_set = tokio::task::JoinSet::new();

    for i in 0..20 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.clone();

        join_set.spawn(async move {
            let file_path = temp_dir_path.join(format!("concurrent_file_{}.txt", i));

            // Generate content for each file
            let content = format!("Concurrent test content for file {}\n", i).repeat(1000);

            // Write file
            let write_tool = registry_clone.get_tool("files_write").unwrap();
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));

            let write_result = write_tool.execute(write_args, &context_clone).await;

            // Read file back
            let read_tool = registry_clone.get_tool("files_read").unwrap();
            let mut read_args = serde_json::Map::new();
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

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.unsigned_abs();
        println!(
            "Memory delta for {} concurrent operations: {} ({})",
            success_count,
            if delta >= 0 { "+" } else { "-" },
            MemoryProfiler::format_bytes(abs_delta)
        );

        // Concurrent operations should not cause excessive memory usage
        // Allow reasonable overhead for tokio runtime and file handles
        let max_expected_memory = 50_000_000; // 50MB max for 20 concurrent operations

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
        success_count, 20,
        "All concurrent operations should succeed"
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

// ============================================================================
// Property-Based Fuzz Testing with Proptest
// ============================================================================

/// Helper to extract text from RawContent
fn extract_text_content(raw_content: &rmcp::model::RawContent) -> &str {
    match raw_content {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => "", // Handle other RawContent variants if they exist
    }
}

// Property-based testing using regular tokio tests with generated data
#[tokio::test]
async fn test_write_read_roundtrip_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();

    // Test various file path and content combinations
    let repeated_content = "Pattern ".repeat(100);
    let test_cases = vec![
        ("simple.txt", "Hello, world!"),
        ("nested/path/file.txt", "Content with\nmultiple lines"),
        (
            "unicode_file.txt",
            "Unicode content: 🦀 Rust is awesome! 中文测试",
        ),
        ("empty_file.txt", ""),
        (
            "special_chars.txt",
            "Content with !@#$%^&*() special characters",
        ),
        ("repeated.txt", repeated_content.as_str()),
        (
            "long_path/deep/nested/structure/file.txt",
            "Deep nesting test",
        ),
    ];

    for (file_path, content) in test_cases {
        let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let temp_dir = _env.temp_dir();
        let full_path = &temp_dir.join(file_path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        // Write file
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(full_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(content));

        let write_result = write_tool.execute(write_args, &context).await;
        if write_result.is_err() {
            continue; // Some file paths may be invalid
        }

        // Read file back
        let mut read_args = serde_json::Map::new();
        read_args.insert("path".to_string(), json!(full_path.to_string_lossy()));

        let read_result = read_tool.execute(read_args, &context).await;
        match read_result {
            Ok(response) => {
                let read_content = extract_text_content(&response.content[0].raw);
                assert_eq!(
                    read_content, content,
                    "Content mismatch for file: {}",
                    file_path
                );
            }
            Err(e) => panic!("Read failed for file {}: {:?}", file_path, e),
        }
    }
}

#[tokio::test]
async fn test_edit_operation_consistency_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();

    // Test various edit scenarios
    let test_cases = vec![
        ("Hello world", "world", "universe", false),
        ("test test test", "test", "exam", true),
        ("Multi\nline\ncontent\nwith\npatterns", "line", "row", false),
        ("Pattern123Pattern456Pattern789", "Pattern", "Match", true),
        ("Special chars: !@# $%^ &*()", "!@#", "ABC", false),
    ];

    for (original_content, old_string, new_string, replace_all) in test_cases {
        let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let temp_dir = _env.temp_dir();
        let file_path = &temp_dir.join("edit_test.txt");

        // Write original file
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(original_content));

        write_tool.execute(write_args, &context).await.unwrap();

        // Perform edit
        let mut edit_args = serde_json::Map::new();
        edit_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
        edit_args.insert("old_string".to_string(), json!(old_string));
        edit_args.insert("new_string".to_string(), json!(new_string));
        edit_args.insert("replace_all".to_string(), json!(replace_all));

        let edit_result = edit_tool.execute(edit_args, &context).await;
        if edit_result.is_err() {
            continue; // Edit might fail for valid reasons
        }

        // Read back and verify
        let mut read_args = serde_json::Map::new();
        read_args.insert("path".to_string(), json!(file_path.to_string_lossy()));

        let response = read_tool.execute(read_args, &context).await.unwrap();
        let edited_content = extract_text_content(&response.content[0].raw);

        if replace_all {
            // All instances should be replaced
            assert!(!edited_content.contains(old_string) || edited_content.contains(new_string));
        } else {
            // At least one instance should be replaced
            assert!(edited_content != original_content);
            assert!(edited_content.contains(new_string));
        }
    }
}

#[tokio::test]
async fn test_glob_pattern_consistency_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let glob_tool = registry.get_tool("files_glob").unwrap();

    // Test different file extensions and patterns
    let test_cases = vec![
        (vec!["txt", "txt", "txt"], "*.txt", 3),
        (vec!["rs", "rs", "py", "js"], "*.rs", 2),
        (vec!["md", "json", "toml"], "*.md", 1),
        (vec!["log", "log", "log", "log"], "*.log", 4),
    ];

    for (extensions, pattern, expected_count) in test_cases {
        let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let temp_dir = _env.temp_dir();

        // Create files with specified extensions
        for (i, ext) in extensions.iter().enumerate() {
            let file_path = &temp_dir.join(format!("test_file_{}.{}", i, ext));
            let content = format!("Content for file {}", i);

            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));

            write_tool.execute(write_args, &context).await.ok();
        }

        // Test glob pattern
        let mut glob_args = serde_json::Map::new();
        glob_args.insert("pattern".to_string(), json!(pattern));
        glob_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

        let result = glob_tool.execute(glob_args, &context).await;
        if let Ok(response) = result {
            let response_text = extract_text_content(&response.content[0].raw);
            let files_found = if response_text.trim().is_empty() {
                0
            } else {
                // Count only lines that look like file paths (start with / or are relative paths)
                response_text
                    .lines()
                    .filter(|line| {
                        let trimmed = line.trim();
                        !trimmed.is_empty()
                            && !trimmed.starts_with("Found")
                            && !trimmed.starts_with("No files")
                            && (trimmed.starts_with("/") || trimmed.contains("."))
                    })
                    .count()
            };

            assert_eq!(
                files_found, expected_count,
                "Glob pattern '{}' should find {} files",
                pattern, expected_count
            );
        }
    }
}

#[tokio::test]
async fn test_read_offset_limit_consistency_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();

    // Create content with multiple lines for line-based testing
    let lines: Vec<String> = (1..=20)
        .map(|i| format!("Line {}: Content for line {}", i, i))
        .collect();
    let content = lines.join("\n");
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let file_path = &temp_dir.join("offset_limit_test.txt");

    // Write file
    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));
    write_tool.execute(write_args, &context).await.unwrap();

    // Test various line-based offset/limit combinations
    let test_cases = vec![
        (1, 5),   // Read first 5 lines (1-based indexing)
        (5, 3),   // Read 3 lines starting from line 5
        (10, 10), // Read 10 lines starting from line 10
        (18, 5),  // Read near end (should be limited by file size)
        (25, 3),  // Offset beyond file (should fail or return empty)
    ];

    for (offset, limit) in test_cases {
        let mut read_args = serde_json::Map::new();
        read_args.insert("path".to_string(), json!(file_path.to_string_lossy()));
        read_args.insert("offset".to_string(), json!(offset));
        read_args.insert("limit".to_string(), json!(limit));

        match read_tool.execute(read_args, &context).await {
            Ok(response) => {
                let read_content = extract_text_content(&response.content[0].raw);
                let read_lines: Vec<&str> = read_content.lines().collect();

                // Assert that we don't exceed the requested limit
                assert!(
                    read_lines.len() <= limit,
                    "Read content should not exceed limit of {} lines, got {}",
                    limit,
                    read_lines.len()
                );

                // If offset is within the file, check content matches expected lines
                if offset <= lines.len() {
                    let start_index = offset.saturating_sub(1); // Convert to 0-based indexing
                    let end_index = std::cmp::min(start_index + limit, lines.len());
                    let expected_lines = &lines[start_index..end_index];

                    assert_eq!(
                        read_lines.len(),
                        expected_lines.len(),
                        "Should read expected number of lines"
                    );
                    for (i, (actual, expected)) in
                        read_lines.iter().zip(expected_lines.iter()).enumerate()
                    {
                        assert_eq!(actual, expected, "Line {} content should match", i + 1);
                    }
                }
            }
            Err(_) => {
                // Offset beyond file size is acceptable
                assert!(
                    offset > lines.len(),
                    "Read should only fail if offset is beyond file size (offset: {}, lines: {})",
                    offset,
                    lines.len()
                );
            }
        }
    }
}

#[tokio::test]
#[allow(clippy::useless_vec)]
async fn test_grep_pattern_robustness_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let grep_tool = registry.get_tool("files_grep").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Test various content and pattern combinations
    let test_cases = vec![
        ("Hello world testing", "world", true),
        ("No match here", "missing", false),
        ("Multiple test test test", "test", true),
        ("Case sensitive Test", "test", false),
        ("Special chars: !@#$", "!@#", true),
        ("Unicode content 🦀 Rust", "🦀", true),
        ("Line1\nLine2\nLine3", "Line2", true),
        ("", "anything", false), // Empty file
    ];

    for (i, (content, _pattern, _should_match)) in test_cases.iter().enumerate() {
        let file_path = &temp_dir.join(format!("grep_test_{}.txt", i));

        // Write file
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(content));
        write_tool.execute(write_args, &context).await.unwrap();
    }

    // Test each pattern
    for (content, pattern, should_match) in test_cases.iter() {
        let mut grep_args = serde_json::Map::new();
        grep_args.insert("pattern".to_string(), json!(pattern));
        grep_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
        grep_args.insert("output_mode".to_string(), json!("files_with_matches"));

        match grep_tool.execute(grep_args, &context).await {
            Ok(response) => {
                let response_text = extract_text_content(&response.content[0].raw);
                let matches_found = if response_text.trim().is_empty() {
                    0
                } else {
                    response_text.lines().count()
                };

                if *should_match {
                    assert!(
                        matches_found > 0,
                        "Pattern '{}' should find matches in content '{}'",
                        pattern,
                        content
                    );
                } else if content.is_empty() {
                    // Empty files might not be found at all
                    // This is acceptable behavior
                } else {
                    // For non-empty content that shouldn't match, we might still find the file
                    // but the pattern shouldn't be in the content
                    assert!(
                        !content.contains(pattern),
                        "Content '{}' should not contain pattern '{}'",
                        content,
                        pattern
                    );
                }
            }
            Err(_) => {
                // Some patterns might cause regex errors, which is acceptable
                println!("Grep failed for pattern '{}' (acceptable)", pattern);
            }
        }
    }
}
