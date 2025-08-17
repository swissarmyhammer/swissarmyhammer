//! Integration tests for web search functionality
//!
//! These tests verify that the web search CLI command works end-to-end with real queries
//! and returns actual results from search engines like Wikipedia.

use assert_cmd::Command;
use predicates::prelude::*;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;

#[test]
fn test_web_search_pear_query_integration() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Run the actual CLI command that was failing in the issue
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", "what is a pear?"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Search Results for"))
        .stdout(predicate::str::contains("pear"));
}

#[test]
fn test_web_search_wikipedia_results() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Run a search that should return Wikipedia results
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", "what is a pear?", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wikipedia"))
        .stdout(predicate::str::contains("results"));
}

#[test]
fn test_web_search_with_results_limit() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test with limited results
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", "what is a pear?", "--results", "3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Search Results for"));
}

#[test]
fn test_web_search_error_handling_empty_query() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test error handling with empty query
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty").or(predicate::str::contains("query")));
}

#[test]
fn test_web_search_invalid_results_count() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test with invalid results count (too high)
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", "test query", "--results", "1000"])
        .assert()
        .failure();
}

#[test]
fn test_web_search_invalid_format() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test with invalid output format
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", "test query", "--format", "invalid"])
        .assert()
        .failure();
}

#[test]
fn test_web_search_extremely_long_query() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test with extremely long query that should be rejected
    let long_query = "a".repeat(1000);
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", &long_query])
        .assert()
        .failure();
}

#[test]
fn test_web_search_special_characters_query() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test with special characters in query that should be handled gracefully
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", "test & query < > \" '"])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::contains("Search Results for"));
}

#[test]
fn test_web_search_unicode_query() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test with Unicode characters
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "web-search", "search", "测试 русский العربية"])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::contains("Search Results for"));
}