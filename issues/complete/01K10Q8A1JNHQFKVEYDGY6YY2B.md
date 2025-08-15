```
 cargo run search index **/*.rs
   Compiling ring v0.17.14
   Compiling rustls-webpki v0.103.4
   Compiling rustls v0.23.29
   Compiling ureq v2.12.1
   Compiling hf-hub v0.3.2
   Compiling swissarmyhammer v0.1.0 (/Users/wballard/github/swissarmyhammer/swissarmyhammer)
   Compiling swissarmyhammer-cli v0.1.0 (/Users/wballard/github/swissarmyhammer/swissarmyhammer-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.49s
     Running `target/debug/swissarmyhammer search index benches/benchmarks.rs benches/issue_performance.rs benches/memo_benchmarks.rs swissarmyhammer-cli/src/bin/sah.rs swissarmyhammer-cli/src/cli.rs swissarmyhammer-cli/src/completions.rs swissarmyhammer-cli/src/doctor/checks.rs swissarmyhammer-cli/src/doctor/mod.rs swissarmyhammer-cli/src/doctor/types.rs swissarmyhammer-cli/src/doctor/utils.rs swissarmyhammer-cli/src/error.rs swissarmyhammer-cli/src/exit_codes.rs swissarmyhammer-cli/src/flow.rs swissarmyhammer-cli/src/issue.rs swissarmyhammer-cli/src/lib.rs swissarmyhammer-cli/src/list.rs swissarmyhammer-cli/src/logging.rs swissarmyhammer-cli/src/main.rs swissarmyhammer-cli/src/memo.rs swissarmyhammer-cli/src/prompt.rs swissarmyhammer-cli/src/search.rs swissarmyhammer-cli/src/signal_handler.rs swissarmyhammer-cli/src/test.rs swissarmyhammer-cli/src/validate.rs swissarmyhammer-cli/tests/binary_aliases_test.rs swissarmyhammer-cli/tests/cli_integration_test.rs swissarmyhammer-cli/tests/mcp_e2e_tests.rs swissarmyhammer-cli/tests/mcp_integration_test.rs swissarmyhammer-cli/tests/mcp_logging_test.rs swissarmyhammer-cli/tests/mcp_mock_integration_tests.rs swissarmyhammer-cli/tests/mcp_notification_simple_test.rs swissarmyhammer-cli/tests/mcp_partial_e2e_test.rs swissarmyhammer-cli/tests/mcp_performance_tests.rs swissarmyhammer-cli/tests/mcp_server_shutdown_test.rs swissarmyhammer-cli/tests/memo_cli_tests.rs swissarmyhammer-cli/tests/search_cli_test.rs swissarmyhammer-cli/tests/test_builtin_validation.rs swissarmyhammer-cli/tests/test_doc_examples.rs swissarmyhammer-cli/tests/test_example_actions_workflow.rs swissarmyhammer-cli/tests/test_set_variables.rs swissarmyhammer-cli/tests/test_sub_workflow_integration.rs swissarmyhammer-cli/tests/test_utils.rs swissarmyhammer/build.rs swissarmyhammer/examples/async_and_mcp.rs swissarmyhammer/examples/basic_usage.rs swissarmyhammer/examples/custom_templates.rs swissarmyhammer/examples/debug_issue_186_listing.rs swissarmyhammer/examples/debug_parse_000186.rs swissarmyhammer/examples/memoranda_usage.rs swissarmyhammer/examples/search_example.rs swissarmyhammer/src/common/abort_handler.rs swissarmyhammer/src/common/env_loader.rs swissarmyhammer/src/common/error_context.rs swissarmyhammer/src/common/file_types.rs swissarmyhammer/src/common/mcp_errors.rs swissarmyhammer/src/common/mod.rs swissarmyhammer/src/common/rate_limiter.rs swissarmyhammer/src/common/validation_builders.rs swissarmyhammer/src/config.rs swissarmyhammer/src/directory_utils.rs swissarmyhammer/src/error.rs swissarmyhammer/src/file_loader.rs swissarmyhammer/src/file_watcher.rs swissarmyhammer/src/fs_utils.rs swissarmyhammer/src/git.rs swissarmyhammer/src/issues/filesystem.rs swissarmyhammer/src/issues/instrumented_storage.rs swissarmyhammer/src/issues/metrics.rs swissarmyhammer/src/issues/mod.rs swissarmyhammer/src/issues/utils.rs swissarmyhammer/src/lib.rs swissarmyhammer/src/mcp.rs swissarmyhammer/src/mcp/error_handling.rs swissarmyhammer/src/mcp/file_watcher.rs swissarmyhammer/src/mcp/memo_types.rs swissarmyhammer/src/mcp/responses.rs swissarmyhammer/src/mcp/shared_utils.rs swissarmyhammer/src/mcp/tool_handlers.rs swissarmyhammer/src/mcp/types.rs swissarmyhammer/src/mcp/utils.rs swissarmyhammer/src/memoranda/advanced_search.rs swissarmyhammer/src/memoranda/mock_storage.rs swissarmyhammer/src/memoranda/mod.rs swissarmyhammer/src/memoranda/storage_markdown_tests.rs swissarmyhammer/src/memoranda/storage.rs swissarmyhammer/src/plugins.rs swissarmyhammer/src/prompt_filter.rs swissarmyhammer/src/prompt_resolver.rs swissarmyhammer/src/prompts.rs swissarmyhammer/src/search_advanced.rs swissarmyhammer/src/search.rs swissarmyhammer/src/security.rs swissarmyhammer/src/semantic/embedding.rs swissarmyhammer/src/semantic/indexer.rs swissarmyhammer/src/semantic/mod.rs swissarmyhammer/src/semantic/parser.rs swissarmyhammer/src/semantic/searcher.rs swissarmyhammer/src/semantic/storage.rs swissarmyhammer/src/semantic/tests.rs swissarmyhammer/src/semantic/types.rs swissarmyhammer/src/semantic/utils.rs swissarmyhammer/src/storage.rs swissarmyhammer/src/template.rs swissarmyhammer/src/test_utils.rs swissarmyhammer/src/validation/mod.rs swissarmyhammer/src/workflow/action_parser.rs swissarmyhammer/src/workflow/actions_tests/action_parsing_tests.rs swissarmyhammer/src/workflow/actions_tests/claude_output_formatting_tests.rs swissarmyhammer/src/workflow/actions_tests/claude_retry_tests.rs swissarmyhammer/src/workflow/actions_tests/common.rs swissarmyhammer/src/workflow/actions_tests/concurrent_action_tests.rs swissarmyhammer/src/workflow/actions_tests/error_handling_tests.rs swissarmyhammer/src/workflow/actions_tests/integration_tests.rs swissarmyhammer/src/workflow/actions_tests/log_action_tests.rs swissarmyhammer/src/workflow/actions_tests/mod.rs swissarmyhammer/src/workflow/actions_tests/prompt_action_tests.rs swissarmyhammer/src/workflow/actions_tests/resource_cleanup_tests.rs swissarmyhammer/src/workflow/actions_tests/simple_state_pollution_test.rs swissarmyhammer/src/workflow/actions_tests/sub_workflow_action_tests.rs swissarmyhammer/src/workflow/actions_tests/sub_workflow_state_pollution_tests.rs swissarmyhammer/src/workflow/actions_tests/wait_action_tests.rs swissarmyhammer/src/workflow/actions.rs swissarmyhammer/src/workflow/cache.rs swissarmyhammer/src/workflow/definition.rs swissarmyhammer/src/workflow/error_utils.rs swissarmyhammer/src/workflow/examples_tests.rs swissarmyhammer/src/workflow/executor/core.rs swissarmyhammer/src/workflow/executor/fork_join.rs swissarmyhammer/src/workflow/executor/mod.rs swissarmyhammer/src/workflow/executor/tests.rs swissarmyhammer/src/workflow/executor/validation.rs swissarmyhammer/src/workflow/graph_tests.rs swissarmyhammer/src/workflow/graph.rs swissarmyhammer/src/workflow/metrics_tests.rs swissarmyhammer/src/workflow/metrics.rs swissarmyhammer/src/workflow/mod.rs swissarmyhammer/src/workflow/parser.rs swissarmyhammer/src/workflow/run.rs swissarmyhammer/src/workflow/state.rs swissarmyhammer/src/workflow/storage.rs swissarmyhammer/src/workflow/test_helpers.rs swissarmyhammer/src/workflow/test_liquid_rendering.rs swissarmyhammer/src/workflow/transition_key.rs swissarmyhammer/src/workflow/transition.rs swissarmyhammer/src/workflow/visualization_tests.rs swissarmyhammer/src/workflow/visualization.rs swissarmyhammer/tests/abort_error_integration_tests.rs swissarmyhammer/tests/abort_error_pattern_tests.rs swissarmyhammer/tests/debug_issue_completion.rs swissarmyhammer/tests/integration_tests.rs swissarmyhammer/tests/issue_all_complete_edge_cases.rs swissarmyhammer/tests/issue_completion_fix_verification.rs swissarmyhammer/tests/issue_completion_path_bug_test.rs swissarmyhammer/tests/mcp_issue_integration_tests.rs swissarmyhammer/tests/mcp_memoranda_tests.rs swissarmyhammer/tests/path_completion_precision_test.rs swissarmyhammer/tests/plugin_tests.rs swissarmyhammer/tests/test_home_integration.rs swissarmyhammer/tests/test_partials_issue.rs target/debug/build/cel-parser-1eae62aa70d4bf09/out/cel.rs target/debug/build/cel-parser-50fb9588f82b112a/out/cel.rs target/debug/build/cel-parser-959817028f054e9e/out/cel.rs target/debug/build/cel-parser-bef601495abcbd25/out/cel.rs target/debug/build/crunchy-dc2d3824bebabe93/out/lib.rs target/debug/build/swissarmyhammer-4798d5a1662fc2b2/out/builtin_prompts.rs target/debug/build/swissarmyhammer-4798d5a1662fc2b2/out/builtin_workflows.rs target/debug/build/swissarmyhammer-ebad453526a31e6e/out/builtin_prompts.rs target/debug/build/swissarmyhammer-ebad453526a31e6e/out/builtin_workflows.rs target/debug/build/swissarmyhammer-f46d402fc6d7df0f/out/builtin_prompts.rs target/debug/build/swissarmyhammer-f46d402fc6d7df0f/out/builtin_workflows.rs target/debug/build/typenum-1a754eeb2c228a52/out/tests.rs target/debug/build/typenum-ca153e5e9973794f/out/tests.rs target/release/build/cel-parser-389803ce62ba8268/out/cel.rs target/release/build/crunchy-1536988b990b8f5d/out/lib.rs target/release/build/crunchy-3e1e8ab0e4bd4ef8/out/lib.rs target/release/build/swissarmyhammer-2e2c41afc5a9e131/out/builtin_prompts.rs target/release/build/swissarmyhammer-2e2c41afc5a9e131/out/builtin_workflows.rs target/release/build/typenum-1a533418ceaebf73/out/tests.rs target/release/build/typenum-6d2d66b249ede1f8/out/tests.rs tests/cli_integration.rs tests/prompt_library_test.rs tests/property_tests.rs tests/test_integration.rs`
error: unexpected argument 'benches/issue_performance.rs' found
```

That folder name 'search' and 'index' should not be in the glob match for starters.

And -- we cannot index from the cli.



## Proposed Solution

The issue is that the CLI `search index` command expects a single `glob: String` parameter, but when the shell expands `**/*.rs`, it becomes multiple individual file arguments. The current CLI definition in `swissarmyhammer-cli/src/cli.rs:815-836` has:

```rust
Index {
    /// Glob pattern for files to index
    glob: String,  // This only accepts ONE argument
    /// Force re-indexing of all files
    #[arg(short, long)]
    force: bool,
},
```

But the command `cargo run search index **/*.rs` gets shell-expanded to:
`cargo run search index file1.rs file2.rs file3.rs ...` (many arguments)

### Fix Steps:

1. **Change CLI definition** - Update `SearchCommands::Index` to accept multiple file patterns:
   ```rust
   Index {
       /// Glob patterns or files to index
       patterns: Vec<String>,  // Accept multiple patterns/files
       /// Force re-indexing of all files
       #[arg(short, long)]
       force: bool,
   },
   ```

2. **Update search.rs** - Modify `run_semantic_index` to handle multiple patterns:
   - Process each pattern/file in the list
   - Support both glob patterns (when quoted) and expanded file lists
   - Aggregate indexing results from all patterns

3. **Update CLI help text** - Clarify that both single patterns and multiple files are supported

4. **Add tests** - Verify both usage patterns work:
   - `swissarmyhammer search index "**/*.rs"` (quoted glob)
   - `swissarmyhammer search index file1.rs file2.rs file3.rs` (expanded files)

This solution maintains backwards compatibility while fixing the shell expansion issue.