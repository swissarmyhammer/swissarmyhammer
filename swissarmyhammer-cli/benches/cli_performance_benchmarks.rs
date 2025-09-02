//! CLI Performance Benchmarks
//!
//! Benchmarks for detecting performance regressions in CLI-MCP integration.
//! These benchmarks measure the performance of CLI commands that use MCP tools.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

/// Setup function to create a standardized benchmark environment
fn setup_benchmark_environment() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_path_buf();

    // Create issues directory
    let issues_dir = temp_path.join("issues");
    std::fs::create_dir_all(&issues_dir).expect("Failed to create issues directory");

    // Create sample issues for benchmarking
    for i in 1..=10 {
        std::fs::write(
            issues_dir.join(format!("BENCH_{i:03}_issue.md")),
            format!(
                r#"# Benchmark Issue {i}

This is benchmark issue number {i} for performance testing.

## Details
- Priority: Medium
- Type: Performance Test
- Created: 2024-01-01
- Iteration: {i}

## Description
This issue exists solely for benchmarking CLI performance.
It contains sufficient content to make operations realistic.
"#
            ),
        )
        .expect("Failed to create benchmark issue");
    }

    // Create .swissarmyhammer directory
    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    std::fs::create_dir_all(&swissarmyhammer_dir)
        .expect("Failed to create .swissarmyhammer directory");

    // Create source files for search benchmarking
    let src_dir = temp_path.join("src");
    std::fs::create_dir_all(&src_dir).expect("Failed to create src directory");

    for i in 1..=5 {
        std::fs::write(
            src_dir.join(format!("benchmark_{i}.rs")),
            format!(
                r#"
//! Benchmark source file {i}

use std::error::Error;

/// Benchmark function {i}
pub fn benchmark_function_{i}() -> Result<String, Box<dyn Error>> {{
    println!("Running benchmark function {i}", {i});
    Ok(format!("Benchmark {i} completed", {i}))
}}

/// Error handling for benchmark {i}
pub fn handle_benchmark_error_{i}(error: &str) -> Result<(), String> {{
    eprintln!("Benchmark error {i}: {{}}", error);
    Err(format!("Benchmark error {i} handled", {i}))
}}

/// Performance critical function {i}
pub fn performance_critical_{i}() {{
    for i in 0..1000 {{
        let _ = i * 2 + 1;
    }}
}}
"#
            ),
        )
        .expect("Failed to create benchmark source file");
    }

    // Initialize git repository for issue operations
    use git2::Repository;

    let repo = Repository::init(&temp_path).expect("Failed to init git repo");

    let mut config = repo.config().expect("Failed to get git config");

    config
        .set_str("user.name", "Benchmark User")
        .expect("Failed to set git user name");

    config
        .set_str("user.email", "benchmark@example.com")
        .expect("Failed to set git user email");

    (temp_dir, temp_path)
}

/// Benchmark issue operations
fn bench_issue_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("issue_operations");
    group.measurement_time(Duration::from_secs(10));

    // Benchmark issue list command
    group.bench_function("issue_list", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        b.iter(|| {
            let output = Command::new("cargo")
                .args(["run", "--bin", "swissarmyhammer", "--", "issue", "list"])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run issue list command");
            black_box(output.status.success())
        })
    });

    // Benchmark issue creation
    group.bench_function("issue_create", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "issue",
                    "create",
                    &format!("bench_issue_{counter}"),
                    "--content",
                    "Benchmark issue content",
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run issue create command");
            black_box(output.status.success())
        })
    });

    // Benchmark issue show
    group.bench_function("issue_show", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        b.iter(|| {
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "issue",
                    "show",
                    "BENCH_001_issue",
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run issue show command");
            black_box(output.status.code())
        })
    });

    group.finish();
}

/// Benchmark memo operations
fn bench_memo_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memo_operations");
    group.measurement_time(Duration::from_secs(10));

    // Benchmark memo creation
    group.bench_function("memo_create", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "memo",
                    "create",
                    &format!("Benchmark Memo {counter}"),
                    "--content",
                    "This is benchmark memo content for performance testing.",
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run memo create command");
            black_box(output.status.success())
        })
    });

    // Benchmark memo listing
    group.bench_function("memo_list", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        // Pre-create some memos
        for i in 1..=5 {
            Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "memo",
                    "create",
                    &format!("Pre-created Memo {i}"),
                    "--content",
                    &format!("Content for memo {i}"),
                ])
                .current_dir(&temp_path)
                .output()
                .expect("Failed to pre-create memo");
        }

        b.iter(|| {
            let output = Command::new("cargo")
                .args(["run", "--bin", "swissarmyhammer", "--", "memo", "list"])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run memo list command");
            black_box(output.status.success())
        })
    });

    group.finish();
}

/// Benchmark search operations
fn bench_search_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_operations");
    group.measurement_time(Duration::from_secs(15));

    // Benchmark search indexing
    group.bench_function("search_index", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        b.iter(|| {
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "search",
                    "index",
                    "src/**/*.rs",
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run search index command");
            black_box(output.status.success())
        })
    });

    // Benchmark search querying (after indexing)
    group.bench_function("search_query", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();

        // Pre-index files
        Command::new("cargo")
            .args([
                "run",
                "--bin",
                "swissarmyhammer",
                "--",
                "search",
                "index",
                "src/**/*.rs",
            ])
            .current_dir(&temp_path)
            .output()
            .expect("Failed to pre-index files");

        b.iter(|| {
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "search",
                    "query",
                    "benchmark function",
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run search query command");
            black_box(output.status.success())
        })
    });

    group.finish();
}

/// Benchmark CLI startup time
fn bench_cli_startup(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_startup");
    group.measurement_time(Duration::from_secs(5));

    // Benchmark help command (minimal operation)
    group.bench_function("help_command", |b| {
        b.iter(|| {
            let output = Command::new("cargo")
                .args(["run", "--bin", "swissarmyhammer", "--", "--help"])
                .output()
                .expect("Failed to run help command");
            black_box(output.status.code())
        })
    });

    // Benchmark version command
    group.bench_function("version_command", |b| {
        b.iter(|| {
            let output = Command::new("cargo")
                .args(["run", "--bin", "swissarmyhammer", "--", "--version"])
                .output()
                .expect("Failed to run version command");
            black_box(output.status.code())
        })
    });

    group.finish();
}

/// Benchmark different output formats
fn bench_output_formats(c: &mut Criterion) {
    let mut group = c.benchmark_group("output_formats");
    group.measurement_time(Duration::from_secs(10));

    let (_temp_dir, temp_path) = setup_benchmark_environment();

    let formats = ["table", "json"];
    for format in &formats {
        group.bench_with_input(
            BenchmarkId::new("issue_list_format", format),
            format,
            |b, format| {
                b.iter(|| {
                    let output = Command::new("cargo")
                        .args([
                            "run",
                            "--bin",
                            "swissarmyhammer",
                            "--",
                            "issue",
                            "list",
                            "--format",
                            format,
                        ])
                        .current_dir(black_box(&temp_path))
                        .output()
                        .expect("Failed to run formatted issue list command");
                    black_box(output.status.success())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark with different data sizes
fn bench_data_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("data_scaling");
    group.measurement_time(Duration::from_secs(15));

    let sizes = [10, 50, 100];
    for size in &sizes {
        group.bench_with_input(
            BenchmarkId::new("issue_list_scaling", size),
            size,
            |b, &size| {
                // Create temporary environment with specified number of issues
                let temp_dir = TempDir::new().expect("Failed to create temp directory");
                let temp_path = temp_dir.path().to_path_buf();
                let issues_dir = temp_path.join("issues");
                std::fs::create_dir_all(&issues_dir).expect("Failed to create issues directory");

                // Create the specified number of issues
                for i in 1..=size {
                    std::fs::write(
                        issues_dir.join(format!("SCALE_{i:03}_issue.md")),
                        format!(
                            r#"# Scaling Issue {i}

This is scaling issue number {i} for performance testing with {size} total issues.

## Content
This issue contains realistic content to test performance at scale.
The goal is to measure how performance changes with data size.
"#
                        ),
                    )
                    .expect("Failed to create scaling issue");
                }

                b.iter(|| {
                    let output = Command::new("cargo")
                        .args(["run", "--bin", "swissarmyhammer", "--", "issue", "list"])
                        .current_dir(black_box(&temp_path))
                        .output()
                        .expect("Failed to run scaled issue list command");
                    black_box(output.status.success())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark error handling performance
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");
    group.measurement_time(Duration::from_secs(5));

    let (_temp_dir, temp_path) = setup_benchmark_environment();

    // Benchmark non-existent issue error
    group.bench_function("nonexistent_issue_error", |b| {
        b.iter(|| {
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "issue",
                    "show",
                    "nonexistent_issue",
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run nonexistent issue command");
            black_box(output.status.code())
        })
    });

    // Benchmark invalid command error
    group.bench_function("invalid_command_error", |b| {
        b.iter(|| {
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "invalid",
                    "command",
                ])
                .output()
                .expect("Failed to run invalid command");
            black_box(output.status.code())
        })
    });

    group.finish();
}

/// Benchmark MCP-specific operations  
fn bench_mcp_integration(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_integration");
    group.measurement_time(Duration::from_secs(10));

    // Benchmark MCP context initialization through CLI command
    group.bench_function("mcp_context_init_via_cli", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();

        b.iter(|| {
            let output = Command::new("cargo")
                .args(["run", "--bin", "swissarmyhammer", "--", "memo", "list"])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run memo list command");
            black_box(output.status.code())
        })
    });

    // Benchmark MCP tool execution through CLI
    group.bench_function("mcp_tool_execution_via_cli", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        let mut counter = 0;

        b.iter(|| {
            counter += 1;
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "memo",
                    "create",
                    &format!("MCP Bench Memo {counter}"),
                    "--content",
                    "MCP benchmark content",
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run MCP benchmark command");
            black_box(output.status.success())
        })
    });

    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    group.measurement_time(Duration::from_secs(5));

    // Benchmark with large content
    group.bench_function("large_content_handling", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        let large_content = "A".repeat(100_000); // 100KB content

        b.iter(|| {
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "issue",
                    "create",
                    "large_content_test",
                    "--content",
                    &large_content,
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run large content command");
            black_box(output.status.code())
        })
    });

    // Benchmark with many small operations
    group.bench_function("many_small_operations", |b| {
        let (_temp_dir, temp_path) = setup_benchmark_environment();
        let mut counter = 0;

        b.iter(|| {
            counter += 1;
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "swissarmyhammer",
                    "--",
                    "memo",
                    "create",
                    &format!("Small Memo {counter}"),
                    "--content",
                    "Small content",
                ])
                .current_dir(black_box(&temp_path))
                .output()
                .expect("Failed to run small operation");
            black_box(output.status.success())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_issue_operations,
    bench_memo_operations,
    bench_search_operations,
    bench_cli_startup,
    bench_output_formats,
    bench_data_scaling,
    bench_error_handling,
    bench_mcp_integration,
    bench_memory_usage
);
criterion_main!(benches);
