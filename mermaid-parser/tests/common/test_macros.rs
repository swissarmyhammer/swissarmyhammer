//! Test macros to reduce boilerplate

/// Macro to create a file-based test function with standard patterns
#[macro_export]
macro_rules! create_diagram_file_test {
    ($test_name:ident, $diagram_type:literal, $expected_prefix:literal) => {
        #[rstest]
        fn $test_name(#[files(concat!("test/", $diagram_type, "/*.mermaid"))] path: PathBuf) {
            let content = $crate::common::read_and_clean_test_file(&path);

            if $crate::common::should_skip_file(&content, $expected_prefix) {
                return;
            }

            let result = parse_diagram(&content);
            $crate::common::assert_parse_success_any(result, &path);
        }
    };
}

/// Macro to create a file-based test with custom validation
#[macro_export]
macro_rules! create_diagram_file_test_with_validation {
    ($test_name:ident, $diagram_type:literal, $expected_prefix:literal, $validation:expr) => {
        #[rstest]
        fn $test_name(#[files(concat!("test/", $diagram_type, "/*.mermaid"))] path: PathBuf) {
            let content = $crate::common::read_and_clean_test_file(&path);

            if $crate::common::should_skip_file(&content, $expected_prefix) {
                return;
            }

            let result = parse_diagram(&content);
            let diagram = $crate::common::assert_parse_success_any(result, &path);

            // Run custom validation
            $validation(diagram, &path);
        }
    };
}

/// Macro to create a file-based test with unsupported syntax filtering
#[macro_export]
macro_rules! create_diagram_file_test_with_filter {
    ($test_name:ident, $diagram_type:literal, $expected_prefix:literal, $filter_fn:expr) => {
        #[rstest]
        fn $test_name(#[files(concat!("test/", $diagram_type, "/*.mermaid"))] path: PathBuf) {
            let content = $crate::common::read_and_clean_test_file(&path);

            if $crate::common::should_skip_file(&content, $expected_prefix) {
                return;
            }

            if $filter_fn(&content) {
                return; // Skip files with unsupported syntax
            }

            let result = parse_diagram(&content);
            $crate::common::assert_parse_success_any(result, &path);
        }
    };
}
