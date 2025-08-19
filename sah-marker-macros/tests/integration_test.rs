//! Integration tests for the cli_exclude attribute macro
//!
//! These tests verify that the macro works correctly when used from outside
//! the defining crate.

use sah_marker_macros::cli_exclude;

/// Test that the cli_exclude attribute can be applied and compiles successfully
#[test]
fn test_cli_exclude_attribute_compiles() {
    #[cli_exclude]
    #[derive(Default, Debug, PartialEq)]
    struct TestTool {
        name: String,
    }

    let tool = TestTool::default();
    assert_eq!(tool.name, "");
}

/// Test that cli_exclude works with multiple attributes and derives
#[test]
fn test_cli_exclude_with_multiple_attributes() {
    #[cli_exclude]
    #[derive(Default, Clone, Debug)]
    #[allow(dead_code)]
    struct MultiAttributeTool {
        id: u32,
        active: bool,
    }

    let tool = MultiAttributeTool::default();
    let cloned = tool.clone();
    assert_eq!(tool.id, cloned.id);
    assert_eq!(tool.active, cloned.active);
}

/// Test that cli_exclude can be applied to different item types
#[test]
fn test_cli_exclude_on_different_items() {
    // Test on struct
    #[cli_exclude]
    struct StructTool;

    // Test on struct with fields
    #[cli_exclude]
    #[derive(Default)]
    struct FieldTool {
        value: i32,
    }

    let _struct_tool = StructTool;
    let field_tool = FieldTool::default();
    assert_eq!(field_tool.value, 0);
}

/// Test that the attribute doesn't interfere with trait implementations
#[test]
fn test_cli_exclude_with_trait_implementations() {
    trait TestTrait {
        fn get_name(&self) -> &str;
    }

    #[cli_exclude]
    #[derive(Default)]
    struct TraitTool {
        name: String,
    }

    impl TestTrait for TraitTool {
        fn get_name(&self) -> &str {
            &self.name
        }
    }

    let tool = TraitTool::default();
    assert_eq!(tool.get_name(), "");
}

/// Test that multiple cli_exclude attributes can be used in the same module
#[test]
fn test_multiple_cli_exclude_in_module() {
    #[cli_exclude]
    #[derive(Default)]
    struct Tool1;

    #[cli_exclude]
    #[derive(Default)]
    struct Tool2;

    let _tool1 = Tool1::default();
    let _tool2 = Tool2::default();
}

/// Test that cli_exclude works with generic structs
#[test]
fn test_cli_exclude_with_generics() {
    #[cli_exclude]
    #[derive(Default)]
    struct GenericTool<T> {
        value: Option<T>,
    }

    let tool: GenericTool<i32> = GenericTool::default();
    assert!(tool.value.is_none());

    let string_tool: GenericTool<String> = GenericTool::default();
    assert!(string_tool.value.is_none());
}
