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
struct TestContext {
    help: String,
    registry: Arc<tokio::sync::RwLock<ToolRegistry>>,
}

impl TestContext {
    /// Create a new test context
    fn new() -> Self {
        let components = TestComponents::new();
        let (registry, cli) = components.into_registry_and_cli();
        let help = get_help_text(&cli);

        Self { help, registry }
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
    /// Create new test components
    fn new() -> Self {
        let registry = Arc::new(tokio::sync::RwLock::new(ToolRegistry::new()));
        let builder = CliBuilder::new(registry.clone());
        let cli = builder.build_cli();
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
    TestComponents::new().into_registry_and_builder()
}

/// Helper function to create a CLI with default configuration for testing
fn create_test_cli_with_defaults() -> (Arc<tokio::sync::RwLock<ToolRegistry>>, Command) {
    TestComponents::new().into_registry_and_cli()
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
    item.is_some_and(|s| text.contains(s))
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
    let Some(item_str) = item else {
        return;
    };

    if !contains_if_present(&ctx.help, Some(item_str)) {
        return;
    }

    let Some(first_category) = ctx.categories().into_iter().next() else {
        return;
    };

    assert_help_position_before(&ctx.help, item_str, &first_category, context_msg);
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

/// Helper to build expected summary items based on validation state
fn build_summary_expectations(total: usize, invalid: usize, errors: usize) -> Vec<&'static str> {
    if total == 0 {
        return vec![];
    }

    if invalid == 0 && errors == 0 {
        return vec!["✓", "All"];
    }

    vec!["⚠"]
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

    let summary_contains = build_summary_expectations(total, invalid, errors);

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
            case.expected_summary_contains = vec!["✓", "All 10 CLI tools are valid"];
            case
        },
        {
            let mut case = validation_test("some invalid", 10, 7, 3, 5);
            case.expected_summary_contains = vec!["⚠", "7 of 10", "70.0%", "5 validation errors"];
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

    // Build CLI with warnings should not panic
    let cli = builder.build_cli_with_warnings();

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

/// Helper to build schema JSON from parameters
fn build_schema_json(
    arg_type: &'static str,
    help: Option<&'static str>,
    enum_values: Option<&Vec<&'static str>>,
    default_value: Option<&'static str>,
) -> serde_json::Value {
    let mut schema_obj = json!({"type": arg_type});

    add_json_field_if_present(&mut schema_obj, "description", help);
    if let Some(e) = enum_values {
        add_json_field_if_present(&mut schema_obj, "enum", Some(json!(e)));
    }
    add_json_field_if_present(&mut schema_obj, "default", default_value);

    schema_obj
}

/// Helper to build argument properties from parameters
fn build_arg_properties(
    arg_type: &'static str,
    required: bool,
    help: Option<&'static str>,
    enum_values: Option<&Vec<&'static str>>,
    default_value: Option<&'static str>,
) -> ArgProperties {
    let mut builder = ArgPropertiesBuilder::default()
        .arg_type(arg_type_from_string(arg_type))
        .is_required(required);

    setter!(builder, help, help, |h: &str| h.to_string());
    setter!(builder, enum_values, possible_values, |v: &Vec<&str>| {
        v.iter().map(|val| val.to_string()).collect::<Vec<_>>()
    });
    setter!(builder, default_value, default_value, |d: &str| d
        .to_string());

    builder.build().unwrap()
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
    let schema_obj = build_schema_json(arg_type, help, enum_values.as_ref(), default_value);
    let properties = build_arg_properties(
        arg_type,
        required,
        help,
        enum_values.as_ref(),
        default_value,
    );

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
    // Note: rule command is now dynamically generated from rules_check MCP tool when tools are registered
    // This test uses an empty registry, so rule won't appear here
    let expected_commands = ["serve", "doctor", "prompt", "validate", "model"];
    assert_commands_exist(&cli, &expected_commands);
}

#[test]
fn test_mcp_tool_categories_appear_in_help() {
    let ctx = TestContext::new();
    let categories = ctx.categories();
    assert_contains_all(&ctx.help, &categories, "Help text for MCP tool category");
}

#[test]
fn test_static_commands_ordering() {
    let ctx = TestContext::new();

    // Test static commands appear before MCP tool categories
    assert_items_before_categories(&ctx, &[("serve", "Static commands")]);

    // Test MCP tool categories are sorted
    let categories = ctx.categories();
    assert_sorted_if_multiple(
        &categories,
        "MCP tool categories should be sorted alphabetically",
    );
}

#[test]
fn test_command_descriptions_are_clean() {
    let ctx = TestContext::new();
    // Verify no separator markers appear in command descriptions
    assert!(
        !ctx.help.contains("────────"),
        "Help text should not contain visual separators"
    );
}

// --- Tests for schema_has_type ---

#[test]
fn test_schema_has_type_string() {
    let schema = json!({"type": "string"});
    assert!(schema_has_type(&schema, "string"));
    assert!(!schema_has_type(&schema, "boolean"));
}

#[test]
fn test_schema_has_type_array_type() {
    let schema = json!({"type": ["string", "null"]});
    assert!(schema_has_type(&schema, "string"));
    assert!(schema_has_type(&schema, "null"));
    assert!(!schema_has_type(&schema, "boolean"));
}

#[test]
fn test_schema_has_type_no_type_field() {
    let schema = json!({"description": "no type"});
    assert!(!schema_has_type(&schema, "string"));
}

#[test]
fn test_schema_has_type_non_string_type() {
    let schema = json!({"type": 42});
    assert!(!schema_has_type(&schema, "string"));
}

// --- Tests for SchemaParser ---

#[test]
fn test_schema_parser_parse_string() {
    let schema = json!({"description": "A field", "default": "hello"});
    assert_eq!(
        SchemaParser::parse_string(&schema, "description"),
        Some("A field".to_string())
    );
    assert_eq!(
        SchemaParser::parse_string(&schema, "default"),
        Some("hello".to_string())
    );
    assert_eq!(SchemaParser::parse_string(&schema, "missing"), None);
}

#[test]
fn test_schema_parser_parse_enum() {
    let schema = json!({"enum": ["a", "b", "c"]});
    let result = SchemaParser::parse_enum(&schema);
    assert_eq!(
        result,
        Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
    );
}

#[test]
fn test_schema_parser_parse_enum_none() {
    let schema = json!({"type": "string"});
    assert_eq!(SchemaParser::parse_enum(&schema), None);
}

#[test]
fn test_schema_parser_parse_description() {
    let schema = json!({"description": "test desc"});
    assert_eq!(
        SchemaParser::parse_description(&schema),
        Some("test desc".to_string())
    );
}

#[test]
fn test_schema_parser_parse_default() {
    let schema = json!({"default": "mydefault"});
    assert_eq!(
        SchemaParser::parse_default(&schema),
        Some("mydefault".to_string())
    );
}

#[test]
fn test_schema_parser_parse_type_boolean() {
    let schema = json!({"type": "boolean"});
    assert!(matches!(
        SchemaParser::parse_type(&schema),
        ArgType::Boolean
    ));
}

#[test]
fn test_schema_parser_parse_type_integer() {
    let schema = json!({"type": "integer"});
    assert!(matches!(
        SchemaParser::parse_type(&schema),
        ArgType::Integer
    ));
}

#[test]
fn test_schema_parser_parse_type_number() {
    let schema = json!({"type": "number"});
    assert!(matches!(SchemaParser::parse_type(&schema), ArgType::Float));
}

#[test]
fn test_schema_parser_parse_type_array() {
    let schema = json!({"type": "array"});
    assert!(matches!(SchemaParser::parse_type(&schema), ArgType::Array));
}

#[test]
fn test_schema_parser_parse_type_string() {
    let schema = json!({"type": "string"});
    assert!(matches!(SchemaParser::parse_type(&schema), ArgType::String));
}

#[test]
fn test_schema_parser_parse_type_nullable_boolean() {
    let schema = json!({"type": ["boolean", "null"]});
    assert!(matches!(
        SchemaParser::parse_type(&schema),
        ArgType::NullableBoolean
    ));
}

#[test]
fn test_schema_parser_is_nullable_boolean() {
    assert!(SchemaParser::is_nullable_boolean(
        &json!({"type": ["boolean", "null"]})
    ));
    assert!(!SchemaParser::is_nullable_boolean(
        &json!({"type": "boolean"})
    ));
    assert!(!SchemaParser::is_nullable_boolean(
        &json!({"type": ["string", "null"]})
    ));
}

#[test]
fn test_schema_parser_get_primary_type_string() {
    assert_eq!(
        SchemaParser::get_primary_type(&json!({"type": "string"})),
        Some("string")
    );
}

#[test]
fn test_schema_parser_get_primary_type_array_filters_null() {
    assert_eq!(
        SchemaParser::get_primary_type(&json!({"type": ["integer", "null"]})),
        Some("integer")
    );
}

#[test]
fn test_schema_parser_get_primary_type_none() {
    assert_eq!(SchemaParser::get_primary_type(&json!({"foo": "bar"})), None);
}

#[test]
fn test_schema_parser_parse_arg_data() {
    let schema = json!({
        "type": "string",
        "description": "A test field",
        "default": "hello",
        "enum": ["a", "b"]
    });
    let arg = SchemaParser::parse_arg_data("test_field", &schema, true);
    assert_eq!(arg.name, "test_field");
    assert_eq!(arg.help, Some("A test field".to_string()));
    assert_eq!(arg.default_value, Some("hello".to_string()));
    assert!(arg.is_required);
    assert_eq!(
        arg.possible_values,
        Some(vec!["a".to_string(), "b".to_string()])
    );
}

// --- Tests for ArgBuilder ---

#[test]
fn test_arg_builder_string_optional() {
    let arg_data = ArgData {
        name: "host".to_string(),
        help: Some("Host name".to_string()),
        is_required: false,
        arg_type: ArgType::String,
        default_value: Some("localhost".to_string()),
        possible_values: None,
    };
    let arg = ArgBuilder::new(&arg_data).build();
    assert_eq!(arg.get_id().as_str(), "host");
}

#[test]
fn test_arg_builder_boolean() {
    let arg_data = ArgData {
        name: "verbose".to_string(),
        help: Some("Verbose output".to_string()),
        is_required: false,
        arg_type: ArgType::Boolean,
        default_value: None,
        possible_values: None,
    };
    let arg = ArgBuilder::new(&arg_data).build();
    assert_eq!(arg.get_id().as_str(), "verbose");
}

#[test]
fn test_arg_builder_nullable_boolean() {
    let arg_data = ArgData {
        name: "flag".to_string(),
        help: None,
        is_required: false,
        arg_type: ArgType::NullableBoolean,
        default_value: None,
        possible_values: None,
    };
    let arg = ArgBuilder::new(&arg_data).build();
    assert_eq!(arg.get_id().as_str(), "flag");
}

#[test]
fn test_arg_builder_integer_required() {
    let arg_data = ArgData {
        name: "port".to_string(),
        help: Some("Port number".to_string()),
        is_required: true,
        arg_type: ArgType::Integer,
        default_value: None,
        possible_values: None,
    };
    let arg = ArgBuilder::new(&arg_data).build();
    assert_eq!(arg.get_id().as_str(), "port");
    assert!(arg.is_required_set());
}

#[test]
fn test_arg_builder_float_optional() {
    let arg_data = ArgData {
        name: "threshold".to_string(),
        help: None,
        is_required: false,
        arg_type: ArgType::Float,
        default_value: None,
        possible_values: None,
    };
    let arg = ArgBuilder::new(&arg_data).build();
    assert_eq!(arg.get_id().as_str(), "threshold");
}

#[test]
fn test_arg_builder_array() {
    let arg_data = ArgData {
        name: "items".to_string(),
        help: Some("Items".to_string()),
        is_required: false,
        arg_type: ArgType::Array,
        default_value: None,
        possible_values: None,
    };
    let arg = ArgBuilder::new(&arg_data).build();
    assert_eq!(arg.get_id().as_str(), "items");
}

#[test]
fn test_arg_builder_with_possible_values() {
    let arg_data = ArgData {
        name: "color".to_string(),
        help: Some("Color choice".to_string()),
        is_required: false,
        arg_type: ArgType::String,
        default_value: None,
        possible_values: Some(vec!["red".to_string(), "blue".to_string()]),
    };
    let arg = ArgBuilder::new(&arg_data).build();
    assert_eq!(arg.get_id().as_str(), "color");
}

// --- Tests for CliValidationStats ---

#[test]
fn test_cli_validation_stats_new() {
    let stats = CliValidationStats::new();
    assert_eq!(stats.total_tools, 0);
    assert_eq!(stats.valid_tools, 0);
    assert_eq!(stats.invalid_tools, 0);
    assert_eq!(stats.validation_errors, 0);
}

#[test]
fn test_cli_validation_stats_has_no_tools() {
    let stats = CliValidationStats::new();
    assert!(stats.has_no_tools());

    let stats = CliValidationStats {
        total_tools: 1,
        ..Default::default()
    };
    assert!(!stats.has_no_tools());
}

#[test]
fn test_cli_validation_stats_is_all_valid() {
    let stats = CliValidationStats {
        total_tools: 5,
        valid_tools: 5,
        invalid_tools: 0,
        validation_errors: 0,
    };
    assert!(stats.is_all_valid());

    let stats = CliValidationStats {
        total_tools: 5,
        valid_tools: 4,
        invalid_tools: 1,
        validation_errors: 2,
    };
    assert!(!stats.is_all_valid());
}

#[test]
fn test_cli_validation_stats_success_rate_no_tools() {
    let stats = CliValidationStats::new();
    assert_eq!(stats.success_rate(), 100.0);
}

#[test]
fn test_cli_validation_stats_success_rate_partial() {
    let stats = CliValidationStats {
        total_tools: 4,
        valid_tools: 3,
        invalid_tools: 1,
        validation_errors: 1,
    };
    assert_eq!(stats.success_rate(), 75.0);
}

#[test]
fn test_cli_validation_stats_summary_all_valid() {
    let stats = CliValidationStats {
        total_tools: 5,
        valid_tools: 5,
        invalid_tools: 0,
        validation_errors: 0,
    };
    let summary = stats.summary();
    assert!(summary.contains("All 5 CLI tools are valid"));
}

#[test]
fn test_cli_validation_stats_summary_has_warnings() {
    let stats = CliValidationStats {
        total_tools: 10,
        valid_tools: 8,
        invalid_tools: 2,
        validation_errors: 3,
    };
    let summary = stats.summary();
    assert!(summary.contains("8 of 10"));
    assert!(summary.contains("80.0%"));
    assert!(summary.contains("3 validation errors"));
}

// --- Tests for get_default_config ---

#[test]
fn test_get_default_config_fallback() {
    // Use unique env var names that won't exist
    let result = get_default_config(
        "SAH_TEST_UNLIKELY_VAR_12345",
        "SWISSARMYHAMMER_TEST_UNLIKELY_VAR_12345",
        "default_val",
    );
    assert_eq!(result, "default_val");
}

#[test]
fn test_get_default_http_port() {
    let port = get_default_http_port();
    // Should be either from env or default 8000
    assert!(!port.is_empty());
}

#[test]
fn test_get_default_http_host() {
    let host = get_default_http_host();
    assert!(!host.is_empty());
}

// --- Tests for ArgSpec builder ---

#[test]
fn test_arg_spec_new() {
    let spec = ArgSpec::new("test", "Help text");
    assert_eq!(spec.name, "test");
    assert_eq!(spec.help, "Help text");
    assert!(spec.long.is_none());
    assert!(spec.short.is_none());
    assert!(!spec.required);
    assert!(!spec.hide);
}

#[test]
fn test_arg_spec_builder_chain() {
    let spec = ArgSpec::new("port", "Port number")
        .long("port")
        .short('p')
        .required(true)
        .value_name("PORT")
        .default_value("8080".to_string())
        .value_parser(ArgSpecValueParser::U16)
        .action(ArgSpecAction::Set);

    assert_eq!(spec.name, "port");
    assert_eq!(spec.long, Some("port"));
    assert_eq!(spec.short, Some('p'));
    assert!(spec.required);
    assert_eq!(spec.value_name, Some("PORT"));
    assert_eq!(spec.default_value, Some("8080".to_string()));
}

#[test]
fn test_arg_spec_build_set_true() {
    let spec = ArgSpec::new("verbose", "Verbose output")
        .long("verbose")
        .action(ArgSpecAction::SetTrue);
    let arg = spec.build();
    assert_eq!(arg.get_id().as_str(), "verbose");
}

#[test]
fn test_arg_spec_build_append() {
    let spec = ArgSpec::new("files", "Input files")
        .long("file")
        .action(ArgSpecAction::Append);
    let arg = spec.build();
    assert_eq!(arg.get_id().as_str(), "files");
}

#[test]
fn test_arg_spec_build_with_value_parser_strings() {
    let spec = ArgSpec::new("format", "Format")
        .long("format")
        .value_parser(ArgSpecValueParser::Strings(vec!["json", "yaml", "table"]));
    let arg = spec.build();
    assert_eq!(arg.get_id().as_str(), "format");
}

#[test]
fn test_arg_spec_build_with_value_parser_u64() {
    let spec = ArgSpec::new("size", "Size")
        .long("size")
        .value_parser(ArgSpecValueParser::U64);
    let arg = spec.build();
    assert_eq!(arg.get_id().as_str(), "size");
}

#[test]
fn test_arg_spec_build_with_value_parser_usize() {
    let spec = ArgSpec::new("count", "Count")
        .long("count")
        .value_parser(ArgSpecValueParser::Usize);
    let arg = spec.build();
    assert_eq!(arg.get_id().as_str(), "count");
}

// --- Tests for SubcommandSpec ---

#[test]
fn test_subcommand_spec_new() {
    let spec = SubcommandSpec::new("test", "About test");
    assert_eq!(spec.name, "test");
    assert_eq!(spec.about, "About test");
    assert!(spec.long_about.is_none());
    assert!(spec.args.is_empty());
}

#[test]
fn test_subcommand_spec_build() {
    let spec = SubcommandSpec::new("sub", "About sub")
        .long_about("Long about sub")
        .args(vec![ArgSpec::new("arg1", "Arg 1").long("arg1")]);
    let cmd = spec.build();
    assert_eq!(cmd.get_name(), "sub");
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "arg1"));
}

#[test]
fn test_subcommand_spec_clone() {
    let spec = SubcommandSpec::new("test", "About")
        .long_about("Long about")
        .args(vec![ArgSpec::new("a", "help")]);
    let cloned = spec.clone();
    assert_eq!(cloned.name, "test");
    assert_eq!(cloned.about, "About");
    assert_eq!(cloned.long_about, Some("Long about"));
    assert_eq!(cloned.args.len(), 1);
}

// --- Tests for CliBuilder static command builders ---

#[test]
fn test_build_serve_command() {
    let cmd = CliBuilder::build_serve_command();
    assert_eq!(cmd.get_name(), "serve");
    // Should have http subcommand
    assert!(cmd.get_subcommands().any(|sub| sub.get_name() == "http"));
}

#[test]
fn test_build_init_command() {
    let cmd = CliBuilder::build_init_command();
    assert_eq!(cmd.get_name(), "init");
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "target"));
}

#[test]
fn test_build_deinit_command() {
    let cmd = CliBuilder::build_deinit_command();
    assert_eq!(cmd.get_name(), "deinit");
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "target"));
    assert!(cmd
        .get_arguments()
        .any(|a| a.get_id().as_str() == "remove-directory"));
}

#[test]
fn test_build_doctor_command() {
    let cmd = CliBuilder::build_doctor_command();
    assert_eq!(cmd.get_name(), "doctor");
}

#[test]
fn test_build_validate_command() {
    let cmd = CliBuilder::build_validate_command();
    assert_eq!(cmd.get_name(), "validate");
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "quiet"));
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "format"));
}

#[test]
fn test_build_prompt_command() {
    let cmd = CliBuilder::build_prompt_command();
    assert_eq!(cmd.get_name(), "prompt");
    let subcmd_names: Vec<&str> = cmd.get_subcommands().map(|s| s.get_name()).collect();
    assert!(subcmd_names.contains(&"list"));
    assert!(subcmd_names.contains(&"test"));
    assert!(subcmd_names.contains(&"render"));
    assert!(subcmd_names.contains(&"new"));
    assert!(subcmd_names.contains(&"show"));
    assert!(subcmd_names.contains(&"edit"));
    assert!(subcmd_names.contains(&"validate"));
}

#[test]
fn test_build_model_command() {
    let cmd = CliBuilder::build_model_command();
    assert_eq!(cmd.get_name(), "model");
    let subcmd_names: Vec<&str> = cmd.get_subcommands().map(|s| s.get_name()).collect();
    assert!(subcmd_names.contains(&"show"));
    assert!(subcmd_names.contains(&"list"));
    assert!(subcmd_names.contains(&"use"));
}

#[test]
fn test_build_agent_command() {
    let cmd = CliBuilder::build_agent_command();
    assert_eq!(cmd.get_name(), "agent");
    let subcmd_names: Vec<&str> = cmd.get_subcommands().map(|s| s.get_name()).collect();
    assert!(subcmd_names.contains(&"acp"));
}

// --- Tests for CliBuilder methods ---

#[test]
fn test_cli_builder_build_args_from_specs() {
    let specs = vec![
        ArgSpec::new("arg1", "Help 1").long("arg1"),
        ArgSpec::new("arg2", "Help 2").long("arg2"),
    ];
    let args = CliBuilder::build_args_from_specs(&specs);
    assert_eq!(args.len(), 2);
}

#[test]
fn test_cli_builder_build_subcommands_from_specs() {
    let specs = vec![
        SubcommandSpec::new("sub1", "About 1"),
        SubcommandSpec::new("sub2", "About 2"),
    ];
    let cmds = CliBuilder::build_subcommands_from_specs(&specs);
    assert_eq!(cmds.len(), 2);
}

#[test]
fn test_cli_builder_build_command_with_docs() {
    let cmd = CliBuilder::build_command_with_docs(CommandConfig {
        name: "test",
        about: "Short about",
        long_about: "Long about text",
    });
    assert_eq!(cmd.get_name(), "test");
}

#[test]
fn test_cli_builder_build_command_with_args() {
    let cmd = CliBuilder::build_command_with_args(
        CommandConfig {
            name: "test",
            about: "About",
            long_about: "Long about",
        },
        vec![
            ArgSpec::new("a", "Help A").long("aaa").build(),
            ArgSpec::new("b", "Help B").long("bbb").build(),
        ],
    );
    assert_eq!(cmd.get_name(), "test");
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "a"));
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "b"));
}

#[test]
fn test_cli_builder_build_command_with_subcommands() {
    let cmd = CliBuilder::build_command_with_subcommands(
        CommandConfig {
            name: "parent",
            about: "Parent",
            long_about: "Parent long",
        },
        vec![
            Command::new("child1").about("Child 1"),
            Command::new("child2").about("Child 2"),
        ],
    );
    assert_eq!(cmd.get_name(), "parent");
    let subcmds: Vec<&str> = cmd.get_subcommands().map(|s| s.get_name()).collect();
    assert!(subcmds.contains(&"child1"));
    assert!(subcmds.contains(&"child2"));
}

#[test]
fn test_cli_builder_create_flag_arg_with_short() {
    let arg = CliBuilder::create_flag_arg("verbose", "verbose", Some('v'), "Be verbose");
    assert_eq!(arg.get_id().as_str(), "verbose");
    assert_eq!(arg.get_short(), Some('v'));
}

#[test]
fn test_cli_builder_create_flag_arg_without_short() {
    let arg = CliBuilder::create_flag_arg("debug", "debug", None, "Debug mode");
    assert_eq!(arg.get_id().as_str(), "debug");
    assert_eq!(arg.get_short(), None);
}

// --- Tests for precompute_args ---

#[test]
fn test_precompute_args_no_properties() {
    let schema = json!({});
    let args = CliBuilder::precompute_args(&schema);
    assert!(args.is_empty());
}

#[test]
fn test_precompute_args_with_properties() {
    let schema = json!({
        "properties": {
            "name": {"type": "string", "description": "Name"},
            "count": {"type": "integer", "description": "Count"}
        },
        "required": ["name"]
    });
    let args = CliBuilder::precompute_args(&schema);
    assert_eq!(args.len(), 2);
    let name_arg = args.iter().find(|a| a.name == "name").unwrap();
    assert!(name_arg.is_required);
    let count_arg = args.iter().find(|a| a.name == "count").unwrap();
    assert!(!count_arg.is_required);
}

#[test]
fn test_extract_required_fields_empty() {
    let schema = json!({"properties": {}});
    let required = CliBuilder::extract_required_fields(&schema);
    assert!(required.is_empty());
}

#[test]
fn test_extract_required_fields_present() {
    let schema = json!({"required": ["name", "id"]});
    let required = CliBuilder::extract_required_fields(&schema);
    assert!(required.contains("name"));
    assert!(required.contains("id"));
    assert_eq!(required.len(), 2);
}

// --- Tests for CliBuilder validation methods ---

#[test]
fn test_cli_builder_validate_all_tools_empty_registry() {
    let (_registry, builder) = create_test_registry_and_builder();
    let errors = builder.validate_all_tools();
    assert!(errors.is_empty());
}

#[test]
fn test_cli_builder_get_validation_warnings_empty_registry() {
    let (_registry, builder) = create_test_registry_and_builder();
    let warnings = builder.get_validation_warnings();
    assert!(warnings.is_empty());
}

#[test]
fn test_cli_builder_get_validation_stats_empty_registry() {
    let (_registry, builder) = create_test_registry_and_builder();
    let stats = builder.get_validation_stats();
    assert_eq!(stats.total_tools, 0);
    assert!(stats.is_all_valid());
    assert_eq!(stats.success_rate(), 100.0);
}

// --- Tests for build_command_from_data ---

#[test]
fn test_build_command_from_data_with_subcommands() {
    let data = CommandData {
        name: "parent".to_string(),
        about: Some("Parent cmd".to_string()),
        long_about: Some("Detailed parent cmd".to_string()),
        args: vec![ArgData {
            name: "flag".to_string(),
            help: Some("A flag".to_string()),
            is_required: false,
            arg_type: ArgType::Boolean,
            default_value: None,
            possible_values: None,
        }],
        subcommands: vec![CommandData {
            name: "child".to_string(),
            about: Some("Child cmd".to_string()),
            long_about: None,
            args: vec![],
            subcommands: vec![],
        }],
    };
    let cmd = CliBuilder::build_command_from_data(&data);
    assert_eq!(cmd.get_name(), "parent");
    assert!(cmd.get_subcommands().any(|s| s.get_name() == "child"));
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "flag"));
}

#[test]
fn test_build_tool_subcommand_from_data() {
    let data = CommandData {
        name: "tool_name".to_string(),
        about: Some("Tool about".to_string()),
        long_about: Some("Long tool about".to_string()),
        args: vec![ArgData {
            name: "input".to_string(),
            help: Some("Input".to_string()),
            is_required: true,
            arg_type: ArgType::String,
            default_value: None,
            possible_values: None,
        }],
        subcommands: vec![],
    };
    let cmd = CliBuilder::build_tool_subcommand_from_data("my_tool", &data);
    assert_eq!(cmd.get_name(), "my_tool");
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "input"));
}

#[test]
fn test_build_command_base() {
    let data = CommandData {
        name: "base".to_string(),
        about: Some("About base".to_string()),
        long_about: Some("Long about base".to_string()),
        args: vec![],
        subcommands: vec![],
    };
    let cmd = CliBuilder::build_command_base(&data);
    assert_eq!(cmd.get_name(), "base");
}

#[test]
fn test_build_command_base_no_about() {
    let data = CommandData {
        name: "minimal".to_string(),
        about: None,
        long_about: None,
        args: vec![],
        subcommands: vec![],
    };
    let cmd = CliBuilder::build_command_base(&data);
    assert_eq!(cmd.get_name(), "minimal");
}

// --- Tests for intern_string deduplication ---

#[test]
fn test_intern_string_deduplication() {
    let s1 = intern_string("unique_test_dedup_string".to_string());
    let s2 = intern_string("unique_test_dedup_string".to_string());
    // Same content should return same pointer
    assert_eq!(s1 as *const str, s2 as *const str);
    assert_eq!(s1, "unique_test_dedup_string");
}

// --- Tests for CommandData clone ---

#[test]
fn test_command_data_clone() {
    let data = CommandData {
        name: "test".to_string(),
        about: Some("About".to_string()),
        long_about: None,
        args: vec![ArgData {
            name: "a".to_string(),
            help: None,
            is_required: false,
            arg_type: ArgType::String,
            default_value: None,
            possible_values: None,
        }],
        subcommands: vec![],
    };
    let cloned = data.clone();
    assert_eq!(cloned.name, "test");
    assert_eq!(cloned.args.len(), 1);
}

// --- Tests for RegistryIterType ---

#[test]
fn test_iter_registry_all_tools_empty() {
    let registry = ToolRegistry::new();
    let mut count = 0;
    CliBuilder::iter_registry(&registry, RegistryIterType::AllTools, |_| {
        count += 1;
    });
    assert_eq!(count, 0);
}

// --- Tests for HTTP subcommand args ---

#[test]
fn test_build_serve_http_subcommand() {
    let cmd = CliBuilder::build_serve_http_subcommand();
    assert_eq!(cmd.get_name(), "http");
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "port"));
    assert!(cmd.get_arguments().any(|a| a.get_id().as_str() == "host"));
}
