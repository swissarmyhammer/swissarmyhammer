use swissarmyhammer_mcp_proxy::ToolFilter;

#[test]
fn test_filter_logic() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec![]).unwrap();

    // Test the filter logic directly
    assert!(filter.is_allowed("files"));
    assert!(!filter.is_allowed("shell_execute"));
    assert!(!filter.is_allowed("web"));
    assert!(!filter.is_allowed("kanban"));
}

#[test]
fn test_filter_allow_precedence() {
    let filter = ToolFilter::new(vec!["^files$".to_string()], vec!["^files$".to_string()]).unwrap();

    // files matches both allow and deny, but allow wins
    assert!(filter.is_allowed("files"));
}

#[test]
fn test_filter_deny_blocks_tools() {
    let filter = ToolFilter::new(
        vec![], // Empty allow list = allow all
        vec!["^shell_.*".to_string()],
    )
    .unwrap();

    assert!(filter.is_allowed("files"));
    assert!(!filter.is_allowed("shell_execute"));
    assert!(!filter.is_allowed("shell_kill"));
}

#[test]
fn test_filter_whitelist_mode() {
    let filter = ToolFilter::new(
        vec!["^files$".to_string(), "^treesitter_search$".to_string()],
        vec![],
    )
    .unwrap();

    assert!(filter.is_allowed("files"));
    assert!(filter.is_allowed("treesitter_search"));
    assert!(!filter.is_allowed("kanban")); // Not in whitelist
    assert!(!filter.is_allowed("shell_execute")); // Not in whitelist
}
