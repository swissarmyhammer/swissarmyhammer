//! End-to-End Workflow Tests
//!
//! Tests for complete user journeys that span multiple CLI commands and verify
//! that entire workflows function correctly with the CLI-MCP integration.
//!
//! ## Known Issue: Parallel Test Race Condition
//!
//! These tests have a race condition when run in parallel due to MCP subprocess
//! initialization conflicts. Tests pass reliably when run sequentially.
//!
//! **Workaround**: Run with `cargo test --test e2e_workflow_tests -- --test-threads=1`
//!
//! **Root Cause**: Multiple MCP subprocesses trying to initialize directories
//! simultaneously, causing "Directory not empty (os error 66)" errors.

mod test_utils;

mod in_process_test_utils;

// Commented out: Issue CLI commands have been removed
// /// Helper function to create and validate a new issue in the lifecycle test (optimized)
// async fn create_and_validate_issue(working_dir: &std::path::Path) -> Result<()> {
//     ...
// }
//
// /// Helper function to show, update, and re-validate issue details (optimized)
// async fn show_and_update_issue(working_dir: &std::path::Path) -> Result<()> {
//     ...
// }
//
// /// Helper function to complete issue and validate final state (optimized)
// async fn complete_issue(working_dir: &std::path::Path) -> Result<()> {
//     ...
// }

// Commented out: Test uses removed issue CLI commands
// /// Test complete issue lifecycle workflow (optimized)
// #[tokio::test]
// async fn test_complete_issue_lifecycle() -> Result<()> {
//     ...
// }

// Test temporarily removed due to MCP integration issue in test environment
// TODO: Restore test_realistic_load_workflow after fixing MCP test environment issues
