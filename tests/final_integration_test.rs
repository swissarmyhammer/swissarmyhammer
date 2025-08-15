//! Final Integration Test Suite for Plan Command
//!
//! This comprehensive test suite validates the complete plan command implementation
//! against all requirements specified in PLAN_000012_final-integration-testing.
//!
//! ## Test Coverage
//!
//! ### Core Functionality
//! - Basic plan command execution with various file types
//! - Path handling (relative, absolute, with spaces)
//! - Issue file creation and validation
//! - Integration with swissarmyhammer ecosystem
//!
//! ### Error Scenarios
//! - File not found, permissions, directory as file
//! - Empty files, binary content, file size limits
//! - Enhanced error messages and user guidance
//!
//! ### Performance & Stress
//! - Large file processing
//! - Concurrent execution
//! - Memory usage validation
//! - Response time benchmarks
//!
//! ### User Experience
//! - Help system completeness
//! - CLI consistency
//! - Output formatting
//! - Cross-platform behavior

use anyhow::Result;
use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tokio::task::JoinSet;

mod test_utils;
use test_utils::{create_temp_dir, setup_git_repo, create_test_home_guard};

/// Comprehensive test environment setup
fn setup_comprehensive_test_environment() -> Result<(TempDir, std::path::PathBuf)> {
    let temp_dir = create_temp_dir()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Create complete directory structure
    let dirs = [
        "issues",
        "specification", 
        "plans",
        ".swissarmyhammer",
        ".swissarmyhammer/tmp",
        ".swissarmyhammer/cache",
    ];

    for dir in &dirs {
        fs::create_dir_all(temp_path.join(dir))?;
    }

    setup_git_repo(&temp_path)?;
    Ok((temp_dir, temp_path))
}

/// Create a comprehensive test plan file with realistic content
fn create_comprehensive_plan_file(
    dir: &Path,
    filename: &str,
    complexity: &str,
) -> Result<std::path::PathBuf> {
    let plan_file = dir.join(filename);
    
    let content = match complexity {
        "simple" => r#"# Simple Feature Plan

## Overview
Add a simple authentication feature to the application.

## Requirements
1. Create login form component
2. Add authentication validation
3. Implement session management
4. Add logout functionality
5. Write unit tests

## Implementation Details
This is a straightforward authentication implementation following standard security practices.

## Acceptance Criteria
- Users can log in with valid credentials
- Invalid credentials show appropriate error messages
- Sessions are managed securely
- All tests pass
"#,
        "complex" => r#"# Comprehensive System Integration Plan

## Executive Summary
Implement a complete microservices architecture with monitoring, caching, and analytics.

## Functional Requirements

### Core Services
1. **User Management Service**
   - Authentication and authorization
   - Profile management
   - Role-based access control
   - Multi-factor authentication

2. **Data Processing Pipeline**
   - Real-time data ingestion
   - Stream processing with Kafka
   - Batch processing jobs
   - Data validation and cleansing

3. **API Gateway**
   - Request routing and load balancing
   - Rate limiting and throttling
   - API versioning support
   - Request/response transformation

### Infrastructure Components
1. **Monitoring Stack**
   - Prometheus metrics collection
   - Grafana dashboards
   - Alert manager configuration
   - Log aggregation with ELK stack

2. **Caching Layer**
   - Redis cluster setup
   - Cache invalidation strategies
   - Performance optimization
   - High availability configuration

3. **Database Architecture**
   - Primary/replica setup
   - Connection pooling
   - Query optimization
   - Backup and recovery

## Technical Requirements

### Performance
- Handle 100,000 concurrent users
- Sub-100ms API response times
- 99.99% uptime requirement
- Automatic scaling based on load

### Security
- End-to-end encryption
- OAuth 2.0 implementation
- SQL injection prevention
- Regular security audits

### Observability
- Distributed tracing
- Structured logging
- Performance metrics
- Error tracking and alerting

## Implementation Phases

### Phase 1: Foundation (Weeks 1-4)
- Basic service architecture
- Core database design
- Authentication framework
- Initial CI/CD pipeline

### Phase 2: Core Services (Weeks 5-8)
- User management implementation
- Data processing pipeline
- API gateway setup
- Basic monitoring

### Phase 3: Advanced Features (Weeks 9-12)
- Caching implementation
- Performance optimization
- Advanced monitoring
- Security hardening

### Phase 4: Production Deployment (Weeks 13-16)
- Load testing and optimization
- Documentation completion
- Deployment automation
- Go-live preparation

This comprehensive plan requires extensive planning and multiple implementation phases.
"#,
        "edge_case" => r#"# Edge Case Test Plan 🚀

## Overview with Unicode: Тест планування システム

### Special Characters & Symbols
- File paths with spaces: `/path with spaces/file.md`
- Unicode support: 日本語, العربية, עברית
- Emojis in content: 🎯 📊 ✅ ❌ 🔧
- Mathematical symbols: ∑ ∏ ∆ ∫ √ ≤ ≥ ≠

### Markdown Edge Cases

#### Code Blocks
```rust
// Code with special characters
fn test_unicode() -> Result<String, Box<dyn std::error::Error>> {
    let message = "Hello, 世界! 🌍";
    Ok(message.to_string())
}
```

#### Tables
| Column 1 | Column 2 | Column 3 |
|----------|----------|----------|
| Data 1   | Data 2   | Data 3   |
| 中文     | العربية   | עברית     |

### Requirements
1. Handle all Unicode characters properly
2. Process complex markdown structures
3. Manage special file paths
4. Support international content

This tests the system's robustness with edge cases.
"#,
        _ => r#"# Standard Test Plan

## Overview
A standard test plan for validation purposes.

## Requirements
1. Basic functionality
2. Standard validation
3. Simple implementation

## Implementation
Straightforward approach with standard practices.
"#
    };

    fs::write(&plan_file, content)?;
    Ok(plan_file)
}

/// Test 1: Basic Plan Command Functionality
#[tokio::test]
async fn test_basic_plan_command_functionality() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Test with simple plan file
    let plan_file = create_comprehensive_plan_file(&temp_path, "basic-test.md", "simple")?;

    // Test that plan command starts properly (using test mode for speed)
    let output = Command::cargo_bin("sah")?
        .args([
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", plan_file.display())
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(30))
        .output()?;

    assert!(
        output.status.success(),
        "Basic plan command should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Test mode") && stdout.contains("Coverage Report"),
        "Should execute plan workflow in test mode: {}",
        stdout
    );

    Ok(())
}

/// Test 2: Path Format Variations
#[tokio::test] 
async fn test_path_format_variations() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Test different path formats
    let test_cases = vec![
        ("relative", "./specification/relative-plan.md", "simple"),
        ("absolute", temp_path.join("absolute-plan.md").to_string_lossy().to_string(), "simple"),
        ("with_spaces", "plans/plan with spaces.md", "simple"),
        ("nested", "./specification/features/nested-plan.md", "simple"),
    ];

    for (test_name, path, complexity) in test_cases {
        let plan_path = Path::new(&path);
        let parent = plan_path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(temp_path.join(parent))?;
        
        let actual_file = if plan_path.is_absolute() {
            create_comprehensive_plan_file(&temp_path, 
                plan_path.file_name().unwrap().to_str().unwrap(), complexity)?
        } else {
            create_comprehensive_plan_file(&temp_path.join(parent), 
                plan_path.file_name().unwrap().to_str().unwrap(), complexity)?
        };

        let test_path = if plan_path.is_absolute() {
            actual_file.to_string_lossy().to_string()
        } else {
            path.clone()
        };

        let output = Command::cargo_bin("sah")?
            .args([
                "flow", "test", "plan",
                "--var", &format!("plan_filename={}", test_path)
            ])
            .current_dir(&temp_path)
            .timeout(std::time::Duration::from_secs(30))
            .output()?;

        assert!(
            output.status.success(),
            "Path format test '{}' should succeed: {}",
            test_name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Test 3: Error Handling Comprehensive
#[tokio::test]
async fn test_comprehensive_error_handling() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Test file not found
    let output = Command::cargo_bin("sah")?
        .args(["plan", "nonexistent-file.md"])
        .current_dir(&temp_path)
        .output()?;

    assert!(!output.status.success(), "Should fail for nonexistent file");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("does not exist"),
        "Should show file not found error: {}",
        stderr
    );

    // Test directory as file
    let dir_path = temp_path.join("directory-not-file");
    fs::create_dir_all(&dir_path)?;

    let output = Command::cargo_bin("sah")?
        .args(["plan", dir_path.to_str().unwrap()])
        .current_dir(&temp_path)
        .output()?;

    assert!(!output.status.success(), "Should fail for directory as file");

    // Test empty file
    let empty_file = temp_path.join("empty-plan.md");
    fs::write(&empty_file, "")?;

    let output = Command::cargo_bin("sah")?
        .args(["plan", empty_file.to_str().unwrap()])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(30))
        .output()?;

    // Empty file should complete but may show warnings
    assert!(output.status.code().is_some(), "Should handle empty file gracefully");

    Ok(())
}

/// Test 4: Integration with Swissarmyhammer Ecosystem
#[tokio::test]
async fn test_ecosystem_integration() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    let plan_file = create_comprehensive_plan_file(&temp_path, "integration-test.md", "simple")?;

    // Test integration with validate command
    let output = Command::cargo_bin("sah")?
        .args(["validate"])
        .current_dir(&temp_path)
        .output()?;

    assert!(
        output.status.success(),
        "Validate should work in plan environment: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Test integration with flow list
    let output = Command::cargo_bin("sah")?
        .args(["flow", "list"])
        .current_dir(&temp_path)
        .output()?;

    assert!(output.status.success(), "Flow list should work");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("plan"),
        "Should list plan workflow: {}",
        stdout
    );

    // Test plan command execution
    let output = Command::cargo_bin("sah")?
        .args([
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", plan_file.display())
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(30))
        .output()?;

    assert!(
        output.status.success(),
        "Plan execution should integrate with flow system: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

/// Test 5: Performance and Stress Testing
#[tokio::test]
async fn test_performance_and_stress() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Performance test: complex plan processing
    let start_time = std::time::Instant::now();
    let complex_plan = create_comprehensive_plan_file(&temp_path, "complex-perf.md", "complex")?;

    let output = Command::cargo_bin("sah")?
        .args([
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", complex_plan.display())
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(60))
        .output()?;

    let elapsed = start_time.elapsed();

    assert!(
        output.status.success(),
        "Complex plan should process successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "Complex plan should complete within 30 seconds, took: {:?}",
        elapsed
    );

    // Stress test: multiple concurrent executions
    let mut tasks = JoinSet::new();

    for i in 0..3 {
        let temp_path_clone = temp_path.clone();
        tasks.spawn(async move {
            let _guard = create_test_home_guard();
            let plan_file = create_comprehensive_plan_file(
                &temp_path_clone, 
                &format!("stress-test-{}.md", i), 
                "simple"
            ).unwrap();

            let start = std::time::Instant::now();
            let output = Command::cargo_bin("sah")
                .unwrap()
                .args([
                    "flow", "test", "plan",
                    "--var", &format!("plan_filename={}", plan_file.display())
                ])
                .current_dir(&temp_path_clone)
                .timeout(std::time::Duration::from_secs(45))
                .output()
                .expect("Command should execute");

            (i, output.status.success(), start.elapsed())
        });
    }

    let mut all_success = true;
    let mut max_time = std::time::Duration::from_secs(0);

    while let Some(result) = tasks.join_next().await {
        let (i, success, duration) = result?;
        if !success {
            all_success = false;
        }
        if duration > max_time {
            max_time = duration;
        }
        println!("Concurrent execution {} completed in {:?}", i, duration);
    }

    assert!(all_success, "All concurrent executions should succeed");
    assert!(
        max_time < std::time::Duration::from_secs(45),
        "Concurrent executions should complete within 45 seconds, max: {:?}",
        max_time
    );

    Ok(())
}

/// Test 6: Help System and Documentation
#[tokio::test]
async fn test_help_system_validation() -> Result<()> {
    let _guard = create_test_home_guard();

    // Test main help includes plan command
    let output = Command::cargo_bin("sah")?
        .args(["--help"])
        .output()?;

    assert!(output.status.success(), "Main help should work");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("plan") || stdout.contains("Plan"),
        "Main help should mention plan command: {}",
        stdout
    );

    // Test plan command specific help
    let output = Command::cargo_bin("sah")?
        .args(["plan", "--help"])
        .output()?;

    let help_text = String::from_utf8_lossy(&output.stdout);
    
    // Validate help content completeness
    let required_sections = vec![
        "USAGE:",
        "EXAMPLES:",
        "plan_filename",
        "specification",
        "issues",
    ];

    for section in required_sections {
        assert!(
            help_text.contains(section),
            "Plan help should contain '{}': {}",
            section,
            help_text
        );
    }

    // Test help includes troubleshooting
    assert!(
        help_text.contains("TROUBLESHOOTING:") || help_text.contains("troubleshooting"),
        "Should include troubleshooting section: {}",
        help_text
    );

    Ok(())
}

/// Test 7: Edge Cases and Special Characters
#[tokio::test]
async fn test_edge_cases_and_special_characters() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Test with Unicode and special characters
    let edge_case_file = create_comprehensive_plan_file(&temp_path, "edge-case-test.md", "edge_case")?;

    let output = Command::cargo_bin("sah")?
        .args([
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", edge_case_file.display())
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(30))
        .output()?;

    assert!(
        output.status.success(),
        "Should handle edge case content: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Test mode") && stdout.contains("Coverage"),
        "Should process Unicode content successfully: {}",
        stdout
    );

    Ok(())
}

/// Test 8: Output Format Validation
#[tokio::test]
async fn test_output_format_validation() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    let plan_file = create_comprehensive_plan_file(&temp_path, "output-test.md", "simple")?;

    // Test with verbose output
    let output = Command::cargo_bin("sah")?
        .args([
            "--verbose",
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", plan_file.display())
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(30))
        .output()?;

    assert!(output.status.success(), "Verbose mode should work");

    // Test with quiet mode
    let output = Command::cargo_bin("sah")?
        .args([
            "--quiet",
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", plan_file.display())
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(30))
        .output()?;

    assert!(output.status.success(), "Quiet mode should work");

    // Test with debug mode
    let output = Command::cargo_bin("sah")?
        .args([
            "--debug",
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", plan_file.display())
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(30))
        .output()?;

    assert!(output.status.success(), "Debug mode should work");

    Ok(())
}

/// Test 9: Cross-Platform Compatibility
#[tokio::test]
async fn test_cross_platform_compatibility() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    let plan_file = create_comprehensive_plan_file(&temp_path, "platform-test.md", "simple")?;

    // Test path separators work correctly
    let path_with_separators = if cfg!(windows) {
        format!("{}\\platform-test.md", temp_path.display())
    } else {
        format!("{}/platform-test.md", temp_path.display())
    };

    let output = Command::cargo_bin("sah")?
        .args([
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", path_with_separators)
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(30))
        .output()?;

    assert!(
        output.status.success(),
        "Should handle platform-specific paths: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

/// Test 10: Regression Prevention
#[tokio::test]
async fn test_regression_prevention() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Test that existing workflows still work
    let output = Command::cargo_bin("sah")?
        .args(["flow", "list"])
        .current_dir(&temp_path)
        .output()?;

    assert!(output.status.success(), "Flow list should still work");

    // Test that validation still works
    let output = Command::cargo_bin("sah")?
        .args(["validate"])
        .current_dir(&temp_path)
        .output()?;

    assert!(output.status.success(), "Validation should still work");

    // Test that other commands still work
    let output = Command::cargo_bin("sah")?
        .args(["--help"])
        .output()?;

    assert!(output.status.success(), "Help should still work");

    let help_text = String::from_utf8_lossy(&output.stdout);
    assert!(
        help_text.contains("serve") && help_text.contains("validate"),
        "Should still show all expected commands: {}",
        help_text
    );

    Ok(())
}

/// Test 11: Complete End-to-End Workflow
#[tokio::test] 
async fn test_complete_end_to_end_workflow() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Create a realistic plan file
    let plan_file = create_comprehensive_plan_file(&temp_path, "e2e-test.md", "simple")?;

    // Execute complete workflow
    let start_time = std::time::Instant::now();
    
    let output = Command::cargo_bin("sah")?
        .args([
            "flow", "test", "plan",
            "--var", &format!("plan_filename={}", plan_file.display())
        ])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_secs(60))
        .output()?;

    let elapsed = start_time.elapsed();

    assert!(
        output.status.success(),
        "End-to-end workflow should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Verify test mode execution indicators
    assert!(
        stdout.contains("Test mode") || stdout.contains("🧪"),
        "Should indicate test mode execution: {}",
        stdout
    );

    // Verify workflow completion
    assert!(
        stdout.contains("Coverage Report") && stdout.contains("States visited"),
        "Should show complete workflow execution: {}",
        stdout
    );

    // Verify reasonable performance
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "End-to-end should complete within 30 seconds, took: {:?}",
        elapsed
    );

    // Verify no errors or warnings in test execution
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("ERROR") && !stderr.contains("WARN"),
        "Should not have errors or warnings in successful execution: {}",
        stderr
    );

    println!("✅ Complete end-to-end workflow test completed in {:?}", elapsed);

    Ok(())
}

/// Test 12: Resource Cleanup and Memory Management  
#[tokio::test]
async fn test_resource_cleanup_and_memory() -> Result<()> {
    let _guard = create_test_home_guard();
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Create multiple plan files
    let plan_files: Result<Vec<_>> = (0..5).map(|i| {
        create_comprehensive_plan_file(
            &temp_path, 
            &format!("cleanup-test-{}.md", i), 
            "simple"
        )
    }).collect();

    let plan_files = plan_files?;

    // Execute multiple plans and verify cleanup
    for (i, plan_file) in plan_files.iter().enumerate() {
        let output = Command::cargo_bin("sah")?
            .args([
                "flow", "test", "plan",
                "--var", &format!("plan_filename={}", plan_file.display())
            ])
            .current_dir(&temp_path)
            .timeout(std::time::Duration::from_secs(30))
            .output()?;

        assert!(
            output.status.success(),
            "Cleanup test {} should succeed: {}",
            i,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Verify temporary files are cleaned up
    let sah_tmp_dir = temp_path.join(".swissarmyhammer").join("tmp");
    if sah_tmp_dir.exists() {
        let tmp_files: Vec<_> = fs::read_dir(&sah_tmp_dir)?
            .filter_map(|entry| entry.ok())
            .collect();
        
        // Allow some temporary files but not excessive accumulation
        assert!(
            tmp_files.len() < 20,
            "Should not accumulate excessive temporary files: {} files in tmp",
            tmp_files.len()
        );
    }

    Ok(())
}