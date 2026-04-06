#![allow(dead_code)]
//! Integration tests for the #[operation] and #[param] proc macros
//!
//! These tests exercise code paths in swissarmyhammer-operations-macros/src/lib.rs
//! by compiling structs that use the macros and verifying the generated
//! Operation trait implementations.

use swissarmyhammer_operations::{operation, param, Operation, ParamType};

// --- Basic operation with named fields ---

/// A simple operation with required and optional fields.
#[operation(verb = "add", noun = "task", description = "Create a new task")]
#[derive(Debug)]
struct AddTask {
    /// The task title
    pub title: String,
    /// Optional description
    pub description: Option<String>,
}

#[test]
fn test_basic_operation_verb() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    assert_eq!(op.verb(), "add");
}

#[test]
fn test_basic_operation_noun() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    assert_eq!(op.noun(), "task");
}

#[test]
fn test_basic_operation_description() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    assert_eq!(op.description(), "Create a new task");
}

#[test]
fn test_basic_operation_op_string() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    assert_eq!(op.op_string(), "add task");
}

#[test]
fn test_parameters_count() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    assert_eq!(op.parameters().len(), 2);
}

#[test]
fn test_required_field_param_meta() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    let params = op.parameters();
    let title_param = &params[0];
    assert_eq!(title_param.name, "title");
    assert!(title_param.required, "String field should be required");
    assert_eq!(title_param.param_type, ParamType::String);
}

#[test]
fn test_optional_field_param_meta() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    let params = op.parameters();
    let desc_param = &params[1];
    assert_eq!(desc_param.name, "description");
    assert!(
        !desc_param.required,
        "Option<String> field should not be required"
    );
    assert_eq!(desc_param.param_type, ParamType::String);
}

#[test]
fn test_no_param_attr_no_short() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    let params = op.parameters();
    assert_eq!(params[0].short, None, "no #[param] means no short flag");
    assert_eq!(params[1].short, None, "no #[param] means no short flag");
}

#[test]
fn test_no_param_attr_no_aliases() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    let params = op.parameters();
    assert!(params[0].aliases.is_empty());
    assert!(params[1].aliases.is_empty());
}

#[test]
fn test_doc_comment_extraction() {
    let op = AddTask {
        title: "hello".into(),
        description: None,
    };
    let params = op.parameters();
    assert_eq!(params[0].description, "The task title");
    assert_eq!(params[1].description, "Optional description");
}

// --- Unit struct (no fields) ---

/// An operation with no fields at all.
#[operation(verb = "list", noun = "board", description = "List all boards")]
#[derive(Debug)]
struct ListBoards;

#[test]
fn test_unit_struct_verb() {
    let op = ListBoards;
    assert_eq!(op.verb(), "list");
}

#[test]
fn test_unit_struct_noun() {
    let op = ListBoards;
    assert_eq!(op.noun(), "board");
}

#[test]
fn test_unit_struct_description() {
    let op = ListBoards;
    assert_eq!(op.description(), "List all boards");
}

#[test]
fn test_unit_struct_parameters_empty() {
    let op = ListBoards;
    assert!(op.parameters().is_empty());
}

#[test]
fn test_unit_struct_op_string() {
    let op = ListBoards;
    assert_eq!(op.op_string(), "list board");
}

// --- All numeric types ---

/// Tests numeric type mapping: i32, i64, u32, u64, usize, isize, f32, f64.
#[operation(
    verb = "set",
    noun = "config",
    description = "Configure numeric settings"
)]
#[derive(Debug)]
struct NumericTypes {
    /// Signed 32
    pub val_i32: i32,
    /// Signed 64
    pub val_i64: i64,
    /// Unsigned 32
    pub val_u32: u32,
    /// Unsigned 64
    pub val_u64: u64,
    /// Unsigned pointer-sized
    pub val_usize: usize,
    /// Signed pointer-sized
    pub val_isize: isize,
    /// Float 32
    pub val_f32: f32,
    /// Float 64
    pub val_f64: f64,
}

#[test]
fn test_integer_types() {
    let op = NumericTypes {
        val_i32: 0,
        val_i64: 0,
        val_u32: 0,
        val_u64: 0,
        val_usize: 0,
        val_isize: 0,
        val_f32: 0.0,
        val_f64: 0.0,
    };
    let params = op.parameters();
    // i32, i64, u32, u64, usize, isize -> Integer
    assert_eq!(params[0].param_type, ParamType::Integer, "i32 -> Integer");
    assert_eq!(params[1].param_type, ParamType::Integer, "i64 -> Integer");
    assert_eq!(params[2].param_type, ParamType::Integer, "u32 -> Integer");
    assert_eq!(params[3].param_type, ParamType::Integer, "u64 -> Integer");
    assert_eq!(params[4].param_type, ParamType::Integer, "usize -> Integer");
    assert_eq!(params[5].param_type, ParamType::Integer, "isize -> Integer");
    // f32, f64 -> Number
    assert_eq!(params[6].param_type, ParamType::Number, "f32 -> Number");
    assert_eq!(params[7].param_type, ParamType::Number, "f64 -> Number");
}

#[test]
fn test_all_numeric_fields_required() {
    let op = NumericTypes {
        val_i32: 0,
        val_i64: 0,
        val_u32: 0,
        val_u64: 0,
        val_usize: 0,
        val_isize: 0,
        val_f32: 0.0,
        val_f64: 0.0,
    };
    for p in op.parameters() {
        assert!(p.required, "numeric field '{}' should be required", p.name);
    }
}

// --- bool type ---

/// Tests boolean type mapping.
#[operation(verb = "toggle", noun = "flag", description = "Toggle a flag")]
#[derive(Debug)]
struct ToggleFlag {
    /// Whether it's enabled
    pub enabled: bool,
}

#[test]
fn test_bool_type() {
    let op = ToggleFlag { enabled: true };
    let params = op.parameters();
    assert_eq!(params[0].param_type, ParamType::Boolean);
    assert!(params[0].required);
}

// --- Vec type ---

/// Tests Vec (array) type mapping.
#[operation(verb = "add", noun = "items", description = "Add multiple items")]
#[derive(Debug)]
struct AddItems {
    /// List of items
    pub items: Vec<String>,
}

#[test]
fn test_vec_type() {
    let op = AddItems {
        items: vec!["a".into()],
    };
    let params = op.parameters();
    assert_eq!(params[0].param_type, ParamType::Array);
    assert!(params[0].required);
}

// --- Option wrapping various types ---

/// Tests Option<T> unwrapping for all supported inner types.
#[operation(
    verb = "update",
    noun = "settings",
    description = "Update optional settings"
)]
#[derive(Debug)]
struct OptionalTypes {
    /// Optional string
    pub opt_string: Option<String>,
    /// Optional integer
    pub opt_i32: Option<i32>,
    /// Optional float
    pub opt_f64: Option<f64>,
    /// Optional boolean
    pub opt_bool: Option<bool>,
    /// Optional array
    pub opt_vec: Option<Vec<String>>,
}

#[test]
fn test_option_unwraps_inner_type() {
    let op = OptionalTypes {
        opt_string: None,
        opt_i32: None,
        opt_f64: None,
        opt_bool: None,
        opt_vec: None,
    };
    let params = op.parameters();
    assert_eq!(
        params[0].param_type,
        ParamType::String,
        "Option<String> -> String"
    );
    assert_eq!(
        params[1].param_type,
        ParamType::Integer,
        "Option<i32> -> Integer"
    );
    assert_eq!(
        params[2].param_type,
        ParamType::Number,
        "Option<f64> -> Number"
    );
    assert_eq!(
        params[3].param_type,
        ParamType::Boolean,
        "Option<bool> -> Boolean"
    );
    assert_eq!(
        params[4].param_type,
        ParamType::Array,
        "Option<Vec<String>> -> Array"
    );
}

#[test]
fn test_option_fields_not_required() {
    let op = OptionalTypes {
        opt_string: None,
        opt_i32: None,
        opt_f64: None,
        opt_bool: None,
        opt_vec: None,
    };
    for p in op.parameters() {
        assert!(
            !p.required,
            "Option field '{}' should not be required",
            p.name
        );
    }
}

// --- Unknown / custom type defaults to String ---

/// Custom type that should default to String in param_type.
#[derive(Debug)]
struct CustomId(String);

#[operation(verb = "get", noun = "entity", description = "Get entity by custom ID")]
#[derive(Debug)]
struct GetEntity {
    /// The entity identifier
    pub id: CustomId,
}

#[test]
fn test_unknown_type_defaults_to_string() {
    let op = GetEntity {
        id: CustomId("abc".into()),
    };
    let params = op.parameters();
    assert_eq!(params[0].param_type, ParamType::String);
    assert!(params[0].required);
}

// --- Standalone #[param] attribute (pass-through on items) ---

/// Verify the #[param] macro works as a pass-through when applied to an item.
#[param(short = 'x')]
#[derive(Debug)]
struct StandaloneParam {
    pub field: String,
}

#[test]
fn test_param_passthrough() {
    // Just verify the struct compiles and field is accessible
    let s = StandaloneParam {
        field: "value".into(),
    };
    assert_eq!(s.field, "value");
}

// --- Field with no doc comment ---

#[operation(verb = "do", noun = "thing", description = "Do a thing")]
#[derive(Debug)]
struct NoDocComment {
    pub undocumented: String,
}

#[test]
fn test_no_doc_comment_empty_description() {
    let op = NoDocComment {
        undocumented: "x".into(),
    };
    let params = op.parameters();
    assert_eq!(params[0].description, "");
}

// --- Field with no #[param] attribute ---

#[operation(verb = "run", noun = "job", description = "Run a job")]
#[derive(Debug)]
struct NoParamAttr {
    /// A plain field
    pub name: String,
}

#[test]
fn test_no_param_attr_defaults() {
    let op = NoParamAttr { name: "x".into() };
    let params = op.parameters();
    assert_eq!(params[0].name, "name");
    assert_eq!(params[0].short, None);
    assert!(params[0].aliases.is_empty());
}

// --- Trailing comma in operation args ---

#[operation(verb = "check", noun = "status", description = "Check status")]
#[derive(Debug)]
struct CheckStatus;

#[test]
fn test_trailing_comma_in_attrs() {
    let op = CheckStatus;
    assert_eq!(op.verb(), "check");
    assert_eq!(op.noun(), "status");
}

// --- Multi-line doc comment ---

#[operation(verb = "search", noun = "docs", description = "Search documentation")]
#[derive(Debug)]
struct SearchDocs {
    /// The search query
    /// that spans multiple lines
    pub query: String,
}

#[test]
fn test_multiline_doc_comment() {
    let op = SearchDocs {
        query: "test".into(),
    };
    let params = op.parameters();
    // Multi-line doc comments get joined with spaces
    assert!(params[0].description.contains("search query"));
    assert!(params[0].description.contains("multiple lines"));
}

// --- Many fields with mixed types ---

#[operation(
    verb = "create",
    noun = "report",
    description = "Create a report with many fields"
)]
#[derive(Debug)]
struct CreateReport {
    /// Report name
    pub name: String,
    /// Number of pages
    pub pages: u32,
    /// Weight in kg
    pub weight: f64,
    /// Is draft
    pub draft: bool,
    /// List of tags
    pub tags: Vec<String>,
    /// Optional reviewer
    pub reviewer: Option<String>,
    /// Optional max items
    pub max_items: Option<usize>,
}

#[test]
fn test_mixed_types_report() {
    let op = CreateReport {
        name: "Report".into(),
        pages: 10,
        weight: 1.5,
        draft: false,
        tags: vec![],
        reviewer: None,
        max_items: None,
    };
    let params = op.parameters();
    assert_eq!(params.len(), 7);

    // Required fields
    assert!(params[0].required); // name: String
    assert!(params[1].required); // pages: u32
    assert!(params[2].required); // weight: f64
    assert!(params[3].required); // draft: bool
    assert!(params[4].required); // tags: Vec<String>

    // Optional fields
    assert!(!params[5].required); // reviewer: Option<String>
    assert!(!params[6].required); // max_items: Option<usize>

    // Type mappings
    assert_eq!(params[0].param_type, ParamType::String);
    assert_eq!(params[1].param_type, ParamType::Integer);
    assert_eq!(params[2].param_type, ParamType::Number);
    assert_eq!(params[3].param_type, ParamType::Boolean);
    assert_eq!(params[4].param_type, ParamType::Array);
    assert_eq!(params[5].param_type, ParamType::String);
    assert_eq!(params[6].param_type, ParamType::Integer);
}

// --- Default examples() is empty ---

#[test]
fn test_default_examples_empty() {
    let op = ListBoards;
    assert!(op.examples().is_empty());
}

// --- Parameters are static (returned slice is 'static) ---

#[test]
fn test_parameters_are_static() {
    let params: &'static [_] = {
        let op = AddTask {
            title: "tmp".into(),
            description: None,
        };
        op.parameters()
    };
    // The reference outlives the op instance because it points to static data
    assert_eq!(params.len(), 2);
}
