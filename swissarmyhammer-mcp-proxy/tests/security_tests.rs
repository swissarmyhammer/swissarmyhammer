use swissarmyhammer_mcp_proxy::ToolFilter;

/// Test that case variations cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_case_variation() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files_read"));
    assert!(!filter.is_allowed("Files_Read")); // Case sensitive
    assert!(!filter.is_allowed("FILES_READ"));
    assert!(!filter.is_allowed("files_READ"));
}

/// Test that whitespace cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_whitespace() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files_read"));
    assert!(!filter.is_allowed(" files_read")); // Leading space
    assert!(!filter.is_allowed("files_read ")); // Trailing space
    assert!(!filter.is_allowed("files read")); // Space in middle
    assert!(!filter.is_allowed("files\nread")); // Newline
    assert!(!filter.is_allowed("files\tread")); // Tab
}

/// Test that prefix/suffix matching cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_prefix_suffix() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files_read"));
    assert!(!filter.is_allowed("files_read_secret")); // Suffix
    assert!(!filter.is_allowed("files_reader")); // Suffix variation
    assert!(!filter.is_allowed("my_files_read")); // Prefix
    assert!(!filter.is_allowed("xfiles_read")); // Prefix char
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
    let filter = ToolFilter::new(vec!["^files_.*".to_string()], vec![]).unwrap();

    assert!(!filter.is_allowed("")); // Empty string doesn't match
}

/// Test that unicode characters don't bypass filters
#[test]
fn test_unicode_characters() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files_read"));
    assert!(!filter.is_allowed("files_read\u{200B}")); // Zero-width space
    assert!(!filter.is_allowed("files\u{200B}_read")); // Zero-width space in middle
    assert!(!filter.is_allowed("files_réad")); // Accented character
}

/// Test that URL encoding doesn't bypass filters
#[test]
fn test_url_encoded_characters() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files_read"));
    assert!(!filter.is_allowed("files%5Fread")); // %5F is underscore
    assert!(!filter.is_allowed("files%20read")); // %20 is space
}

/// Test that regex injection cannot bypass filters
#[test]
fn test_regex_injection_attempts() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files_read"));
    // These tool names contain regex metacharacters but should not match
    assert!(!filter.is_allowed("files_read|shell_execute"));
    assert!(!filter.is_allowed("files_read.*"));
    assert!(!filter.is_allowed("(files_read)"));
    assert!(!filter.is_allowed("[files_read]"));
}

/// Test that deny patterns are enforced when no allow patterns exist
#[test]
fn test_deny_patterns_enforced() {
    let filter = ToolFilter::new(
        vec![], // Empty allow list means allow all except denied
        vec!["^shell_.*".to_string(), ".*_write$".to_string()],
    )
    .unwrap();

    // Denied tools
    assert!(!filter.is_allowed("shell_execute"));
    assert!(!filter.is_allowed("shell_kill"));
    assert!(!filter.is_allowed("files_write"));
    assert!(!filter.is_allowed("dangerous_write"));

    // Allowed tools
    assert!(filter.is_allowed("files_read"));
    assert!(filter.is_allowed("web_fetch"));

    // Variations - these should still be blocked or allowed based on pattern matching
    assert!(filter.is_allowed("Shell_execute")); // Case variation doesn't match deny pattern, and no allow patterns, so allowed
    assert!(!filter.is_allowed("shell_execute ")); // Trailing space still matches ^shell_.* pattern
}

/// Test combined allow and deny with bypass attempts
#[test]
fn test_combined_filters_security() {
    let filter = ToolFilter::new(
        vec!["^files_.*".to_string()], // Allow files_*
        vec![".*_write$".to_string()], // Deny *_write
    )
    .unwrap();

    // files_write matches allow pattern, so it's allowed (allow wins)
    assert!(filter.is_allowed("files_write"));

    // Variations don't match the allow pattern ^files_.*, so they're denied
    assert!(!filter.is_allowed("files write")); // Space instead of underscore - doesn't match ^files_.*
    assert!(filter.is_allowed("files_write ")); // Trailing space still matches ^files_.* (the .* matches the space too)
    assert!(!filter.is_allowed("Files_write")); // Case - doesn't match ^files_.* (case sensitive)
}

/// Test that long tool names are handled correctly
#[test]
fn test_very_long_tool_names() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

    let long_name = "a".repeat(10000);
    assert!(!filter.is_allowed(&long_name));

    let long_prefix = format!("{}files_read", "x".repeat(1000));
    assert!(!filter.is_allowed(&long_prefix));
}

/// Test that control characters don't bypass filters
#[test]
fn test_control_characters() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("files_read"));
    assert!(!filter.is_allowed("files\x00read")); // Null byte
    assert!(!filter.is_allowed("files\x01read")); // SOH
    assert!(!filter.is_allowed("files\x1Bread")); // ESC
}
