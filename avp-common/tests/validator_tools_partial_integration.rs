//! Integration tests asserting that the validator-tools partial used by
//! every rule prompt advertises **only** tools that the validator MCP
//! server actually exposes.
//!
//! ## Why
//!
//! The `.system/rule` and `.system/validator` prompt templates render an
//! "Available Tools" section pulled from the shared
//! `_partials/validator-tools` partial. That section tells the validator
//! agent which MCP tools it may call. If the partial drifts from the
//! validator MCP server's `tools/list` — either advertising a tool that
//! isn't exposed (e.g. `bash`, `edit`) or omitting one that is — the
//! agent either tries to call something it can't or fails to use a tool
//! it should.
//!
//! These tests catch that drift in two complementary ways:
//!
//! 1. **Static check**: parse the partial's "Available Tools" section,
//!    extract the top-level tool identifiers, and assert each one is in
//!    the expected validator allowlist. Forbids leftover claude-only
//!    names like `bash`, `write_file`, `edit_file`.
//!
//! 2. **Runtime cross-check**: spin up the validator MCP server, call
//!    `tools/list`, and assert that every top-level tool mentioned in the
//!    partial appears in the runtime list, and vice versa.
//!
//! The runtime list is the authority — the static allowlist exists only
//! to keep the test self-contained when run without network or filesystem
//! permission to bind a port.

use std::collections::BTreeSet;
use std::path::PathBuf;

use swissarmyhammer_tools::mcp::test_utils::create_test_client;
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server_with_options, McpServerMode};

/// Path to the workspace's `builtin/` directory, computed from the crate
/// manifest. Used to read prompt and partial source files for static
/// assertions.
fn builtin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("builtin")
}

/// Read a file from the source tree under `builtin/`.
///
/// Tests use files on disk (rather than the embedded copy compiled into
/// the binary) because they assert on the *source of truth* a developer
/// edits, not on the compiled artefact.
fn read_builtin(rel_path: &str) -> String {
    let path = builtin_dir().join(rel_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

/// Read the validator-tools partial from the source tree.
fn read_validator_tools_partial() -> String {
    read_builtin("_partials/validator-tools.md")
}

/// Extract the set of top-level tool identifiers mentioned in the partial.
///
/// The partial's "Available Tools" section lists each tool as a markdown
/// bullet with a backtick-quoted name as the first non-whitespace token.
/// Only the *top-level* tool names are pulled out — `op:` arguments and
/// inner schema field names (e.g. `path`, `pattern`) are ignored because
/// they are not separate MCP tools.
///
/// Returns the set of tool identifiers that appear as the leading
/// backtick-quoted token on a top-level bullet line in the
/// "Available Tools" section.
fn extract_top_level_tool_names(partial_body: &str) -> BTreeSet<String> {
    let section = extract_available_tools_section(partial_body);
    let mut tools = BTreeSet::new();
    for line in section.lines() {
        // Match top-level bullets only: `- ` at column zero. Indented
        // sub-bullets (e.g. `op:` lists) are ignored on purpose.
        let Some(rest) = line.strip_prefix("- ") else {
            continue;
        };
        // The first backtick-quoted identifier on the line is the tool name.
        let Some(start) = rest.find('`') else {
            continue;
        };
        let after_open = &rest[start + 1..];
        let Some(end) = after_open.find('`') else {
            continue;
        };
        let name = &after_open[..end];
        // Identifier sanity check — tool names are lowercase + underscores.
        if name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
            && !name.is_empty()
        {
            tools.insert(name.to_string());
        }
    }
    tools
}

/// Slice out the body of the `## Available Tools` section.
///
/// The section runs from its `## Available Tools` heading to the next
/// `## ` heading (or end of file). The heading line itself is excluded.
fn extract_available_tools_section(body: &str) -> String {
    let marker = "## Available Tools";
    let Some(start) = body.find(marker) else {
        panic!(
            "validator-tools partial is missing the '## Available Tools' heading.\n\
             Body was:\n{}",
            body
        );
    };
    // Skip past the heading line.
    let after_heading = &body[start + marker.len()..];
    let after_newline = after_heading.find('\n').map(|n| n + 1).unwrap_or(0);
    let body_after = &after_heading[after_newline..];
    // Find the next top-level heading, if any.
    let next = body_after
        .find("\n## ")
        .map(|n| n + 1)
        .unwrap_or(body_after.len());
    body_after[..next].to_string()
}

/// The validator MCP server's `tools/list` is the authority for what tools
/// are reachable. This is the same allowlist asserted in
/// `test_validator_endpoint_lists_only_validator_tools` in
/// `swissarmyhammer-tools` — kept in sync intentionally so a drift on
/// either side trips a test.
fn expected_validator_tools() -> BTreeSet<String> {
    ["read_file", "glob_files", "grep_files", "code_context"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Static check: every top-level tool name mentioned in the partial must
/// be in the expected validator allowlist, and vice versa. No drift, no
/// leftover claude-only names like `bash` or `edit_file`.
#[test]
fn test_validator_tools_partial_matches_expected_allowlist() {
    let partial = read_validator_tools_partial();
    let mentioned = extract_top_level_tool_names(&partial);
    let expected = expected_validator_tools();

    assert_eq!(
        mentioned, expected,
        "Tool names mentioned at the top level of the validator-tools \
         partial must exactly match the validator MCP server's allowlist.\n\
         Mentioned: {:?}\n\
         Expected:  {:?}",
        mentioned, expected
    );
}

/// Static check: the partial must not advertise any tool the validator
/// cannot actually call. This is the negative half of
/// `test_validator_tools_partial_matches_expected_allowlist` — it stays
/// even if the allowlist grows, because these names are forbidden
/// regardless.
#[test]
fn test_validator_tools_partial_has_no_forbidden_tools() {
    let partial = read_validator_tools_partial();
    let mentioned = extract_top_level_tool_names(&partial);

    // Tools that have repeatedly leaked into validator prompts and must
    // never reappear. Mirrors the defense-in-depth list in
    // `test_validator_endpoint_lists_only_validator_tools`.
    let forbidden = [
        "bash",
        "shell",
        "git",
        "kanban",
        "web",
        "questions",
        "ralph",
        "skill",
        "agent",
        "write_file",
        "edit_file",
        "files", // unified op-dispatched tool — not exposed on validator route
    ];

    for name in forbidden {
        assert!(
            !mentioned.contains(name),
            "validator-tools partial must NOT advertise '{}' — got: {:?}",
            name,
            mentioned
        );
    }
}

/// Static check: the `.system/rule` prompt template must include the
/// validator-tools partial. If someone deletes the include or renames
/// the partial, every rule prompt loses its "Available Tools" section
/// silently — this test catches that.
#[test]
fn test_system_rule_template_includes_validator_tools_partial() {
    let body = read_builtin("prompts/.system/rule.md");
    assert!(
        body.contains(r#"{% include "_partials/validator-tools" %}"#),
        ".system/rule.md must include the validator-tools partial.\n\
         Body was:\n{}",
        body
    );
}

/// Static check: the `.system/validator` prompt template must include
/// the same partial. The validator template is used for the standalone
/// validator entry point alongside the rule template; both must agree.
#[test]
fn test_system_validator_template_includes_validator_tools_partial() {
    let body = read_builtin("prompts/.system/validator.md");
    assert!(
        body.contains(r#"{% include "_partials/validator-tools" %}"#),
        ".system/validator.md must include the validator-tools partial.\n\
         Body was:\n{}",
        body
    );
}

/// Static check: no rule body may re-declare its own "## Available Tools"
/// section. The partial is the single source of truth — if a rule body
/// also has its own section, the rendered prompt will contain two
/// conflicting tool lists.
#[test]
fn test_no_rule_body_redeclares_available_tools_section() {
    let validators_dir = builtin_dir().join("validators");
    let mut offenders = Vec::new();

    for entry in walkdir::WalkDir::new(&validators_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.contains("## Available Tools") {
            offenders.push(path.display().to_string());
        }
    }

    assert!(
        offenders.is_empty(),
        "No rule body or VALIDATOR.md may declare its own '## Available \
         Tools' section — that's the partial's job. Offenders:\n{}",
        offenders.join("\n")
    );
}

/// Static check: no rule body or VALIDATOR.md may use claude-only
/// imperative-rewrite language. Validators judge, they don't fix —
/// telling them to "use the bash tool" or "edit the file" advertises a
/// capability they don't have on the validator MCP route.
#[test]
fn test_no_rule_body_advertises_claude_only_tools() {
    let validators_dir = builtin_dir().join("validators");
    let forbidden_phrases = [
        "use the bash tool",
        "use the Bash tool",
        "edit the file to",
        "write a corrected",
        "run the test",
        "run the tests",
        "check git history",
        "check the git history",
    ];

    let mut offenders: Vec<String> = Vec::new();

    for entry in walkdir::WalkDir::new(&validators_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for phrase in forbidden_phrases {
            if content.contains(phrase) {
                offenders.push(format!(
                    "{} contains forbidden phrase: {:?}",
                    path.display(),
                    phrase
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "No rule body may instruct the validator to take actions that \
         require tools it doesn't have. Offenders:\n{}",
        offenders.join("\n")
    );
}

/// Runtime cross-check: the validator MCP server's `tools/list` must
/// match the partial's top-level tool names. If someone adds a tool to
/// the validator route without updating the partial (or vice versa),
/// this test fails at the same boundary the validator agent talks to.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_validator_tools_partial_matches_runtime_tools_list() {
    // Bind an in-process HTTP MCP server in a clean tempdir so its index
    // does not walk the host monorepo.
    let temp = tempfile::TempDir::new().unwrap();
    let mut server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(temp.path().to_path_buf()),
        // agent_mode is irrelevant for the validator route — it filters
        // by `is_validator_tool()`, not `is_agent_tool()`.
        true,
    )
    .await
    .expect("Failed to start in-process MCP server");

    let port = server.port().expect("HTTP server must report a bound port");
    let validator_url = format!("http://127.0.0.1:{}/mcp/validator", port);

    let client = create_test_client(&validator_url).await;
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("tools/list against /mcp/validator must succeed");

    let runtime: BTreeSet<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();

    let partial = read_validator_tools_partial();
    let mentioned = extract_top_level_tool_names(&partial);

    assert_eq!(
        mentioned, runtime,
        "Top-level tool names in the validator-tools partial must match \
         the validator MCP server's `tools/list` exactly.\n\
         Partial mentions: {:?}\n\
         Runtime exposes: {:?}",
        mentioned, runtime
    );

    client.cancel().await.unwrap();
    server.shutdown().await.unwrap();
}

// ============================================================================
// Helper-function unit tests
// ============================================================================

#[cfg(test)]
mod helper_tests {
    use super::*;

    /// The extractor must slice from `## Available Tools` to the next `## `
    /// heading and drop the heading itself.
    #[test]
    fn test_extract_available_tools_section_slices_correctly() {
        let body =
            "intro\n\n## Available Tools\n\n- `read_file` — desc\n\n## Next Section\n\nbody\n";
        let section = extract_available_tools_section(body);
        assert!(
            section.contains("- `read_file`"),
            "Section should include the bullet, got: {:?}",
            section
        );
        assert!(
            !section.contains("Next Section"),
            "Section should stop at the next heading, got: {:?}",
            section
        );
    }

    /// Indented sub-bullets (op: lists) must not be picked up as top-level
    /// tools. Their backticks are inner schema details, not tool names.
    #[test]
    fn test_extract_top_level_tool_names_skips_indented_bullets() {
        let body = r#"## Available Tools

- `read_file` — read a file
- `code_context` — symbol intel
  - `"get symbol"` — sub-op
  - `"search symbol"` — sub-op
"#;
        let names = extract_top_level_tool_names(body);
        let expected: BTreeSet<String> = ["read_file", "code_context"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(names, expected);
    }

    /// Empty section (no bullets) yields an empty set, not a panic.
    #[test]
    fn test_extract_top_level_tool_names_empty_section() {
        let body = "## Available Tools\n\nProse only, no bullets.\n";
        let names = extract_top_level_tool_names(body);
        assert!(names.is_empty());
    }
}
