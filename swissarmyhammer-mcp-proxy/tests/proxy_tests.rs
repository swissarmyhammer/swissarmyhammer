use swissarmyhammer_mcp_proxy::ToolFilter;

#[test]
fn test_filter_logic() {
    let filter = ToolFilter::new(vec!["^files_.*".to_string()], vec![]).unwrap();

    // Test the filter logic directly
    assert!(filter.is_allowed("files_read"));
    assert!(filter.is_allowed("files_write"));
    assert!(!filter.is_allowed("shell_execute"));
    assert!(!filter.is_allowed("web_fetch"));
}

#[test]
fn test_filter_allow_precedence() {
    let filter = ToolFilter::new(
        vec!["^files_.*".to_string()],
        vec!["^files_write$".to_string()],
    )
    .unwrap();

    // files_write matches both allow and deny, but allow wins
    assert!(filter.is_allowed("files_write"));
    assert!(filter.is_allowed("files_read"));
}

#[test]
fn test_filter_deny_blocks_tools() {
    let filter = ToolFilter::new(
        vec![], // Empty allow list = allow all
        vec!["^shell_.*".to_string()],
    )
    .unwrap();

    assert!(filter.is_allowed("files_read"));
    assert!(!filter.is_allowed("shell_execute"));
    assert!(!filter.is_allowed("shell_kill"));
}

#[test]
fn test_filter_whitelist_mode() {
    let filter = ToolFilter::new(
        vec!["^files_read$".to_string(), "^files_grep$".to_string()],
        vec![],
    )
    .unwrap();

    assert!(filter.is_allowed("files_read"));
    assert!(filter.is_allowed("files_grep"));
    assert!(!filter.is_allowed("files_write")); // Not in whitelist
    assert!(!filter.is_allowed("shell_execute")); // Not in whitelist
}
