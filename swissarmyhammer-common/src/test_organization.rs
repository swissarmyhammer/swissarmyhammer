//! Test organization utilities and patterns
//!
//! This module provides utilities for organizing tests more efficiently,
//! reducing duplication and improving maintainability. These utilities
//! support the test consolidation effort to reduce the overall test count
//! while maintaining comprehensive coverage.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

/// Parameterized test runner for reducing test duplication
///
/// This utility allows running the same test logic across multiple parameter sets,
/// reducing the need for individual test functions for similar test cases.
///
/// # Example
///
/// ```rust,ignore
/// // Example usage in test files:
/// use swissarmyhammer::test_organization::TestMatrix;
///
/// #[derive(Debug, Clone)]
/// struct ParseTestCase {
///     input: &'static str,
///     expected: &'static str,
/// }
///
/// // In your test module:
/// fn test_parse_actions() {
///     let test_cases = vec![
///         ParseTestCase { input: r#"Execute prompt "test""#, expected: "prompt" },
///         ParseTestCase { input: "Wait 30 seconds", expected: "wait" },
///         ParseTestCase { input: r#"Log "message""#, expected: "log" },
///     ];
///
///     TestMatrix::new("parse_actions")
///         .run_tests(test_cases, |case| {
///             let result = parse_action(&case.input);
///             assert_eq!(result.action_type(), case.expected);
///         });
/// }
/// ```
pub struct TestMatrix<T> {
    name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TestMatrix<T>
where
    T: Debug + Clone + Send + 'static,
{
    /// Create a new test matrix with a descriptive name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Run tests for all provided test cases
    ///
    /// Each test case is run individually with descriptive failure messages
    /// that include the test case details for easier debugging.
    pub fn run_tests<F>(&self, test_cases: Vec<T>, test_fn: F)
    where
        F: Fn(&T) + std::panic::RefUnwindSafe,
        T: std::panic::RefUnwindSafe,
    {
        for (index, case) in test_cases.iter().enumerate() {
            let result = std::panic::catch_unwind(|| test_fn(case));

            if let Err(panic_info) = result {
                // Extract panic message if possible
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "Unknown panic".to_string()
                };

                panic!(
                    "Test matrix '{}' failed on case {} ({:?}): {}",
                    self.name, index, case, panic_msg
                );
            }
        }
    }

    /// Run async tests for all provided test cases
    pub async fn run_async_tests<F, Fut>(&self, test_cases: Vec<T>, test_fn: F)
    where
        F: Fn(T) -> Fut + Send + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        for (index, case) in test_cases.into_iter().enumerate() {
            // Use tokio::spawn to isolate panics
            let case_clone = case.clone();
            let name = self.name.clone();
            let test_fn_clone = test_fn.clone();

            let result = tokio::spawn(async move { test_fn_clone(case_clone).await }).await;

            if let Err(join_error) = result {
                if join_error.is_panic() {
                    panic!(
                        "Async test matrix '{}' failed on case {} ({:?}): {:?}",
                        name, index, case, join_error
                    );
                }
            }
        }
    }
}

/// Property-based test generator for common patterns
///
/// Provides pre-defined test case generators for common testing scenarios
/// to reduce boilerplate in property-based tests.
pub struct PropertyTestGenerator;

impl PropertyTestGenerator {
    /// Generate test cases for string parsing operations
    ///
    /// Returns test cases that cover edge cases like empty strings,
    /// whitespace, special characters, and unicode.
    pub fn string_parsing_cases() -> Vec<(&'static str, &'static str)> {
        vec![
            ("", "empty"),
            ("   ", "whitespace_only"),
            ("hello", "simple"),
            ("hello world", "with_space"),
            ("hello\nworld", "with_newline"),
            ("hello\tworld", "with_tab"),
            ("hello\"world", "with_quote"),
            ("hello'world", "with_apostrophe"),
            ("hello\\world", "with_backslash"),
            ("hello/world", "with_slash"),
            ("hello.world", "with_dot"),
            ("hello-world", "with_dash"),
            ("hello_world", "with_underscore"),
            ("HELLO", "uppercase"),
            ("Hello", "mixed_case"),
            ("123", "numeric"),
            ("hello123", "alphanumeric"),
            ("!@#$%", "special_chars"),
            ("héllo", "unicode_accented"),
            ("🚀", "unicode_emoji"),
        ]
    }

    /// Generate test cases for duration parsing/formatting
    pub fn duration_test_cases() -> Vec<(&'static str, u64)> {
        vec![
            ("1s", 1),
            ("30s", 30),
            ("1m", 60),
            ("5m", 300),
            ("1h", 3600),
            ("2h", 7200),
            ("1d", 86400),
            ("0s", 0),
        ]
    }

    /// Generate test cases for file path operations
    pub fn file_path_test_cases() -> Vec<(&'static str, bool)> {
        vec![
            ("test.txt", true),
            ("/absolute/path.txt", true),
            ("./relative/path.txt", true),
            ("../parent/path.txt", true),
            ("path with spaces.txt", true),
            ("path-with-dashes.txt", true),
            ("path_with_underscores.txt", true),
            ("PATH.TXT", true),
            ("file.with.dots.txt", true),
            ("", false),
            (".", false),
            ("..", false),
            ("///", false),
            ("file\0with\0null.txt", false),
        ]
    }

    /// Generate test cases for variable substitution patterns
    pub fn variable_substitution_cases() -> Vec<(
        &'static str,
        HashMap<&'static str, &'static str>,
        &'static str,
    )> {
        let mut vars1 = HashMap::new();
        vars1.insert("name", "test");
        vars1.insert("value", "42");

        let mut vars2 = HashMap::new();
        vars2.insert("file", "example.rs");
        vars2.insert("count", "100");

        vec![
            ("Hello ${name}", vars1.clone(), "Hello test"),
            ("Value: ${value}", vars1.clone(), "Value: 42"),
            ("${name} = ${value}", vars1, "test = 42"),
            (
                "Process ${file} with ${count} items",
                vars2,
                "Process example.rs with 100 items",
            ),
            ("No variables here", HashMap::new(), "No variables here"),
        ]
    }
}

/// Test assertion helpers for common patterns
///
/// Provides more descriptive assertion macros and helpers to improve
/// test failure messages and reduce assertion boilerplate.
pub struct TestAssertions;

impl TestAssertions {
    /// Assert that a result contains an expected error type
    pub fn assert_error_type<T, E>(result: &Result<T, E>, expected_error_fragment: &str)
    where
        E: Debug,
    {
        match result {
            Ok(_) => panic!(
                "Expected error containing '{}', but got Ok",
                expected_error_fragment
            ),
            Err(e) => {
                let error_debug = format!("{:?}", e);
                assert!(
                    error_debug.contains(expected_error_fragment),
                    "Error '{}' does not contain expected fragment '{}'",
                    error_debug,
                    expected_error_fragment
                );
            }
        }
    }

    /// Assert that a collection contains all expected items
    pub fn assert_contains_all<T>(collection: &[T], expected_items: &[T])
    where
        T: Debug + PartialEq,
    {
        for expected in expected_items {
            assert!(
                collection.contains(expected),
                "Collection {:?} does not contain expected item {:?}",
                collection,
                expected
            );
        }
    }

    /// Assert that a string matches a pattern without regex dependency
    pub fn assert_string_pattern(actual: &str, pattern_fragments: &[&str]) {
        for fragment in pattern_fragments {
            assert!(
                actual.contains(fragment),
                "String '{}' does not contain expected fragment '{}'",
                actual,
                fragment
            );
        }
    }

    /// Assert that a hash map contains all expected key-value pairs
    pub fn assert_map_contains<K, V>(map: &HashMap<K, V>, expected_pairs: &[(K, V)])
    where
        K: Debug + Eq + std::hash::Hash + Clone,
        V: Debug + PartialEq + Clone,
    {
        for (key, expected_value) in expected_pairs {
            match map.get(key) {
                Some(actual_value) => {
                    assert_eq!(
                        actual_value, expected_value,
                        "Map value for key {:?} is {:?}, expected {:?}",
                        key, actual_value, expected_value
                    );
                }
                None => panic!("Map does not contain expected key {:?}", key),
            }
        }
    }
}

/// Mock builders for common test objects
///
/// Provides builder patterns for creating test objects with sensible defaults,
/// reducing setup boilerplate in tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockActionBuilder {
    action_type: String,
    description: String,
    parameters: HashMap<String, String>,
}

impl Default for MockActionBuilder {
    fn default() -> Self {
        Self {
            action_type: "test_action".to_string(),
            description: "Test action description".to_string(),
            parameters: HashMap::new(),
        }
    }
}

impl MockActionBuilder {
    /// Create a new mock action builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the action type
    pub fn action_type(mut self, action_type: &str) -> Self {
        self.action_type = action_type.to_string();
        self
    }

    /// Set the description
    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Add a parameter
    pub fn parameter(mut self, key: &str, value: &str) -> Self {
        self.parameters.insert(key.to_string(), value.to_string());
        self
    }

    /// Add multiple parameters
    pub fn parameters(mut self, params: HashMap<String, String>) -> Self {
        self.parameters.extend(params);
        self
    }

    /// Build the mock action
    pub fn build(self) -> MockAction {
        MockAction {
            action_type: self.action_type,
            description: self.description,
            parameters: self.parameters,
        }
    }
}

/// Mock action for testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MockAction {
    /// The type of action
    pub action_type: String,
    /// Human-readable description of the action
    pub description: String,
    /// Key-value parameters for the action
    pub parameters: HashMap<String, String>,
}

impl MockAction {
    /// Create a new mock action builder
    pub fn builder() -> MockActionBuilder {
        MockActionBuilder::new()
    }

    /// Get the action type
    pub fn action_type(&self) -> &str {
        &self.action_type
    }

    /// Get the description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get a parameter value
    pub fn get_parameter(&self, key: &str) -> Option<&String> {
        self.parameters.get(key)
    }
}

/// Test timing utilities for performance testing
pub struct TestTiming;

impl TestTiming {
    /// Time a function execution and assert it completes within expected duration
    pub fn assert_completes_within<F, R>(duration: std::time::Duration, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        let result = f();
        let elapsed = start.elapsed();

        assert!(
            elapsed <= duration,
            "Function took {:?}, expected to complete within {:?}",
            elapsed,
            duration
        );

        result
    }

    /// Time an async function and assert it completes within expected duration
    pub async fn assert_async_completes_within<F, Fut, R>(duration: std::time::Duration, f: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = std::time::Instant::now();
        let result = f().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed <= duration,
            "Async function took {:?}, expected to complete within {:?}",
            elapsed,
            duration
        );

        result
    }

    /// Measure function performance and return timing information
    pub fn measure<F, R>(f: F) -> (R, std::time::Duration)
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_basic() {
        #[derive(Debug, Clone)]
        struct TestCase {
            input: i32,
            expected: i32,
        }

        let test_cases = vec![
            TestCase {
                input: 1,
                expected: 2,
            },
            TestCase {
                input: 2,
                expected: 4,
            },
            TestCase {
                input: 3,
                expected: 6,
            },
        ];

        TestMatrix::new("multiplication_by_two").run_tests(test_cases, |case| {
            assert_eq!(case.input * 2, case.expected);
        });
    }

    #[test]
    fn test_property_generator_strings() {
        let cases = PropertyTestGenerator::string_parsing_cases();
        assert!(!cases.is_empty());

        // Verify we have expected edge cases
        let inputs: Vec<&str> = cases.iter().map(|(input, _)| *input).collect();
        assert!(inputs.contains(&""));
        assert!(inputs.contains(&"   "));
        assert!(inputs.contains(&"🚀"));
    }

    #[test]
    fn test_property_generator_durations() {
        let cases = PropertyTestGenerator::duration_test_cases();
        assert!(!cases.is_empty());

        // Verify some expected cases
        assert!(cases.contains(&("1s", 1)));
        assert!(cases.contains(&("1m", 60)));
        assert!(cases.contains(&("1h", 3600)));
    }

    #[test]
    fn test_assertions_error_type() {
        let result: Result<(), &str> = Err("This is a test error");
        TestAssertions::assert_error_type(&result, "test error");
    }

    #[test]
    #[should_panic]
    fn test_assertions_error_type_wrong() {
        let result: Result<(), &str> = Err("This is a test error");
        TestAssertions::assert_error_type(&result, "different error");
    }

    #[test]
    fn test_assertions_contains_all() {
        let collection = vec![1, 2, 3, 4, 5];
        let expected = vec![2, 4];
        TestAssertions::assert_contains_all(&collection, &expected);
    }

    #[test]
    fn test_assertions_string_pattern() {
        let text = "Hello, world! This is a test.";
        let patterns = vec!["Hello", "world", "test"];
        TestAssertions::assert_string_pattern(text, &patterns);
    }

    #[test]
    fn test_assertions_map_contains() {
        let mut map = HashMap::new();
        map.insert("key1", "value1");
        map.insert("key2", "value2");
        map.insert("key3", "value3");

        let expected_pairs = vec![("key1", "value1"), ("key3", "value3")];
        TestAssertions::assert_map_contains(&map, &expected_pairs);
    }

    #[test]
    fn test_mock_action_builder() {
        let mut params = HashMap::new();
        params.insert("arg1".to_string(), "value1".to_string());

        let action = MockAction::builder()
            .action_type("prompt")
            .description("Execute a prompt")
            .parameter("prompt_name", "test-prompt")
            .parameters(params)
            .build();

        assert_eq!(action.action_type(), "prompt");
        assert_eq!(action.description(), "Execute a prompt");
        assert_eq!(
            action.get_parameter("prompt_name"),
            Some(&"test-prompt".to_string())
        );
        assert_eq!(action.get_parameter("arg1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_timing_completes_within() {
        let result =
            TestTiming::assert_completes_within(std::time::Duration::from_millis(100), || {
                std::thread::sleep(std::time::Duration::from_millis(10));
                42
            });
        assert_eq!(result, 42);
    }

    #[test]
    fn test_timing_measure() {
        let (result, duration) = TestTiming::measure(|| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            "test result"
        });

        assert_eq!(result, "test result");
        assert!(duration >= std::time::Duration::from_millis(10));
        assert!(duration < std::time::Duration::from_millis(100)); // Should be much less
    }

    #[test]
    #[should_panic(expected = "Test matrix")]
    fn test_matrix_reports_failure_with_context() {
        #[derive(Debug, Clone)]
        struct FailCase {
            value: i32,
        }

        let cases = vec![FailCase { value: 42 }];
        TestMatrix::new("should_fail").run_tests(cases, |case| {
            assert_eq!(case.value, 99, "wrong value");
        });
    }

    #[test]
    fn test_matrix_empty_cases() {
        #[derive(Debug, Clone)]
        struct EmptyCase;

        let cases: Vec<EmptyCase> = vec![];
        // Should not panic with empty cases
        TestMatrix::new("empty").run_tests(cases, |_case| {
            panic!("Should not be called");
        });
    }

    #[test]
    fn test_matrix_single_case() {
        #[derive(Debug, Clone)]
        struct SingleCase(i32);

        let cases = vec![SingleCase(1)];
        TestMatrix::new("single").run_tests(cases, |case| {
            assert_eq!(case.0, 1);
        });
    }

    #[test]
    #[should_panic(expected = "Unknown panic")]
    fn test_matrix_non_string_panic() {
        #[derive(Debug, Clone)]
        struct PanicCase;

        let cases = vec![PanicCase];
        TestMatrix::new("non_string_panic").run_tests(cases, |_case| {
            std::panic::panic_any(42_i32);
        });
    }

    #[tokio::test]
    async fn test_matrix_async_basic() {
        #[derive(Debug, Clone)]
        struct AsyncCase {
            input: i32,
            expected: i32,
        }

        let cases = vec![
            AsyncCase {
                input: 1,
                expected: 2,
            },
            AsyncCase {
                input: 5,
                expected: 10,
            },
        ];

        TestMatrix::new("async_multiply")
            .run_async_tests(cases, |case| async move {
                assert_eq!(case.input * 2, case.expected);
            })
            .await;
    }

    #[tokio::test]
    #[should_panic(expected = "Async test matrix")]
    async fn test_matrix_async_failure() {
        #[derive(Debug, Clone)]
        struct AsyncFailCase {
            value: i32,
        }

        let cases = vec![AsyncFailCase { value: 1 }];

        TestMatrix::new("async_fail")
            .run_async_tests(cases, |case| async move {
                assert_eq!(case.value, 999);
            })
            .await;
    }

    #[tokio::test]
    async fn test_matrix_async_empty_cases() {
        #[derive(Debug, Clone)]
        struct EmptyAsync;

        let cases: Vec<EmptyAsync> = vec![];
        TestMatrix::new("async_empty")
            .run_async_tests(cases, |_case| async move {
                panic!("Should not be called");
            })
            .await;
    }

    #[test]
    fn test_property_generator_file_paths() {
        let cases = PropertyTestGenerator::file_path_test_cases();
        assert!(!cases.is_empty());

        // Verify we have both valid and invalid cases
        let valid_count = cases.iter().filter(|(_, valid)| *valid).count();
        let invalid_count = cases.iter().filter(|(_, valid)| !*valid).count();
        assert!(valid_count > 0);
        assert!(invalid_count > 0);

        // Verify specific expected cases
        assert!(cases.contains(&("test.txt", true)));
        assert!(cases.contains(&("", false)));
    }

    #[test]
    fn test_property_generator_variable_substitution() {
        let cases = PropertyTestGenerator::variable_substitution_cases();
        assert!(!cases.is_empty());

        // Verify the case with no variables
        let no_vars_case = cases
            .iter()
            .find(|(input, _, _)| *input == "No variables here");
        assert!(no_vars_case.is_some());
        let (_, vars, expected) = no_vars_case.unwrap();
        assert!(vars.is_empty());
        assert_eq!(*expected, "No variables here");

        // Verify cases with variables
        let name_case = cases.iter().find(|(input, _, _)| *input == "Hello ${name}");
        assert!(name_case.is_some());
        let (_, vars, expected) = name_case.unwrap();
        assert_eq!(*vars.get("name").unwrap(), "test");
        assert_eq!(*expected, "Hello test");
    }

    #[test]
    #[should_panic(expected = "Expected error")]
    fn test_assertions_error_type_on_ok_panics() {
        let result: Result<i32, &str> = Ok(42);
        TestAssertions::assert_error_type(&result, "some error");
    }

    #[test]
    #[should_panic(expected = "does not contain expected item")]
    fn test_assertions_contains_all_missing_item() {
        let collection = vec![1, 2, 3];
        let expected = vec![4];
        TestAssertions::assert_contains_all(&collection, &expected);
    }

    #[test]
    fn test_assertions_contains_all_empty_expected() {
        let collection = vec![1, 2, 3];
        let expected: Vec<i32> = vec![];
        // Should succeed - empty expected is always satisfied
        TestAssertions::assert_contains_all(&collection, &expected);
    }

    #[test]
    #[should_panic(expected = "does not contain expected fragment")]
    fn test_assertions_string_pattern_missing_fragment() {
        let text = "Hello world";
        let patterns = vec!["Hello", "missing"];
        TestAssertions::assert_string_pattern(text, &patterns);
    }

    #[test]
    fn test_assertions_string_pattern_empty_patterns() {
        let text = "Hello world";
        let patterns: Vec<&str> = vec![];
        // Should succeed - empty patterns always satisfied
        TestAssertions::assert_string_pattern(text, &patterns);
    }

    #[test]
    #[should_panic(expected = "does not contain expected key")]
    fn test_assertions_map_contains_missing_key() {
        let mut map = HashMap::new();
        map.insert("a", "1");

        let expected = vec![("b", "2")];
        TestAssertions::assert_map_contains(&map, &expected);
    }

    #[test]
    #[should_panic(expected = "Map value for key")]
    fn test_assertions_map_contains_wrong_value() {
        let mut map = HashMap::new();
        map.insert("a", "1");

        let expected = vec![("a", "2")];
        TestAssertions::assert_map_contains(&map, &expected);
    }

    #[test]
    fn test_assertions_map_contains_empty_expected() {
        let mut map = HashMap::new();
        map.insert("a", "1");

        let expected: Vec<(&str, &str)> = vec![];
        TestAssertions::assert_map_contains(&map, &expected);
    }

    #[test]
    fn test_mock_action_builder_default() {
        let builder = MockActionBuilder::default();
        let action = builder.build();

        assert_eq!(action.action_type(), "test_action");
        assert_eq!(action.description(), "Test action description");
        assert!(action.parameters.is_empty());
    }

    #[test]
    fn test_mock_action_builder_new() {
        let builder = MockActionBuilder::new();
        let action = builder.build();

        assert_eq!(action.action_type(), "test_action");
        assert_eq!(action.description(), "Test action description");
    }

    #[test]
    fn test_mock_action_builder_chain() {
        let action = MockActionBuilder::new()
            .action_type("deploy")
            .description("Deploy to production")
            .parameter("env", "prod")
            .parameter("version", "1.0.0")
            .build();

        assert_eq!(action.action_type(), "deploy");
        assert_eq!(action.description(), "Deploy to production");
        assert_eq!(action.get_parameter("env"), Some(&"prod".to_string()));
        assert_eq!(action.get_parameter("version"), Some(&"1.0.0".to_string()));
    }

    #[test]
    fn test_mock_action_builder_parameters_bulk() {
        let mut params = HashMap::new();
        params.insert("key1".to_string(), "val1".to_string());
        params.insert("key2".to_string(), "val2".to_string());

        let action = MockActionBuilder::new().parameters(params).build();

        assert_eq!(action.get_parameter("key1"), Some(&"val1".to_string()));
        assert_eq!(action.get_parameter("key2"), Some(&"val2".to_string()));
    }

    #[test]
    fn test_mock_action_builder_static_method() {
        let action = MockAction::builder().action_type("test").build();

        assert_eq!(action.action_type(), "test");
    }

    #[test]
    fn test_mock_action_get_parameter_missing() {
        let action = MockAction::builder().build();
        assert_eq!(action.get_parameter("nonexistent"), None);
    }

    #[test]
    fn test_mock_action_serialization() {
        let action = MockAction::builder()
            .action_type("serialize_test")
            .description("Test serialization")
            .parameter("key", "value")
            .build();

        let json = serde_json::to_string(&action).unwrap();
        let deserialized: MockAction = serde_json::from_str(&json).unwrap();

        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_mock_action_clone() {
        let action = MockAction::builder()
            .action_type("clone_test")
            .parameter("k", "v")
            .build();

        let cloned = action.clone();
        assert_eq!(action, cloned);
    }

    #[test]
    fn test_mock_action_debug() {
        let action = MockAction::builder().action_type("debug_test").build();
        let debug = format!("{:?}", action);
        assert!(debug.contains("debug_test"));
    }

    #[test]
    #[should_panic(expected = "expected to complete within")]
    fn test_timing_completes_within_exceeds() {
        TestTiming::assert_completes_within(std::time::Duration::from_millis(1), || {
            std::thread::sleep(std::time::Duration::from_millis(50));
        });
    }

    #[test]
    fn test_timing_measure_fast() {
        let (result, duration) = TestTiming::measure(|| 42);
        assert_eq!(result, 42);
        assert!(duration < std::time::Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_timing_async_completes_within() {
        let result = TestTiming::assert_async_completes_within(
            std::time::Duration::from_secs(1),
            || async { 42 },
        )
        .await;
        assert_eq!(result, 42);
    }

    #[test]
    fn test_matrix_name_stored() {
        let matrix: TestMatrix<i32> = TestMatrix::new("my_matrix");
        assert_eq!(matrix.name, "my_matrix");
    }

    #[test]
    fn test_property_generator_string_parsing_categories() {
        let cases = PropertyTestGenerator::string_parsing_cases();
        let labels: Vec<&str> = cases.iter().map(|(_, label)| *label).collect();

        // Verify all categories are present
        assert!(labels.contains(&"empty"));
        assert!(labels.contains(&"whitespace_only"));
        assert!(labels.contains(&"simple"));
        assert!(labels.contains(&"with_space"));
        assert!(labels.contains(&"with_newline"));
        assert!(labels.contains(&"with_tab"));
        assert!(labels.contains(&"with_quote"));
        assert!(labels.contains(&"with_apostrophe"));
        assert!(labels.contains(&"with_backslash"));
        assert!(labels.contains(&"with_slash"));
        assert!(labels.contains(&"with_dot"));
        assert!(labels.contains(&"with_dash"));
        assert!(labels.contains(&"with_underscore"));
        assert!(labels.contains(&"uppercase"));
        assert!(labels.contains(&"mixed_case"));
        assert!(labels.contains(&"numeric"));
        assert!(labels.contains(&"alphanumeric"));
        assert!(labels.contains(&"special_chars"));
        assert!(labels.contains(&"unicode_accented"));
        assert!(labels.contains(&"unicode_emoji"));
    }

    #[test]
    fn test_property_generator_duration_all_cases() {
        let cases = PropertyTestGenerator::duration_test_cases();

        // Verify all expected duration mappings
        assert!(cases.contains(&("30s", 30)));
        assert!(cases.contains(&("5m", 300)));
        assert!(cases.contains(&("2h", 7200)));
        assert!(cases.contains(&("1d", 86400)));
        assert!(cases.contains(&("0s", 0)));
    }

    #[test]
    fn test_property_generator_file_path_all_valid() {
        let cases = PropertyTestGenerator::file_path_test_cases();

        // Check specific valid path patterns
        assert!(cases.contains(&("/absolute/path.txt", true)));
        assert!(cases.contains(&("./relative/path.txt", true)));
        assert!(cases.contains(&("../parent/path.txt", true)));
        assert!(cases.contains(&("path with spaces.txt", true)));
        assert!(cases.contains(&("path-with-dashes.txt", true)));
        assert!(cases.contains(&("path_with_underscores.txt", true)));
        assert!(cases.contains(&("PATH.TXT", true)));
        assert!(cases.contains(&("file.with.dots.txt", true)));
    }

    #[test]
    fn test_property_generator_file_path_all_invalid() {
        let cases = PropertyTestGenerator::file_path_test_cases();

        // Check specific invalid path patterns
        assert!(cases.contains(&(".", false)));
        assert!(cases.contains(&("..", false)));
        assert!(cases.contains(&("///", false)));
    }

    #[test]
    fn test_property_generator_variable_substitution_multi_var() {
        let cases = PropertyTestGenerator::variable_substitution_cases();

        // Find the multi-variable case
        let multi_case = cases
            .iter()
            .find(|(input, _, _)| *input == "${name} = ${value}");
        assert!(multi_case.is_some());
        let (_, vars, expected) = multi_case.unwrap();
        assert_eq!(*vars.get("name").unwrap(), "test");
        assert_eq!(*vars.get("value").unwrap(), "42");
        assert_eq!(*expected, "test = 42");
    }

    #[test]
    fn test_property_generator_variable_substitution_file_case() {
        let cases = PropertyTestGenerator::variable_substitution_cases();

        let file_case = cases.iter().find(|(input, _, _)| input.contains("${file}"));
        assert!(file_case.is_some());
        let (_, vars, expected) = file_case.unwrap();
        assert_eq!(*vars.get("file").unwrap(), "example.rs");
        assert_eq!(*vars.get("count").unwrap(), "100");
        assert_eq!(*expected, "Process example.rs with 100 items");
    }
}
