use swissarmyhammer_mcp_proxy::ToolFilter;

/// Test that case variations cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_case_variation() {
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("git"));
    assert!(!filter.is_allowed("Git")); // Case sensitive
    assert!(!filter.is_allowed("GIT"));
    assert!(!filter.is_allowed("giT"));
}

/// Test that whitespace cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_whitespace() {
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("git"));
    assert!(!filter.is_allowed(" git")); // Leading space
    assert!(!filter.is_allowed("git ")); // Trailing space
    assert!(!filter.is_allowed("g it")); // Space in middle
    assert!(!filter.is_allowed("g\nit")); // Newline
    assert!(!filter.is_allowed("g\tit")); // Tab
}

/// Test that prefix/suffix matching cannot bypass the filter
#[test]
fn test_cannot_bypass_filter_with_prefix_suffix() {
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("git"));
    assert!(!filter.is_allowed("git_secret")); // Suffix
    assert!(!filter.is_allowed("git2")); // Suffix variation
    assert!(!filter.is_allowed("my_git")); // Prefix
    assert!(!filter.is_allowed("xgit")); // Prefix char
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
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    assert!(!filter.is_allowed("")); // Empty string doesn't match
}

/// Test that unicode characters don't bypass filters
#[test]
fn test_unicode_characters() {
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("git"));
    assert!(!filter.is_allowed("git\u{200B}")); // Zero-width space
    assert!(!filter.is_allowed("g\u{200B}it")); // Zero-width space in middle
    assert!(!filter.is_allowed("gít")); // Accented character
}

/// Test that URL encoding doesn't bypass filters
#[test]
fn test_url_encoded_characters() {
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("git"));
    assert!(!filter.is_allowed("g%69t")); // %69 is 'i'
    assert!(!filter.is_allowed("git%00")); // %00 is null
}

/// Test that regex injection cannot bypass filters
#[test]
fn test_regex_injection_attempts() {
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("git"));
    // These tool names contain regex metacharacters but should not match
    assert!(!filter.is_allowed("git|shell_execute"));
    assert!(!filter.is_allowed("git.*"));
    assert!(!filter.is_allowed("(git)"));
    assert!(!filter.is_allowed("[git]"));
}

/// Test that deny patterns are enforced when no allow patterns exist
#[test]
fn test_deny_patterns_enforced() {
    let filter = ToolFilter::new(
        vec![], // Empty allow list means allow all except denied
        vec!["^shell".to_string(), "^kanban$".to_string()],
    )
    .unwrap();

    // Denied tools
    assert!(!filter.is_allowed("shell"));
    assert!(!filter.is_allowed("shell_kill"));
    assert!(!filter.is_allowed("kanban"));

    // Allowed tools
    assert!(filter.is_allowed("git"));
    assert!(filter.is_allowed("web"));

    // Variations - these should still be blocked or allowed based on pattern matching
    assert!(filter.is_allowed("Shell_execute")); // Case variation doesn't match deny pattern, and no allow patterns, so allowed
    assert!(!filter.is_allowed("shell_execute ")); // Trailing space still matches ^shell pattern
}

/// Test combined allow and deny with bypass attempts
#[test]
fn test_combined_filters_security() {
    let filter = ToolFilter::new(
        vec!["^git$".to_string()],  // Allow git
        vec!["^shell".to_string()], // Deny shell tools
    )
    .unwrap();

    // git matches allow pattern, so it's allowed
    assert!(filter.is_allowed("git"));

    // Variations don't match the allow pattern ^git$, so they're denied
    assert!(!filter.is_allowed("git2")); // Suffix - doesn't match ^git$
    assert!(!filter.is_allowed("Git")); // Case - doesn't match ^git$ (case sensitive)
    assert!(!filter.is_allowed("shell")); // Not in allow list
}

/// Test that long tool names are handled correctly
#[test]
fn test_very_long_tool_names() {
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    let long_name = "a".repeat(10000);
    assert!(!filter.is_allowed(&long_name));

    let long_prefix = format!("{}git", "x".repeat(1000));
    assert!(!filter.is_allowed(&long_prefix));
}

/// Test that control characters don't bypass filters
#[test]
fn test_control_characters() {
    let filter = ToolFilter::new(vec!["^git$".to_string()], vec![]).unwrap();

    assert!(filter.is_allowed("git"));
    assert!(!filter.is_allowed("git\x00")); // Null byte
    assert!(!filter.is_allowed("g\x01it")); // SOH
    assert!(!filter.is_allowed("g\x1Bit")); // ESC
}
