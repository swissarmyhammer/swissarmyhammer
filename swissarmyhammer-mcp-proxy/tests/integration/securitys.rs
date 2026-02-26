use swissarmyhammer_mcp_proxy::ToolFilter;

/// Test that case variations cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_case_variation() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files"));
    assert!(!filter.is_allowed("Files")); // Case sensitive
    assert!(!filter.is_allowed("FILES"));
    assert!(!filter.is_allowed("fiLES"));
}

/// Test that whitespace cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_whitespace() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files"));
    assert!(!filter.is_allowed(" files")); // Leading space
    assert!(!filter.is_allowed("files ")); // Trailing space
    assert!(!filter.is_allowed("fi les")); // Space in middle
    assert!(!filter.is_allowed("fi\nles")); // Newline
    assert!(!filter.is_allowed("fi\tles")); // Tab
}

/// Test that prefix/suffix matching cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_prefix_suffix() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files"));
    assert!(!filter.is_allowed("files_secret")); // Suffix
    assert!(!filter.is_allowed("files2")); // Suffix variation
    assert!(!filter.is_allowed("my_files")); // Prefix
    assert!(!filter.is_allowed("xfiles")); // Prefix char
}

/// Test that special regex characters in tool names are handled correctly
#[test]
fn test_special_characters_in_tool_names() {
    let filter = ToolFilter::new(vec!["^test\\.tool$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("test.tool")); // Literal dot
    assert!(!filter.is_allowed("test_tool")); // Underscore instead of dot
    assert!(!filter.is_allowed("testXtool")); // Any char instead of dot
}

/// Test that empty strings are handled correctly
#[test]
fn test_empty_string_tool_name() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    assert!(!filter.is_allowed("")); // Empty string doesn't match
}

/// Test that unicode characters don't bypass filters
#[test]
fn test_unicode_characters() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files"));
    assert!(!filter.is_allowed("files\u{200B}")); // Zero-width space
    assert!(!filter.is_allowed("fi\u{200B}les")); // Zero-width space in middle
    assert!(!filter.is_allowed("filés")); // Accented character
}

/// Test that URL encoding doesn't bypass filters
#[test]
fn test_url_encoded_characters() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files"));
    assert!(!filter.is_allowed("fi%6Ces")); // %6C is 'l'
    assert!(!filter.is_allowed("files%00")); // %00 is null
}

/// Test that regex injection cannot bypass filters
#[test]
fn test_regex_injection_attempts() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files"));
    // These tool names contain regex metacharacters but should not match
    assert!(!filter.is_allowed("files|shell_execute"));
    assert!(!filter.is_allowed("files.*"));
    assert!(!filter.is_allowed("(files)"));
    assert!(!filter.is_allowed("[files]"));
}

/// Test that deny patterns are enforced when no allow patterns exist
#[test]
fn test_deny_patterns_enforced() {
    let filter = ToolFilter::new(
        vec![], // Empty allow list means allow all except denied
        vec!["^shell_.*".to_string(), "^kanban$".to_string()],
    )
    .unwrap();

    // Denied tools
    assert!(!filter.is_allowed("shell_execute"));
    assert!(!filter.is_allowed("shell_kill"));
    assert!(!filter.is_allowed("kanban"));

    // Allowed tools
    assert!(filter.is_allowed("files"));
    assert!(filter.is_allowed("web"));

    // Variations - these should still be blocked or allowed based on pattern matching
    assert!(filter.is_allowed("Shell_execute")); // Case variation doesn't match deny pattern, and no allow patterns, so allowed
    assert!(!filter.is_allowed("shell_execute ")); // Trailing space still matches ^shell_.* pattern
}

/// Test combined allow and deny with bypass attempts
#[test]
fn test_combined_filters_security() {
    let filter = ToolFilter::new(
        vec!["^files$".to_string()],   // Allow files
        vec!["^shell_.*".to_string()], // Deny shell tools
    )
    .unwrap();

    // files matches allow pattern, so it's allowed
    assert!(filter.is_allowed("files"));

    // Variations don't match the allow pattern ^files$, so they're denied
    assert!(!filter.is_allowed("files2")); // Suffix - doesn't match ^files$
    assert!(!filter.is_allowed("Files")); // Case - doesn't match ^files$ (case sensitive)
    assert!(!filter.is_allowed("shell_execute")); // Not in allow list
}

/// Test that long tool names are handled correctly
#[test]
fn test_very_long_tool_names() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    let long_name = "a".repeat(10000);
    assert!(!filter.is_allowed(&long_name));

    let long_prefix = format!("{}files", "x".repeat(1000));
    assert!(!filter.is_allowed(&long_prefix));
}

/// Test that control characters don't bypass filters
#[test]
fn test_control_characters() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files"));
    assert!(!filter.is_allowed("files\x00")); // Null byte
    assert!(!filter.is_allowed("fi\x01les")); // SOH
    assert!(!filter.is_allowed("fi\x1Bles")); // ESC
}
