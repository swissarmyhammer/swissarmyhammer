//! Integration tests for the no-test-cheating validator.
//!
//! These tests verify that the no-test-cheating validator correctly:
//! 1. Loads from builtins with partial support
//! 2. Matches PostToolUse hooks for edit/write operations on test files
//! 3. Detects test.skip and similar patterns
//! 4. Executes via PlaybackAgent for deterministic testing

mod test_helpers;

use avp_common::{
    strategy::ClaudeCodeHookStrategy,
    types::HookType,
    validator::{ValidatorLoader, ValidatorRunner},
};
use test_helpers::{create_context_with_playback, create_test_context, HookInputBuilder};

// ============================================================================
// Validator Loading Tests
// ============================================================================

#[test]
fn test_no_test_cheating_validator_loads() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let validator = loader.get("no-test-cheating");
    assert!(
        validator.is_some(),
        "no-test-cheating validator should be loaded"
    );

    let validator = validator.unwrap();
    assert_eq!(validator.name(), "no-test-cheating");
    assert!(
        validator.body.contains("test.skip") || validator.body.contains("skip"),
        "validator body should mention test skipping patterns"
    );
}

#[test]
fn test_no_test_cheating_validator_includes_partial() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let validator = loader.get("no-test-cheating").unwrap();

    // The validator should contain the include directive for the partial
    assert!(
        validator.body.contains("include 'test-remediation'"),
        "validator body should include the test-remediation partial"
    );
}

#[test]
fn test_test_remediation_partial_in_builtins() {
    // Check that the test-remediation partial is in the raw builtins
    let builtins = avp_common::builtin::validators_raw();
    let partial = builtins
        .iter()
        .find(|(name, _)| name.contains("test-remediation"));

    assert!(
        partial.is_some(),
        "test-remediation partial should be in builtins"
    );

    let (name, content) = partial.unwrap();
    assert!(
        name.starts_with("_partials/"),
        "partial name should start with _partials/, got: {}",
        name
    );
    assert!(
        content.contains("Alternative Approaches"),
        "partial should contain alternative approaches content"
    );
}

// ============================================================================
// Validator Matching Tests
// ============================================================================

#[test]
#[serial_test::serial(cwd)]
fn test_no_test_cheating_validator_matches_write_to_test_file() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    let input = HookInputBuilder::post_tool_use_write(
        "src/utils.test.ts",
        "test.skip('broken test', () => {})",
    );
    let matching = strategy.matching_validators(HookType::PostToolUse, &input);

    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
    assert!(
        names.contains(&"no-test-cheating"),
        "no-test-cheating validator should match PostToolUse + Write to .test.ts file, got: {:?}",
        names
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_no_test_cheating_validator_matches_source_files() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    // Source files should match because tests can be embedded anywhere
    let input = HookInputBuilder::post_tool_use_write("src/utils.ts", "export const foo = 'bar';");

    let matching = strategy.matching_validators(HookType::PostToolUse, &input);
    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
    assert!(
        names.contains(&"no-test-cheating"),
        "no-test-cheating validator should match source files (tests can be anywhere), but got: {:?}",
        names
    );
}

// ============================================================================
// PlaybackAgent Integration Tests
// ============================================================================

/// Integration test using PlaybackAgent to verify validator detects test.skip.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_no_test_cheating_validator_detects_skip_playback() {
    let (temp, _) = create_test_context();

    // Create context with PlaybackAgent using the skip-detected fixture
    let context = create_context_with_playback(&temp, "no_test_cheating_detect_skip.json");

    // Get agent from context and create runner
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    // Load the no-test-cheating validator
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("no-test-cheating").unwrap();

    // Build input with test file containing test.skip
    let input = HookInputBuilder::post_tool_use_write(
        "src/feature.test.ts",
        r#"
describe('feature', () => {
    test.skip('broken test', () => {
        expect(true).toBe(true);
    });
});
"#,
    );

    // Execute the validator
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::PostToolUse, &input, None)
        .await;

    // The validator should FAIL (test.skip detected)
    assert!(
        !result.result.passed(),
        "Validator should fail when test.skip is used. Got result: {:?}",
        result
    );
    assert!(
        result.result.message().contains("skip")
            || result.result.message().contains("test integrity"),
        "Message should mention skip or test integrity: {}",
        result.result.message()
    );
}

/// Integration test using PlaybackAgent to verify validator passes for clean tests.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_no_test_cheating_validator_passes_clean_code_playback() {
    let (temp, _) = create_test_context();

    // Create context with PlaybackAgent using the clean-code fixture
    let context = create_context_with_playback(&temp, "no_test_cheating_clean_code.json");

    // Get agent from context and create runner
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    // Load the no-test-cheating validator
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("no-test-cheating").unwrap();

    // Build input with clean test file
    let input = HookInputBuilder::post_tool_use_write(
        "src/feature.test.ts",
        r#"
describe('feature', () => {
    test('should work correctly', () => {
        const result = calculateSum(1, 2);
        expect(result).toBe(3);
    });
});
"#,
    );

    // Execute the validator
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::PostToolUse, &input, None)
        .await;

    // The validator should PASS (clean code)
    assert!(
        result.result.passed(),
        "Validator should pass for clean test code. Got result: {:?}",
        result
    );
}
