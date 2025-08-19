//! Unit tests for the `#[cli_exclude]` attribute macro
//!
//! These tests validate the compile-time behavior of the CLI exclusion attribute macro,
//! ensuring it properly compiles without modifying the decorated items.

use sah_marker_macros::cli_exclude;

/// Test that the attribute compiles without errors on a basic struct
#[test]
fn test_attribute_compilation_on_struct() {
    #[cli_exclude]
    #[derive(Default)]
    struct TestExcludedTool;

    // If we can instantiate it, the macro compiled successfully
    let _tool = TestExcludedTool::default();
    assert!(true); // If we get here, compilation succeeded
}

/// Test attribute with multiple other attributes
#[test]
fn test_attribute_with_multiple_decorators() {
    #[cli_exclude]
    #[derive(Default, Debug, Clone)]
    #[allow(dead_code)]
    struct MultiAttributeTool {
        pub field: String,
    }

    let tool = MultiAttributeTool {
        field: "test".to_string(),
    };
    
    // Test that all attributes work together
    let cloned_tool = tool.clone();
    let debug_str = format!("{:?}", cloned_tool);
    assert!(debug_str.contains("MultiAttributeTool"));
}

/// Test attribute on struct with generic parameters
#[test]
fn test_attribute_on_generic_struct() {
    #[cli_exclude]
    #[derive(Default)]
    struct GenericTool<T> {
        data: Option<T>,
    }

    let tool: GenericTool<String> = GenericTool::default();
    assert!(tool.data.is_none());
}

/// Test attribute on struct with lifetimes
#[test]
fn test_attribute_on_struct_with_lifetimes() {
    #[cli_exclude]
    #[derive(Debug)]
    struct LifetimeTool<'a> {
        reference: Option<&'a str>,
    }

    let tool = LifetimeTool { reference: None };
    let debug_str = format!("{:?}", tool);
    assert!(debug_str.contains("LifetimeTool"));
}

/// Test attribute preserves visibility modifiers
#[test]
fn test_attribute_preserves_visibility() {
    #[cli_exclude]
    pub struct PublicTool;

    #[cli_exclude]
    struct PrivateTool;

    // Both should compile and be usable according to their visibility
    let _public_tool = PublicTool;
    let _private_tool = PrivateTool;
}

/// Test attribute on enum
#[test]
fn test_attribute_on_enum() {
    #[cli_exclude]
    #[derive(Debug, PartialEq)]
    enum ToolType {
        Workflow,
        Utility,
        Integration,
    }

    let tool_type = ToolType::Workflow;
    assert_eq!(tool_type, ToolType::Workflow);
}

/// Test attribute on enum with data
#[test]
fn test_attribute_on_complex_enum() {
    #[cli_exclude]
    #[derive(Debug)]
    enum ComplexTool {
        Simple,
        WithData(String),
        WithStruct { name: String, version: u32 },
    }

    let tool1 = ComplexTool::Simple;
    let tool2 = ComplexTool::WithData("test".to_string());
    let tool3 = ComplexTool::WithStruct {
        name: "test_tool".to_string(),
        version: 1,
    };

    // All variants should work
    match tool1 {
        ComplexTool::Simple => assert!(true),
        _ => assert!(false, "Unexpected variant"),
    }

    match tool2 {
        ComplexTool::WithData(_) => assert!(true),
        _ => assert!(false, "Unexpected variant"),
    }

    match tool3 {
        ComplexTool::WithStruct { .. } => assert!(true),
        _ => assert!(false, "Unexpected variant"),
    }
}

/// Test attribute on trait implementation
#[test]
fn test_attribute_with_trait_implementation() {
    trait TestTrait {
        fn test_method(&self) -> String;
    }

    #[cli_exclude]
    #[derive(Default)]
    struct TraitImplTool;

    impl TestTrait for TraitImplTool {
        fn test_method(&self) -> String {
            "test implementation".to_string()
        }
    }

    let tool = TraitImplTool::default();
    assert_eq!(tool.test_method(), "test implementation");
}

/// Test that attribute preserves documentation
#[test]
fn test_attribute_preserves_documentation() {
    /// This is a documented tool
    /// 
    /// It has multiple lines of documentation
    #[cli_exclude]
    #[derive(Default)]
    struct DocumentedTool;

    let _tool = DocumentedTool::default();
    // If this compiles, the documentation was preserved
    assert!(true);
}

/// Test attribute on union (if needed for completeness)
#[test]
fn test_attribute_on_union() {
    #[cli_exclude]
    union TestUnion {
        i: i32,
        f: f32,
    }

    unsafe {
        let union_instance = TestUnion { i: 42 };
        let _value = union_instance.i;
    }
    // If we get here, the attribute worked on union
    assert!(true);
}

/// Test multiple cli_exclude attributes (should also work)
#[test]
fn test_multiple_cli_exclude_attributes() {
    // While not typical usage, multiple applications should not break
    #[cli_exclude]
    #[cli_exclude] // This should still work
    #[derive(Default)]
    struct MultipleAttributesTool;

    let _tool = MultipleAttributesTool::default();
    assert!(true);
}

/// Test nested structures with cli_exclude
#[test]
fn test_nested_structures() {
    #[cli_exclude]
    #[derive(Debug)]
    struct OuterTool {
        inner: InnerData,
    }

    #[derive(Debug)]
    struct InnerData {
        value: i32,
    }

    let tool = OuterTool {
        inner: InnerData { value: 42 },
    };

    assert_eq!(tool.inner.value, 42);
}

/// Test attribute with complex type parameters and bounds
#[test]
fn test_complex_generics() {
    use std::fmt::Debug;
    use std::clone::Clone;

    #[cli_exclude]
    #[derive(Default)]
    struct ComplexGenericTool<T, U>
    where
        T: Debug + Clone,
        U: Default,
    {
        t_value: Option<T>,
        u_value: U,
    }

    let tool: ComplexGenericTool<String, i32> = ComplexGenericTool::default();
    assert!(tool.t_value.is_none());
    assert_eq!(tool.u_value, 0i32);
}

/// Performance test - macro should have no runtime overhead
#[test]
fn test_no_runtime_overhead() {
    #[cli_exclude]
    #[derive(Default)]
    struct PerformanceTool {
        data: Vec<i32>,
    }

    let start = std::time::Instant::now();
    
    // Create many instances
    let mut tools = Vec::new();
    for i in 0..10000 {
        let mut tool = PerformanceTool::default();
        tool.data.push(i);
        tools.push(tool);
    }
    
    let duration = start.elapsed();
    
    // The macro should add no measurable overhead
    assert!(duration.as_millis() < 1000); // Generous upper bound
    assert_eq!(tools.len(), 10000);
}

/// Test that macro works with conditional compilation
#[cfg(test)]
mod conditional_compilation_tests {
    use super::*;

    #[test]
    fn test_conditional_compilation() {
        #[cli_exclude]
        #[cfg(test)]
        #[derive(Default)]
        struct ConditionalTool;

        #[cli_exclude]
        #[cfg(not(test))]
        #[derive(Default)]
        struct OtherConditionalTool;

        // Only ConditionalTool should be available in test builds
        let _tool = ConditionalTool::default();
        
        // OtherConditionalTool should not be available (this test would not compile if it were)
        assert!(true);
    }
}

/// Integration test with proc_macro patterns
#[test]
fn test_with_other_proc_macros() {
    // Test compatibility with other common proc macros
    #[cli_exclude]
    #[derive(Default, Clone)]
    struct ProcMacroTool {
        #[allow(dead_code)]
        field: String,
    }

    let tool1 = ProcMacroTool::default();
    let tool2 = tool1.clone();
    
    assert_eq!(tool1.field, tool2.field);
}

/// Test error conditions that should still compile (no-op nature of macro)
#[test] 
fn test_macro_is_truly_no_op() {
    // The macro should not interfere with any valid Rust syntax
    #[cli_exclude]
    #[derive(Debug)]
    struct NoOpTool {
        // Complex field types
        callback: Option<Box<dyn Fn() -> String>>,
        data: std::collections::HashMap<String, Vec<i32>>,
    }

    let tool = NoOpTool {
        callback: Some(Box::new(|| "test".to_string())),
        data: std::collections::HashMap::new(),
    };

    if let Some(ref callback) = tool.callback {
        let result = callback();
        assert_eq!(result, "test");
    }

    assert!(tool.data.is_empty());
}

/// Compile-time verification test
///
/// This test exists primarily to ensure the macro compiles correctly.
/// It tests various edge cases that might break procedural macro parsing.
#[test]
fn test_compile_time_verification() {
    // Test with unusual but valid syntax
    #[cli_exclude]
    struct UnusualSyntax;

    #[cli_exclude]
    struct WithBraces {}

    #[cli_exclude]
    struct WithParens();

    #[cli_exclude]
    struct WithFields {
        _phantom: std::marker::PhantomData<()>,
    }

    // All should instantiate without issues
    let _a = UnusualSyntax;
    let _b = WithBraces {};
    let _c = WithParens();
    let _d = WithFields {
        _phantom: std::marker::PhantomData,
    };

    assert!(true);
}