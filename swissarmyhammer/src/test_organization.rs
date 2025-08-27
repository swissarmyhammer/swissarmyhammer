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
}
