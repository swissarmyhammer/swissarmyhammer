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
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("web-search")
        .arg("search")
        .arg("what is a pear?")
        .assert()
        .success()
        .stdout(predicate::str::contains("Search Results for"))
        .stdout(predicate::str::contains("pear"));
}

#[test]
fn test_web_search_wikipedia_results() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Run a search that should return Wikipedia results
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("web-search")
        .arg("search")
        .arg("what is a pear?")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("wikipedia"))
        .stdout(predicate::str::contains("results"));
}

#[test]
fn test_web_search_with_results_limit() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test with limited results
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("web-search")
        .arg("search")
        .arg("what is a pear?")
        .arg("--results")
        .arg("3")
        .assert()
        .success()
        .stdout(predicate::str::contains("Search Results for"));
}

#[test]
fn test_web_search_error_handling() {
    let _guard = IsolatedTestEnvironment::new();
    
    // Test error handling with empty query
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("web-search")
        .arg("search")
        .arg("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty"));
}