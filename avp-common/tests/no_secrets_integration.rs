//! Integration tests for the no-secrets validator.
//!
//! These tests verify that the no-secrets validator correctly:
//! 1. Loads from builtins
//! 2. Matches PostToolUse hooks for Write/Edit operations on code files
//! 3. Parses validator responses correctly
//! 4. Produces Claude Code compatible output format
//! 5. Executes via PlaybackAgent for deterministic testing

mod test_helpers;

use agent_client_protocol::StopReason;
use avp_common::{
    context::AvpContext,
    strategy::ClaudeCodeHookStrategy,
    types::HookType,
    validator::{parse_validator_response, ExecutedValidator, ValidatorLoader, ValidatorResult},
};
use test_helpers::{
    assert_message_contains, assert_validator_failed, assert_validator_passed,
    create_context_with_playback, create_test_context, HookInputBuilder,
};

/// Code sample containing hardcoded secrets (should trigger validator failure).
const CODE_WITH_SECRETS: &str = r#"
const config = {
    // This contains hardcoded secrets that should be detected
    apiKey: "sk-proj-1234567890abcdefghijklmnopqrstuvwxyz",
    awsAccessKey: "AKIAIOSFODNN7EXAMPLE",
    password: "super_secret_password_123",
    databaseUrl: "postgresql://admin:mysecretpassword@localhost:5432/mydb"
};

export default config;
"#;

/// Code sample without secrets (should pass validation).
const CODE_WITHOUT_SECRETS: &str = r#"
const config = {
    // All secrets come from environment variables
    apiKey: process.env.API_KEY,
    awsAccessKey: process.env.AWS_ACCESS_KEY,
    databaseUrl: process.env.DATABASE_URL
};

export default config;
"#;

// ============================================================================
// Validator Loading Tests
// ============================================================================

#[test]
fn test_no_secrets_validator_loads() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let validator = loader.get("no-secrets");
    assert!(validator.is_some(), "no-secrets validator should be loaded");

    let validator = validator.unwrap();
    assert_eq!(validator.name(), "no-secrets");
    assert!(
        validator.body.contains("hardcoded secrets"),
        "validator body should mention hardcoded secrets"
    );
}

// ============================================================================
// Validator Matching Tests
// ============================================================================

#[test]
#[serial_test::serial(cwd)]
fn test_no_secrets_validator_matches_post_tool_use_write() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    let input = HookInputBuilder::post_tool_use_write("config.ts", CODE_WITH_SECRETS);
    let matching = strategy.matching_validators(HookType::PostToolUse, &input);

    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
    assert!(
        names.contains(&"no-secrets"),
        "no-secrets validator should match PostToolUse + Write + *.ts, got: {:?}",
        names
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_no_secrets_validator_does_not_match_non_code_files() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    let input =
        HookInputBuilder::post_tool_use_write("readme.md", "# README\nThis is documentation.");
    let matching = strategy.matching_validators(HookType::PostToolUse, &input);

    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
    assert!(
        !names.contains(&"no-secrets"),
        "no-secrets validator should not match *.md files, but got: {:?}",
        names
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_no_secrets_validator_matches_edit_tool() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    // Edit tool should also match
    let input = serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/test-transcript.jsonl",
        "cwd": "/tmp",
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Edit",
        "tool_input": {
            "file_path": "config.py",
            "old_string": "api_key = None",
            "new_string": "api_key = 'sk-1234'"
        },
        "tool_response": "success",
        "tool_use_id": "toolu_test456"
    });

    let matching = strategy.matching_validators(HookType::PostToolUse, &input);
    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
    assert!(
        names.contains(&"no-secrets"),
        "no-secrets validator should match PostToolUse + Edit + *.py, got: {:?}",
        names
    );
}

// ============================================================================
// Response Parsing Tests
// ============================================================================

#[test]
fn test_parse_validator_response_failed_with_secrets() {
    // Simulated Claude response when secrets are detected
    let response = r#"{"status": "failed", "message": "Found 3 potential secrets - Line 4: Possible API key 'sk-proj-...' in variable 'apiKey'; Line 5: AWS access key detected; Line 6: Hardcoded password"}"#;

    let result = parse_validator_response(response, &StopReason::EndTurn);

    assert!(!result.passed(), "Should fail when secrets detected");
    assert!(
        result.message().contains("secret")
            || result.message().contains("API")
            || result.message().contains("password"),
        "Message should describe findings: {}",
        result.message()
    );
}

#[test]
fn test_parse_validator_response_passed_clean_code() {
    // Simulated Claude response when code is clean
    let response = r#"{"status": "passed", "message": "No hardcoded secrets detected. All sensitive values are properly retrieved from environment variables."}"#;

    let result = parse_validator_response(response, &StopReason::EndTurn);

    assert!(result.passed(), "Should pass when no secrets found");
    assert!(
        result.message().contains("No hardcoded secrets"),
        "Message should confirm clean code: {}",
        result.message()
    );
}

#[test]
fn test_parse_validator_response_with_markdown_wrapper() {
    // Claude sometimes wraps JSON in markdown
    let response = r#"Here's my analysis:

```json
{"status": "failed", "message": "Found hardcoded API key"}
```

The code contains secrets."#;

    let result = parse_validator_response(response, &StopReason::EndTurn);

    assert!(!result.passed(), "Should fail when secrets detected");
    assert!(
        result.message().contains("API key"),
        "Should extract message from markdown: {}",
        result.message()
    );
}

#[test]
fn test_parse_validator_response_handles_malformed_duplicates() {
    // Sometimes streaming causes duplicated content
    let response = r#"```json
{"status": "passed", "message": "passed", "message": "No secrets detected"}"#;

    let result = parse_validator_response(response, &StopReason::EndTurn);

    // Should still detect the status: passed
    assert!(
        result.passed(),
        "Should handle malformed response gracefully"
    );
}

// ============================================================================
// Output Format Tests (Claude Code Compatibility)
// ============================================================================

#[test]
fn test_post_tool_use_block_output_format() {
    let output = avp_common::types::HookOutput::post_tool_use_block(
        "blocked by validator 'no-secrets': Found hardcoded API key",
    );

    let json = serde_json::to_string(&output).unwrap();

    // Verify Claude Code format
    assert!(
        output
            .decision
            .as_ref()
            .map(|d| d == "block")
            .unwrap_or(false),
        "Should have decision: 'block'"
    );
    assert!(output.reason.is_some(), "Should have a reason");
    assert!(
        output.continue_execution,
        "PostToolUse blocking should have continue: true (tool already ran)"
    );

    // Verify JSON structure
    assert!(
        json.contains(r#""decision":"block""#),
        "JSON should contain decision: block"
    );
    assert!(
        json.contains("no-secrets"),
        "Reason should mention validator name"
    );
}

#[test]
fn test_executed_validator_blocking_detection() {
    use avp_common::validator::Severity;

    // Error severity + failed = blocking
    let blocking = ExecutedValidator {
        name: "no-secrets".to_string(),
        severity: Severity::Error,
        result: ValidatorResult::fail("Found hardcoded API key"),
    };
    assert!(blocking.is_blocking(), "Error + Failed should be blocking");

    // Warn severity + failed = not blocking
    let warning = ExecutedValidator {
        name: "no-secrets".to_string(),
        severity: Severity::Warn,
        result: ValidatorResult::fail("Found hardcoded API key"),
    };
    assert!(
        !warning.is_blocking(),
        "Warn + Failed should not be blocking"
    );

    // Error severity + passed = not blocking
    let passed = ExecutedValidator {
        name: "no-secrets".to_string(),
        severity: Severity::Error,
        result: ValidatorResult::pass("Clean code"),
    };
    assert!(
        !passed.is_blocking(),
        "Error + Passed should not be blocking"
    );
}

// ============================================================================
// PlaybackAgent Integration Tests
// ============================================================================

/// Integration test using PlaybackAgent to verify validator detects secrets.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_no_secrets_validator_detects_secrets_playback() {
    use avp_common::validator::ValidatorRunner;

    let (temp, _) = create_test_context();

    // Create context with PlaybackAgent using the secrets-detected fixture
    let context = create_context_with_playback(&temp, "no_secrets_detect_secrets.json");

    // Get agent from context and create runner
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    // Load the no-secrets validator
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("no-secrets").unwrap();

    // Build input with secrets
    let input = HookInputBuilder::post_tool_use_write("config.ts", CODE_WITH_SECRETS);

    // Execute the validator
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::PostToolUse, &input, None)
        .await;

    // The validator should FAIL (secrets detected)
    assert_validator_failed(&result, "when secrets are present");
    assert_message_contains(&result, &["secret", "API"]);
}

/// Integration test using PlaybackAgent to verify validator passes clean code.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_no_secrets_validator_passes_clean_code_playback() {
    use avp_common::validator::ValidatorRunner;

    let (temp, _) = create_test_context();

    // Create context with PlaybackAgent using the clean-code fixture
    let context = create_context_with_playback(&temp, "no_secrets_clean_code.json");

    // Get agent from context and create runner
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    // Load the no-secrets validator
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("no-secrets").unwrap();

    // Build input with clean code
    let input = HookInputBuilder::post_tool_use_write("config.ts", CODE_WITHOUT_SECRETS);

    // Execute the validator
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::PostToolUse, &input, None)
        .await;

    // The validator should PASS (no secrets)
    assert_validator_passed(&result, "when code uses environment variables");
    assert_message_contains(&result, &["No hardcoded secrets", "environment"]);
}

// ============================================================================
// Live Integration Tests (require Claude CLI)
// ============================================================================

/// Integration test that actually calls Claude to validate secrets detection.
///
/// Run with: `cargo test -p avp-common --test no_secrets_integration -- --ignored`
#[tokio::test]
#[ignore]
#[serial_test::serial(cwd)]
async fn test_no_secrets_validator_detects_secrets_live() {
    use avp_common::validator::ValidatorRunner;

    let (temp, _) = create_test_context();

    // Change to temp directory to create live context
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create live context and get agent
    let context = AvpContext::init().expect("Should create context");
    let (agent, notifications) = context.agent().await.expect("Claude CLI required");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    std::env::set_current_dir(&original_dir).unwrap();

    // Load the no-secrets validator
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("no-secrets").unwrap();

    // Build input with secrets
    let input = HookInputBuilder::post_tool_use_write("config.ts", CODE_WITH_SECRETS);

    // Execute the validator
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::PostToolUse, &input, None)
        .await;

    // The validator should FAIL
    assert_validator_failed(&result, "when secrets are present");
}

/// Integration test that verifies clean code passes validation.
///
/// Run with: `cargo test -p avp-common --test no_secrets_integration -- --ignored`
#[tokio::test]
#[ignore]
#[serial_test::serial(cwd)]
async fn test_no_secrets_validator_passes_clean_code_live() {
    use avp_common::validator::ValidatorRunner;

    let (temp, _) = create_test_context();

    // Change to temp directory to create live context
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create live context and get agent
    let context = AvpContext::init().expect("Should create context");
    let (agent, notifications) = context.agent().await.expect("Claude CLI required");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    std::env::set_current_dir(&original_dir).unwrap();

    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("no-secrets").unwrap();

    let input = HookInputBuilder::post_tool_use_write("config.ts", CODE_WITHOUT_SECRETS);

    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::PostToolUse, &input, None)
        .await;

    assert_validator_passed(&result, "when code uses environment variables");
}
