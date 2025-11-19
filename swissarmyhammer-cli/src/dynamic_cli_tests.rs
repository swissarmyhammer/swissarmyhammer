//! Tests for dynamic CLI builder

use super::*;
use derive_builder::Builder;
use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;

/// Macro to conditionally set builder fields when Option values are Some
macro_rules! setter {
    ($builder:expr, $value:expr, $setter:ident) => {
        if let Some(v) = $value {
            $builder = $builder.$setter(v);
        }
    };
    ($builder:expr, $value:expr, $setter:ident, $transform:expr) => {
        if let Some(v) = $value {
            $builder = $builder.$setter($transform(v));
        }
    };
}

/// Test context containing all components needed for CLI testing
struct TestContext<'a> {
    help: String,
    registry: Arc<tokio::sync::RwLock<ToolRegistry>>,
    workflows: Vec<swissarmyhammer_workflow::Workflow>,
    #[allow(dead_code)]
    storage: Option<&'a swissarmyhammer_workflow::WorkflowStorage>,
}

impl<'a> TestContext<'a> {
    /// Create a new test context with optional workflow storage
    fn new(storage: Option<&'a swissarmyhammer_workflow::WorkflowStorage>) -> Self {
        let components = TestComponents::new(storage);
        let (registry, cli) = components.into_registry_and_cli();
        let help = get_help_text(&cli);
        let workflows = storage
            .and_then(|s| s.list_workflows().ok())
            .unwrap_or_default();

        Self {
            help,
            registry,
            workflows,
            storage,
        }
    }

    /// Get registry categories as a vector of strings
    fn categories(&self) -> Vec<String> {
        let lock = self.registry.try_read().unwrap();
        let categories = lock.get_cli_categories();
        categories.iter().map(|s| s.to_string()).collect()
    }
}

/// Test components containing all elements needed for CLI testing
struct TestComponents {
    registry: Arc<tokio::sync::RwLock<ToolRegistry>>,
    builder: CliBuilder,
    cli: Command,
}

impl TestComponents {
    /// Create new test components with optional workflow storage
    fn new(storage: Option<&swissarmyhammer_workflow::WorkflowStorage>) -> Self {
        let registry = Arc::new(tokio::sync::RwLock::new(ToolRegistry::new()));
        let builder = CliBuilder::new(registry.clone());
        let cli = builder.build_cli(storage);
        Self {
            registry,
            builder,
            cli,
        }
    }

    /// Extract registry and builder
    fn into_registry_and_builder(self) -> (Arc<tokio::sync::RwLock<ToolRegistry>>, CliBuilder) {
        (self.registry, self.builder)
    }

    /// Extract registry and CLI
    fn into_registry_and_cli(self) -> (Arc<tokio::sync::RwLock<ToolRegistry>>, Command) {
        (self.registry, self.cli)
    }
}

/// Helper function to create a test tool registry and CLI builder
fn create_test_registry_and_builder() -> (Arc<tokio::sync::RwLock<ToolRegistry>>, CliBuilder) {
    TestComponents::new(None).into_registry_and_builder()
}

/// Helper function to create a CLI with default configuration for testing
fn create_test_cli_with_defaults() -> (Arc<tokio::sync::RwLock<ToolRegistry>>, Command) {
    TestComponents::new(None).into_registry_and_cli()
}

/// Generic helper to assert text contains all specified items
fn assert_contains_all<S: AsRef<str>>(text: &str, items: &[S], context: &str) {
    for item in items {
        let item_str = item.as_ref();
        assert!(
            text.contains(item_str),
            "{}: should contain '{}', but got: {}",
            context,
            item_str,
            text
        );
    }
}

/// Helper to check if text contains string when option is present
fn contains_if_present(text: &str, item: Option<&str>) -> bool {
    item.map_or(false, |s| text.contains(s))
}

/// Generic helper to assert a property matches an optional expected value
fn assert_optional_property<T: PartialEq + std::fmt::Debug>(
    actual: &T,
    expected: Option<T>,
    property_name: &str,
) {
    if let Some(exp) = expected {
        assert_eq!(actual, &exp, "{} mismatch", property_name);
    }
}

/// Helper to verify validation stats against expected values
fn verify_validation_stats(
    stats: &CliValidationStats,
    expected_all_valid: bool,
    expected_rate: f64,
    expected_summary_contains: &[&str],
    context: &str,
) {
    assert_eq!(stats.is_all_valid(), expected_all_valid, "{}", context);
    assert_eq!(stats.success_rate(), expected_rate, "{}", context);
    let summary = stats.summary();
    assert_contains_all(&summary, expected_summary_contains, context);
}

/// Helper function to find an item in text or panic with context
fn find_or_panic(text: &str, item: &str, context: &str) -> usize {
    text.find(item)
        .unwrap_or_else(|| panic!("{}: '{}' should appear in the help text", context, item))
}

/// Helper function to assert ordering of items in help text
fn assert_help_position_before(help: &str, first_item: &str, second_item: &str, context: &str) {
    let first_pos = find_or_panic(help, first_item, context);
    let second_pos = find_or_panic(help, second_item, context);
    assert!(
        first_pos < second_pos,
        "{}: '{}' should appear before '{}'",
        context,
        first_item,
        second_item
    );
}

/// Extract clap output from a command by triggering specific error kinds
fn extract_clap_output(
    cli: &Command,
    args: &[&str],
    expected_kind: clap::error::ErrorKind,
) -> String {
    match cli.clone().try_get_matches_from(args) {
        Err(e) if e.kind() == expected_kind => format!("{}", e),
        Ok(_) => panic!("Expected {:?} error, but command succeeded", expected_kind),
        Err(e) => panic!(
            "Expected {:?} error, but got {:?}: {:?}",
            expected_kind,
            e.kind(),
            e
        ),
    }
}

/// Helper function to extract help text from a CLI command
fn get_help_text(cli: &Command) -> String {
    use clap::error::ErrorKind;
    extract_clap_output(cli, &["swissarmyhammer", "--help"], ErrorKind::DisplayHelp)
}

/// Helper function to get filtered subcommand names
fn get_filtered_subcommand_names<F>(cli: &Command, filter: F) -> Vec<&str>
where
    F: Fn(&Command) -> bool,
{
    cli.get_subcommands()
        .filter(|cmd| filter(cmd))
        .map(|cmd| cmd.get_name())
        .collect()
}

/// Generic helper to assert items are sorted if multiple exist
fn assert_sorted_if_multiple<T: Clone + Ord + PartialEq + std::fmt::Debug>(
    items: &[T],
    message: &str,
) {
    if items.len() > 1 {
        let mut sorted = items.to_vec();
        sorted.sort();
        assert_eq!(items, &sorted, "{}", message);
    }
}

/// Assert that a specific subcommand exists in CLI
fn assert_subcommand_exists(cli: &Command, name: &str, error_msg: &str) {
    let subcommand_names = get_filtered_subcommand_names(cli, |_| true);
    assert!(subcommand_names.contains(&name), "{}: {}", error_msg, name);
}

/// Assert that expected commands exist in CLI
fn assert_commands_exist(cli: &Command, expected: &[&str]) {
    for cmd in expected {
        assert_subcommand_exists(cli, cmd, "Missing expected command");
    }
}

/// Assert that an item appears before the first MCP tool category in help text if item exists
fn assert_item_before_first_category_if_present(
    ctx: &TestContext,
    item: Option<&str>,
    context_msg: &str,
) {
    if let Some(item_str) = item {
        if contains_if_present(&ctx.help, Some(item_str)) {
            if let Some(first_category) = ctx.categories().into_iter().next() {
                assert_help_position_before(&ctx.help, item_str, &first_category, context_msg);
            }
        }
    }
}

/// Test case for validation statistics
#[derive(Builder)]
#[builder(pattern = "owned")]
struct ValidationTestCase {
    description: &'static str,
    #[builder(default = "0")]
    total: usize,
    #[builder(default = "0")]
    valid: usize,
    #[builder(default = "0")]
    invalid: usize,
    #[builder(default = "0")]
    errors: usize,
    #[builder(default = "true")]
    expected_all_valid: bool,
    #[builder(default = "100.0")]
    expected_rate: f64,
    #[builder(default = "vec![]")]
    expected_summary_contains: Vec<&'static str>,
}

/// Simplified helper to create validation test case with default expectations
fn validation_test(
    description: &'static str,
    total: usize,
    valid: usize,
    invalid: usize,
    errors: usize,
) -> ValidationTestCase {
    let expected_all_valid = invalid == 0 && errors == 0;
    let expected_rate = if total > 0 {
        (valid as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    let mut summary_contains = vec![];
    if total > 0 {
        if expected_all_valid {
            summary_contains.extend_from_slice(&["✅", "All"]);
        } else {
            summary_contains.extend_from_slice(&["⚠️"]);
        }
    }

    ValidationTestCaseBuilder::default()
        .description(description)
        .total(total)
        .valid(valid)
        .invalid(invalid)
        .errors(errors)
        .expected_all_valid(expected_all_valid)
        .expected_rate(expected_rate)
        .expected_summary_contains(summary_contains)
        .build()
        .unwrap()
}

/// Generic helper to assert multiple items appear before first category
fn assert_items_before_categories(ctx: &TestContext, items: &[(&str, &str)]) {
    for (item_name, context_msg) in items {
        assert_item_before_first_category_if_present(ctx, Some(item_name), context_msg);
    }
}

/// Generic helper to assert all collections in context are sorted
fn assert_all_collections_sorted(ctx: &TestContext) {
    let categories = ctx.categories();
    assert_sorted_if_multiple(
        &categories,
        "MCP tool categories should be sorted alphabetically",
    );

    let workflow_names: Vec<String> = ctx.workflows.iter().map(|w| w.name.to_string()).collect();
    assert_sorted_if_multiple(
        &workflow_names,
        "Workflow shortcuts should be sorted alphabetically",
    );
}

#[test]
fn test_string_interning() {
    // Table-driven test for string interning behavior
    let test_cases = vec![
        ("test_string", "test_string", true, "same strings"),
        ("string1", "string2", false, "different strings"),
    ];

    for &(s1, s2, should_equal, description) in &test_cases {
        let ptr1 = intern_string(s1.to_string()) as *const str;
        let ptr2 = intern_string(s2.to_string()) as *const str;
        if should_equal {
            assert_eq!(ptr1, ptr2, "Failed test case: {}", description);
        } else {
            assert_ne!(ptr1, ptr2, "Failed test case: {}", description);
        }
    }
}

#[test]
fn test_validation_stats() {
    // Table-driven test cases for validation stats
    let test_cases = vec![
        {
            let mut case = validation_test("all valid", 10, 10, 0, 0);
            case.expected_summary_contains = vec!["✅", "All 10 CLI tools are valid"];
            case
        },
        {
            let mut case = validation_test("some invalid", 10, 7, 3, 5);
            case.expected_summary_contains = vec!["⚠️", "7 of 10", "70.0%", "5 validation errors"];
            case
        },
        validation_test("zero tools", 0, 0, 0, 0),
    ];

    for test_case in &test_cases {
        let error_msg = format!("Failed test case: {}", test_case.description);
        let stats = CliValidationStats {
            total_tools: test_case.total,
            valid_tools: test_case.valid,
            invalid_tools: test_case.invalid,
            validation_errors: test_case.errors,
        };
        verify_validation_stats(
            &stats,
            test_case.expected_all_valid,
            test_case.expected_rate,
            &test_case.expected_summary_contains,
            &error_msg,
        );
    }
}

#[test]
fn test_cli_builder_creates_tool_registry() {
    // Create CLI builder - this tests that CliBuilder::new() succeeds
    let (_registry, _builder) = create_test_registry_and_builder();

    // Builder should be created successfully without panicking
}

#[test]
fn test_cli_builder_graceful_degradation() {
    let (_registry, builder) = create_test_registry_and_builder();

    // Build CLI with warnings should not panic even with no workflows
    let cli = builder.build_cli_with_warnings(None);

    // Should successfully create CLI
    assert_eq!(cli.get_name(), "swissarmyhammer");

    // Should have basic structure
    assert!(cli.get_subcommands().any(|cmd| cmd.get_name() == "serve"));
    assert!(cli.get_subcommands().any(|cmd| cmd.get_name() == "doctor"));
}

/// Test case for argument data precomputation
#[derive(Builder)]
#[builder(pattern = "owned")]
struct ArgTestCase {
    name: &'static str,
    schema: serde_json::Value,
    #[builder(default = "false")]
    required: bool,
    validate: Box<dyn Fn(&ArgData)>,
}

/// Expected properties for argument validation
#[derive(Default, Builder, Clone)]
#[builder(pattern = "owned")]
#[builder(setter(into, strip_option))]
struct ArgProperties {
    #[builder(default)]
    arg_type: Option<ArgType>,
    #[builder(default = "false")]
    is_required: bool,
    #[builder(default)]
    possible_values: Option<Vec<String>>,
    #[builder(default)]
    help: Option<String>,
    #[builder(default)]
    default_value: Option<String>,
}

/// Generic helper to assert argument properties
fn assert_arg_properties(arg: &ArgData, expected: ArgProperties) {
    if let Some(expected_type) = expected.arg_type {
        assert!(
            std::mem::discriminant(&arg.arg_type) == std::mem::discriminant(&expected_type),
            "Expected arg type {:?}, got {:?}",
            expected_type,
            arg.arg_type
        );
    }

    if expected.is_required {
        assert!(arg.is_required, "Expected arg to be required");
    }

    assert_optional_property(
        &arg.possible_values,
        expected.possible_values.map(Some),
        "Possible values",
    );
    assert_optional_property(&arg.help, expected.help.map(Some), "Help text");
    assert_optional_property(
        &arg.default_value,
        expected.default_value.map(Some),
        "Default value",
    );
}

/// Helper function to create a validation closure for argument properties
fn validate_arg_type(expected: ArgProperties) -> Box<dyn Fn(&ArgData)> {
    Box::new(move |arg| assert_arg_properties(arg, expected.clone()))
}

/// Generic helper to conditionally add a JSON field if value is present
fn add_json_field_if_present<T: Into<serde_json::Value>>(
    obj: &mut serde_json::Value,
    key: &str,
    value: Option<T>,
) {
    if let Some(v) = value {
        obj[key] = v.into();
    }
}

/// Helper function to create an argument test case from schema parameters
fn create_arg_test_case(
    name: &'static str,
    arg_type: &'static str,
    help: Option<&'static str>,
    required: bool,
    enum_values: Option<Vec<&'static str>>,
    default_value: Option<&'static str>,
) -> ArgTestCase {
    let mut schema_obj = json!({"type": arg_type});

    // Add optional schema fields using helper
    add_json_field_if_present(&mut schema_obj, "description", help);
    if let Some(e) = enum_values.as_ref() {
        add_json_field_if_present(&mut schema_obj, "enum", Some(json!(e)));
    }
    add_json_field_if_present(&mut schema_obj, "default", default_value);

    // Build properties using derive_builder pattern with chained calls
    let mut builder = ArgPropertiesBuilder::default()
        .arg_type(arg_type_from_string(arg_type))
        .is_required(required);

    setter!(builder, help, help, |h: &str| h.to_string());
    setter!(builder, enum_values.as_ref(), possible_values, |v: &Vec<
        &str,
    >| {
        v.iter().map(|val| val.to_string()).collect::<Vec<_>>()
    });
    setter!(builder, default_value, default_value, |d: &str| d
        .to_string());

    let properties = builder.build().unwrap();

    ArgTestCaseBuilder::default()
        .name(name)
        .schema(schema_obj)
        .required(required)
        .validate(validate_arg_type(properties))
        .build()
        .unwrap()
}

/// Convert type string to ArgType
fn arg_type_from_string(type_str: &str) -> ArgType {
    match type_str {
        "boolean" => ArgType::Boolean,
        "integer" => ArgType::Integer,
        "array" => ArgType::Array,
        _ => ArgType::String,
    }
}

#[test]
fn test_precompute_arg_data_types() {
    let test_cases = vec![
        create_arg_test_case(
            "feature_flag",
            "boolean",
            Some("Enable feature"),
            false,
            None,
            None,
        ),
        create_arg_test_case("port", "integer", Some("Port number"), true, None, None),
        create_arg_test_case("files", "array", Some("List of files"), false, None, None),
        create_arg_test_case(
            "env",
            "string",
            Some("Environment"),
            true,
            Some(vec!["dev", "staging", "prod"]),
            None,
        ),
        create_arg_test_case(
            "host",
            "string",
            Some("Host"),
            false,
            None,
            Some("localhost"),
        ),
    ];

    for test_case in &test_cases {
        let arg =
            CliBuilder::precompute_arg_data(test_case.name, &test_case.schema, test_case.required);
        (test_case.validate)(&arg);
    }
}

#[test]
fn test_build_cli_basic_structure() {
    let (_registry, cli) = create_test_cli_with_defaults();

    // Verify basic structure
    assert_eq!(cli.get_name(), "swissarmyhammer");

    // Verify core subcommands exist
    // Note: plan and implement are now dynamic workflow shortcuts, not hardcoded commands
    // Note: rule command is now dynamically generated from rules_check MCP tool when tools are registered
    // This test uses an empty registry, so rule won't appear here
    let expected_commands = ["serve", "doctor", "prompt", "flow", "validate", "agent"];
    assert_commands_exist(&cli, &expected_commands);
}

#[test]
fn test_mcp_tool_categories_appear_in_help() {
    let ctx = TestContext::new(None);
    let categories = ctx.categories();
    assert_contains_all(&ctx.help, &categories, "Help text for MCP tool category");
}

#[test]
fn test_workflow_shortcuts_and_ordering() {
    let ctx = TestContext::new(None);

    // Test workflow presence in help when workflows exist
    if let Some(first_workflow) = ctx.workflows.first() {
        let workflow_names: Vec<String> =
            ctx.workflows.iter().map(|w| w.name.to_string()).collect();
        let has_workflow = workflow_names.iter().any(|name| ctx.help.contains(name));
        assert!(
            has_workflow,
            "Help text should contain workflow shortcuts when workflows are present"
        );

        // Test workflow ordering before MCP tools
        let workflow_name = first_workflow.name.to_string();
        assert_items_before_categories(&ctx, &[(&workflow_name, "Workflows")]);
    }

    // Test static commands ordering and collection sorting
    assert_items_before_categories(&ctx, &[("serve", "Static commands")]);
    assert_all_collections_sorted(&ctx);
}

#[test]
fn test_command_descriptions_are_clean() {
    let ctx = TestContext::new(None);
    // Verify no separator markers appear in command descriptions
    assert!(
        !ctx.help.contains("────────"),
        "Help text should not contain visual separators"
    );
}
