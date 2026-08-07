//! Integration tests for the operation-based `review` tool.
//!
//! These drive the **registered** tool (real registry, real loader, real engine)
//! end to end:
//!
//! - registration: every op is advertised;
//! - `list validators`: seeded user + project layers surface with the right
//!   `source_layer` and their declared `probes`;
//! - `check validators`: a validator declaring an unknown probe is reported;
//! - `review working`: a temp git repo with a planted duplicate, a seeded
//!   on-disk code_context index, and a scripted ACP agent → a `ReviewReport`
//!   flagging the issue at the right severity.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_client_protocol::DynConnectTo;
use serde_json::json;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
// The ONE shared review test seam, consumed via the validators crate's
// `test-support` feature instead of per-file copies: the scripted ACP agent
// harness, the throwaway git repo, the on-disk index builder + row seeders, and
// the shared embedding dimension.
use swissarmyhammer_validators::review::test_support::{
    body as dup_body, dup_emb, engine_matched_validator_names, on_disk_index_conn, seed_chunk,
    write_tool_rule_ruleset, ScriptedAdapter, ScriptedAgent, ScriptedReply, TestRepo, DIM,
};
use tokio::sync::broadcast;

use super::review_op::{AgentFactory, AgentHandle, EmbedderFactory};
use super::*;
use crate::mcp::tool_handlers::ToolHandlers;
use crate::mcp::tool_registry::{ToolContext, ToolRegistry};

/// Capacity of the per-connection backend broadcast each scripted agent streams
/// onto. A single review run here emits few notifications, well under capacity,
/// so the subscriber never lags a chunk away.
///
/// This (and [`extract_text`] / [`scripted_factory`] / [`mock_embedder_factory`])
/// deliberately mirror the integration-test copies in
/// `tests/integration/review_fixture.rs`. The two cannot share a helper: this is a
/// `#[cfg(test)]` unit-test module and that one is an integration-test module —
/// separate compilation units that cannot import each other. The factories return
/// tools-crate-local types (`AgentFactory`/`EmbedderFactory`), so they cannot move
/// to the cross-crate `test_support` seam, and this crate forbids adding a
/// `test-support` feature. So the small per-unit copies stand by design; only the
/// buffer capacity is named.
const SCRIPTED_AGENT_NOTIFY_BUFFER_SIZE: usize = 64;

// ---------------------------------------------------------------------------
// registration
// ---------------------------------------------------------------------------

#[test]
fn review_tool_is_registered_with_its_ops() {
    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);

    let tool = registry.get_tool("review").expect("review tool registered");
    let op_strings: Vec<String> = tool.operations().iter().map(|o| o.op_string()).collect();
    for expected in [
        "review file",
        "review working",
        "review sha",
        "list validators",
        "dump validators",
        "get validator",
        "check validators",
    ] {
        assert!(
            op_strings.iter().any(|s| s == expected),
            "op `{expected}` must be advertised, got: {op_strings:?}"
        );
    }
}

/// [`DEFAULT_OP`] (the op dispatched when a call omits `op`) must stay the
/// `review working` operation's canonical op string, keeping the constant tied
/// to the [`REVIEW_OPERATIONS`] source of truth.
#[test]
fn default_op_is_the_advertised_review_working_op_string() {
    assert_eq!(DEFAULT_OP, REVIEW_WORKING.op_string());
}

/// The manual `Debug` impl renders the trait-object factory fields by
/// presence/absence (closures are unprintable) alongside the plain fields.
#[test]
fn review_tool_debug_summarizes_factory_presence() {
    let bare = format!("{:?}", ReviewTool::new());
    assert!(bare.contains("agent_factory: None"), "{bare}");
    assert!(bare.contains("embedder_factory: None"), "{bare}");
    assert!(bare.contains("concurrency: None"), "{bare}");

    let factory: AgentFactory = Arc::new(|| Box::pin(async { Err("unused".to_string()) }));
    let wired = format!(
        "{:?}",
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory())
            .with_concurrency(Some(3))
    );
    assert!(wired.contains("agent_factory: Some"), "{wired}");
    assert!(wired.contains("embedder_factory: Some"), "{wired}");
    assert!(wired.contains("concurrency: Some(3)"), "{wired}");
}

// ---------------------------------------------------------------------------
// wire / full schema split
// ---------------------------------------------------------------------------

/// The FULL schema carries the heavy CLI-generation keys; the WIRE schema drops
/// every one of them. Mirrors the post-`^4ez75dw` pattern used by
/// `web/mod.rs` and `shell/mod.rs`.
#[test]
fn review_full_schema_carries_heavy_keys_wire_omits_them() {
    let tool = ReviewTool::new();

    // Full (in-process CLI) surface: the per-op detail the command tree needs.
    let full = tool.schema_full();
    assert!(
        full["x-op-signatures"].is_object(),
        "full schema x-op-signatures must be an object"
    );
    assert!(
        full["x-operation-schemas"].is_array(),
        "full schema x-operation-schemas must be an array"
    );

    // Wire (model-facing) surface: the full-only keys must be absent.
    let wire = tool.schema();
    assert!(
        wire.get("x-op-signatures").is_none(),
        "wire schema must omit x-op-signatures"
    );
    assert!(
        wire.get("x-operation-schemas").is_none(),
        "wire schema must omit x-operation-schemas"
    );

    // And mechanically: the wire surface drops every WIRE_DROPPED_KEYS key.
    let wire_obj = wire.as_object().unwrap();
    for key in swissarmyhammer_operations::WIRE_DROPPED_KEYS {
        assert!(!wire_obj.contains_key(key), "wire schema must omit {key:?}");
    }
}

/// The wire schema carries no per-op parameter metadata (only the `op` enum),
/// so the model-facing description is the ONLY channel that names each op's
/// arguments — it is what tells a model that `review sha` takes `sha`. Every
/// required parameter must therefore be named, backtick-quoted, in
/// `description.md`.
#[test]
fn review_description_names_every_required_param() {
    let missing = swissarmyhammer_operations::required_params_missing_from_description(
        ReviewTool::new().description(),
        &REVIEW_OPERATIONS,
    );
    assert!(
        missing.is_empty(),
        "model-facing description omits required params: {missing:?}"
    );
}

// ---------------------------------------------------------------------------
// CLI command tree coverage
// ---------------------------------------------------------------------------

/// Every `REVIEW_OPERATIONS` op must surface as a `noun → verb` pair in the
/// command tree the shared generator builds from review's FULL schema. The
/// expected set is DERIVED from the canonical op table, so adding an op is
/// covered mechanically without editing this test.
#[test]
fn review_command_tree_covers_all_operations() {
    use std::collections::HashSet;
    use swissarmyhammer_operations::cli_gen::build_commands_from_schema;
    use swissarmyhammer_operations::cli_gen::test_support::collect_verb_noun_pairs;

    let schema = ReviewTool::new().schema_full();
    let commands = build_commands_from_schema(&schema);
    let generated = collect_verb_noun_pairs(&commands);

    let expected: HashSet<String> = REVIEW_OPERATIONS.iter().map(|op| op.op_string()).collect();
    assert_eq!(
        generated, expected,
        "generated command tree and REVIEW_OPERATIONS diverge"
    );

    // Spot-check the documented verbs still resolve.
    for op in [
        "review file",
        "review working",
        "review sha",
        "list validators",
        "dump validators",
        "get validator",
        "check validators",
    ] {
        assert!(
            generated.contains(op),
            "verb `{op}` missing from review command tree: {generated:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

/// The shared fixture rule body a RuleSet carries when a test does not need a
/// distinct one.
const DEFAULT_RULE_BODY: &str = "Check the code.";

/// Write a minimal RuleSet (VALIDATOR.md + one rule) under `base/<name>/`.
///
/// `match_yaml` is the indented YAML body of the frontmatter `match:` key, so
/// ONE writer backs every match shape the fixtures need — a file glob alone, a
/// glob plus a tool, a glob plus a workspace project type.
fn write_ruleset_with_match(
    base: &Path,
    name: &str,
    match_yaml: &str,
    probes: &[&str],
    body: &str,
) {
    let dir = base.join(name);
    std::fs::create_dir_all(dir.join("rules")).unwrap();
    let probes_yaml = if probes.is_empty() {
        String::new()
    } else {
        let items: Vec<String> = probes.iter().map(|p| format!("  - {p}")).collect();
        format!("probes:\n{}\n", items.join("\n"))
    };
    std::fs::write(
        dir.join("VALIDATOR.md"),
        format!(
            "---\nname: {name}\ndescription: {name} ruleset\nmatch:\n{match_yaml}{probes_yaml}---\n\n# {name}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("rules/check.md"),
        format!("---\nname: check\ndescription: Check\n---\n\n{body}\n"),
    )
    .unwrap();
}

/// The `match:` YAML body selecting one file glob.
fn files_match_yaml(glob: &str) -> String {
    format!("  files:\n    - \"{glob}\"\n")
}

/// Write a minimal RuleSet (VALIDATOR.md + one rule) under `base/<name>/`, with
/// the given file glob and probe list. The single rule carries the shared
/// fixture body [`DEFAULT_RULE_BODY`].
fn write_ruleset(base: &Path, name: &str, glob: &str, probes: &[&str]) {
    write_ruleset_with_rule_body(base, name, glob, probes, DEFAULT_RULE_BODY);
}

/// Write a minimal RuleSet whose single rule carries `body`.
///
/// A distinct body per RuleSet lets the `dump validators` tests count each rule
/// body exactly once in the produced markdown.
fn write_ruleset_with_rule_body(base: &Path, name: &str, glob: &str, probes: &[&str], body: &str) {
    write_ruleset_with_match(base, name, &files_match_yaml(glob), probes, body);
}

/// Write a RuleSet whose match criteria pin it to a TOOL as well as a file glob.
///
/// The review engine matches by changed file with no tool name in context, so it
/// never pairs such a validator with a file — the fixture that proves the tool's
/// `match` filter uses the engine matcher rather than a glob test of its own.
fn write_tool_scoped_ruleset(base: &Path, name: &str, glob: &str, tool: &str) {
    write_ruleset_with_match(
        base,
        name,
        &format!("{}  tools:\n    - {tool}\n", files_match_yaml(glob)),
        &[],
        DEFAULT_RULE_BODY,
    );
}

/// Write a RuleSet whose match criteria pin it to a workspace PROJECT TYPE as
/// well as a file glob — the shape the tool rules use.
///
/// Such a set matches only when the caller resolved the workspace's detected
/// project type keys into the match context.
fn write_project_type_scoped_ruleset(
    base: &Path,
    name: &str,
    glob: &str,
    project_type: &str,
    body: &str,
) {
    write_ruleset_with_match(
        base,
        name,
        &format!(
            "{}  project_types:\n    - {project_type}\n",
            files_match_yaml(glob)
        ),
        &[],
        body,
    );
}

/// Write a malformed RuleSet under `base/<name>/`: a VALIDATOR.md whose
/// frontmatter does not parse (unterminated YAML), so the loader drops it.
fn write_malformed_ruleset(base: &Path, name: &str) {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("VALIDATOR.md"), "---\nmatch: [unterminated\n").unwrap();
}

/// Extract the JSON text body of a tool result.
fn extract_text(result: &rmcp::model::CallToolResult) -> String {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text content"),
    }
}

/// Build a `ToolContext` rooted at `dir`.
async fn context_at(dir: &Path) -> ToolContext {
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(swissarmyhammer_config::ChatModelConfig::default());
    let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = Some(dir.to_path_buf());
    ctx
}

// ---------------------------------------------------------------------------
// list / check validators (pure loader reads)
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial(cwd)]
async fn list_validators_surfaces_user_and_project_layers_with_probes() {
    let home = IsolatedTestEnvironment::new().expect("isolated env");

    // User store: ~/.validators/<name> (resolved via the isolated temp HOME)
    let user_validators = home.home_path().join(".validators");
    write_ruleset(&user_validators, "user-dedup", "*.rs", &["duplicates"]);

    // Project store: <git_root>/.validators/<name>
    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let project_validators = project.path().join(".validators");
    write_ruleset(&project_validators, "project-dead", "*.rs", &["callers"]);
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("list validators"));
    let result = tool.execute(args, &context).await.expect("list validators");
    let body = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = parsed.as_array().expect("list returns an array");

    let find = |name: &str| rows.iter().find(|r| r["name"] == json!(name));

    let user = find("user-dedup").expect("user validator listed");
    assert_eq!(user["source_layer"], json!("user"));
    assert_eq!(user["probes"], json!(["duplicates"]));

    let project_row = find("project-dead").expect("project validator listed");
    assert_eq!(project_row["source_layer"], json!("project"));
    assert_eq!(project_row["probes"], json!(["callers"]));
}

/// Create a throwaway workspace holding one project-layer RuleSet.
///
/// The directory gets a `.git` marker so it reads as a repository root, and a
/// `.validators/<name>` set matching every Rust file. Returns the `TempDir`,
/// which the caller must keep alive for the workspace to exist.
fn workspace_with_project_ruleset(name: &str) -> tempfile::TempDir {
    let workspace = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(workspace.path().join(".git")).unwrap();
    write_ruleset(&workspace.path().join(".validators"), name, "*.rs", &[]);
    workspace
}

/// The project validator layer follows the SESSION working directory, never the
/// process current directory.
///
/// Two workspaces each carry their own `.validators` set. The session names
/// workspace A while the process CWD sits in workspace B, so a `list validators`
/// call must report A's set and never B's.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn list_validators_reads_the_session_workspace_not_the_process_cwd() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let session_workspace = workspace_with_project_ruleset("session-workspace-set");
    let cwd_workspace = workspace_with_project_ruleset("cwd-workspace-set");
    let _cwd = CurrentDirGuard::new(cwd_workspace.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(session_workspace.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("list validators"));
    let result = tool.execute(args, &context).await.expect("list validators");
    let body = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<String> = parsed
        .as_array()
        .expect("list returns an array")
        .iter()
        .map(|row| row["name"].as_str().unwrap().to_string())
        .collect();

    assert!(
        names.contains(&"session-workspace-set".to_string()),
        "the session working directory's project layer must load: {names:?}"
    );
    assert!(
        !names.contains(&"cwd-workspace-set".to_string()),
        "the process current directory's project layer must NOT load: {names:?}"
    );
}

/// The tool-rule `run` script every seeded tool rule in these tests carries.
/// The tests assert on exposure (`tool.run` round-trips through the ops), never
/// on execution, so one fixed script serves every seeding call.
const TOOL_RULE_RUN: &str = "ruff check \"$@\"";

/// `get validator` and a `rules: true` listing expose a tool rule's `tool`
/// block and `supersedes`, and a prompt rule row carries neither key.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn get_validator_exposes_tool_block_and_supersedes() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let project_validators = project.path().join(".validators");
    write_tool_rule_ruleset(&project_validators, "tooled-set", "**/*.py", TOOL_RULE_RUN)
        .expect("write tool ruleset");
    write_ruleset(&project_validators, "prompt-set", "**/*.py", &[]);
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("get validator"));
    args.insert("name".to_string(), json!("tooled-set"));
    let result = tool.execute(args, &context).await.expect("get validator");
    let detail: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    let rule = &detail["rules"][0];
    assert_eq!(rule["name"], json!("docs-tool"));
    assert_eq!(rule["supersedes"], json!("missing-docs"));
    assert_eq!(rule["tool"]["scope"], json!("files"));
    assert_eq!(rule["tool"]["run"], json!(TOOL_RULE_RUN));
    assert_eq!(rule["tool"]["doctor"]["check_command"], json!("which ruff"));

    // A prompt rule carries neither `tool` nor `supersedes`.
    let mut prompt_args = serde_json::Map::new();
    prompt_args.insert("op".to_string(), json!("get validator"));
    prompt_args.insert("name".to_string(), json!("prompt-set"));
    let prompt = tool
        .execute(prompt_args, &context)
        .await
        .expect("get validator");
    let prompt: serde_json::Value = serde_json::from_str(&extract_text(&prompt)).unwrap();
    let prompt_rule = &prompt["rules"][0];
    assert!(prompt_rule.get("tool").is_none(), "{prompt_rule}");
    assert!(prompt_rule.get("supersedes").is_none(), "{prompt_rule}");

    // The `rules: true` listing carries the same tool block.
    let mut list_args = serde_json::Map::new();
    list_args.insert("op".to_string(), json!("list validators"));
    list_args.insert("match".to_string(), json!("pkg/tool.py"));
    list_args.insert("rules".to_string(), json!(true));
    let listed = tool
        .execute(list_args, &context)
        .await
        .expect("list validators");
    let listed: serde_json::Value = serde_json::from_str(&extract_text(&listed)).unwrap();
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == json!("tooled-set"))
        .expect("the tooled set is listed");
    assert_eq!(row["rules"][0]["tool"]["scope"], json!("files"));
    assert_eq!(row["rules"][0]["supersedes"], json!("missing-docs"));
}

/// `dump validators` on a path matched by a tool-rule validator renders the
/// rule's `supersedes` into the markdown, so the dumped rules file shows the
/// same rule relationship the structured API (`get validator`,
/// `list validators`) exposes.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn dump_validators_renders_tool_rule_supersedes() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let project_validators = project.path().join(".validators");
    write_tool_rule_ruleset(&project_validators, "tooled-set", "**/*.py", TOOL_RULE_RUN)
        .expect("write tool ruleset");
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("dump validators"));
    args.insert("paths".to_string(), json!(["pkg/tool.py"]));
    let result = tool.execute(args, &context).await.expect("dump validators");
    let response: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    // Builtin validators match *.py too; the seeded set must be among them.
    let validators = response["validators"].as_array().expect("validators array");
    assert!(validators.contains(&json!("tooled-set")), "{validators:?}");
    let rules_path = response["path"].as_str().expect("rules file path");
    let markdown = std::fs::read_to_string(rules_path).expect("read dumped rules file");
    assert!(markdown.contains("### docs-tool"), "{markdown}");
    assert!(markdown.contains("Supersedes: missing-docs"), "{markdown}");
}

/// The Rust source path the `match` filter targets in the pairing tests. Nothing
/// reads it from disk — validator matching is a pure glob test over the path.
const RUST_MATCH_TARGET: &str = "src/lib.rs";

/// One call answers "what rules will a review enforce on this file?": `match: <a
/// .rs path>` + `rules: true` must return EXACTLY the validators the engine pairs
/// with that path (via its own `match_validators_and_files`), each carrying its
/// rule bodies verbatim — byte-identical to what `get validator` returns.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn list_validators_with_rules_pairs_like_the_engine_and_carries_bodies() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let project_validators = project.path().join(".validators");
    write_ruleset(&project_validators, "rust-rules", "**/*.rs", &[]);
    write_ruleset(&project_validators, "ts-rules", "**/*.ts", &[]);
    // Matches the .rs glob but is pinned to a tool, so the engine never pairs it
    // with a file: listing it would mean the tool matched by its own glob test.
    write_tool_scoped_ruleset(&project_validators, "edit-hook-rules", "**/*.rs", "Edit");
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("list validators"));
    args.insert("match".to_string(), json!(RUST_MATCH_TARGET));
    args.insert("rules".to_string(), json!(true));
    let result = tool.execute(args, &context).await.expect("list validators");
    let body = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = parsed.as_array().expect("list returns an array");

    // The tool's answer IS the engine's pairing for that path — same code path,
    // so the two can never disagree about what a review run will enforce.
    let loader = swissarmyhammer_validators::load_rules(Some(project.path())).expect("load rules");
    let listed: Vec<String> = rows
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        listed,
        engine_matched_validator_names(RUST_MATCH_TARGET, &loader),
        "`list validators` must pair with the file exactly as the engine does: {body}"
    );
    assert!(
        listed.contains(&"rust-rules".to_string()),
        "the Rust-matching validator must be paired: {listed:?}"
    );
    assert!(
        !listed.contains(&"ts-rules".to_string()),
        "a TypeScript-only validator must not be paired with a .rs path: {listed:?}"
    );
    assert!(
        !listed.contains(&"edit-hook-rules".to_string()),
        "a tool-scoped validator the engine never pairs with a file must not be listed: {listed:?}"
    );

    // Every row carries the ruleset's rules verbatim — the same shape and the
    // same bytes `get validator` returns for that name.
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let mut detail_args = serde_json::Map::new();
        detail_args.insert("op".to_string(), json!("get validator"));
        detail_args.insert("name".to_string(), json!(name));
        let detail = tool
            .execute(detail_args, &context)
            .await
            .expect("get validator");
        let detail: serde_json::Value = serde_json::from_str(&extract_text(&detail)).unwrap();
        assert_eq!(
            row["rules"], detail["rules"],
            "`{name}` rules must be the verbatim `get validator` bodies"
        );
    }

    // And those bodies are the real rule text, not empty placeholders.
    let fixture = rows
        .iter()
        .find(|r| r["name"] == json!("rust-rules"))
        .expect("the fixture validator is listed");
    assert_eq!(fixture["rules"][0]["name"], json!("check"));
    assert!(
        fixture["rules"][0]["body"]
            .as_str()
            .unwrap_or_default()
            .contains("Check the code"),
        "rule bodies must be the verbatim markdown: {fixture}"
    );
}

/// An agent is the intended caller of `rules: true`, and agents routinely send
/// the JSON string `"true"` where the schema declares a boolean. The string must
/// return the same bodies the boolean does: dropping it would answer with a
/// success-shaped row carrying `rule_count` and no rules, which reads as "this
/// validator has no rule text" rather than "you sent the wrong type".
#[tokio::test]
#[serial_test::serial(cwd)]
async fn list_validators_honors_a_string_rules_flag_from_an_agent() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    write_ruleset(
        &project.path().join(".validators"),
        "rust-rules",
        "**/*.rs",
        &[],
    );
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let rows_for = |flag: serde_json::Value| {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("list validators"));
        args.insert("match".to_string(), json!(RUST_MATCH_TARGET));
        args.insert("rules".to_string(), flag);
        args
    };

    let from_bool = tool
        .execute(rows_for(json!(true)), &context)
        .await
        .expect("boolean flag");
    let from_string = tool
        .execute(rows_for(json!("true")), &context)
        .await
        .expect("string flag");
    assert_eq!(
        extract_text(&from_bool),
        extract_text(&from_string),
        "`rules: \"true\"` must return the same bodies as `rules: true`"
    );

    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&from_string)).unwrap();
    let fixture = parsed
        .as_array()
        .expect("list returns an array")
        .iter()
        .find(|r| r["name"] == json!("rust-rules"))
        .expect("the fixture validator is listed");
    assert!(
        fixture["rules"][0]["body"]
            .as_str()
            .unwrap_or_default()
            .contains("Check the code"),
        "the string flag must carry the verbatim rule bodies: {fixture}"
    );

    // A value that is neither a boolean nor "true"/"false" is rejected, not
    // quietly read as `false` — the caller learns the flag did not apply.
    let error = tool
        .execute(rows_for(json!("yes")), &context)
        .await
        .expect_err("an unreadable flag must not silently default");
    assert!(
        error.to_string().contains("rules"),
        "the error names the offending parameter: {error}"
    );
}

/// A glob-fragment `match` (not a concrete path) keeps its documented lenient
/// behavior: it answers "which validators declare this glob?", so a caller can
/// still discover a ruleset by the pattern it matches on.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn list_validators_matches_a_glob_fragment_leniently() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let project_validators = project.path().join(".validators");
    write_ruleset(&project_validators, "rust-rules", "**/*.rs", &[]);
    write_ruleset(&project_validators, "ts-rules", "**/*.ts", &[]);
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("list validators"));
    args.insert("match".to_string(), json!("**/*.ts"));
    let result = tool.execute(args, &context).await.expect("list validators");
    let body = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let listed: Vec<String> = parsed
        .as_array()
        .expect("list returns an array")
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_string())
        .collect();

    assert!(
        listed.contains(&"ts-rules".to_string()),
        "a glob fragment must find the validator declaring it: {body}"
    );
    assert!(
        !listed.contains(&"rust-rules".to_string()),
        "a glob fragment must not drag in unrelated validators: {listed:?}"
    );
}

/// An empty `match` is no filter at all, not a path that matches nothing: the
/// listing is the same one a call with no `match` returns.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn list_validators_treats_an_empty_match_as_no_filter() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let project_validators = project.path().join(".validators");
    write_ruleset(&project_validators, "rust-rules", "**/*.rs", &[]);
    write_ruleset(&project_validators, "ts-rules", "**/*.ts", &[]);
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let listed = |match_value: Option<&str>| {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("list validators"));
        if let Some(value) = match_value {
            args.insert("match".to_string(), json!(value));
        }
        async {
            let result = tool.execute(args, &context).await.expect("list validators");
            let parsed: serde_json::Value =
                serde_json::from_str(&extract_text(&result)).expect("json array");
            parsed
                .as_array()
                .expect("list returns an array")
                .iter()
                .map(|r| r["name"].as_str().unwrap().to_string())
                .collect::<Vec<String>>()
        }
    };

    let unfiltered = listed(None).await;
    assert_eq!(
        listed(Some("")).await,
        unfiltered,
        "an empty `match` must not filter anything out"
    );
    assert!(
        unfiltered.contains(&"rust-rules".to_string())
            && unfiltered.contains(&"ts-rules".to_string()),
        "the unfiltered listing carries every validator: {unfiltered:?}"
    );
}

/// `rules` defaults to false: a plain `list validators` row stays a summary and
/// carries no rule bodies.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn list_validators_omits_rule_bodies_by_default() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    write_ruleset(
        &project.path().join(".validators"),
        "rust-rules",
        "*.rs",
        &[],
    );
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("list validators"));
    let result = tool.execute(args, &context).await.expect("list validators");
    let body = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

    let row = parsed
        .as_array()
        .expect("list returns an array")
        .iter()
        .find(|r| r["name"] == json!("rust-rules"))
        .expect("the fixture validator is listed");
    assert_eq!(
        row["rule_count"],
        json!(1),
        "the summary still counts rules"
    );
    assert!(
        row.get("rules").is_none(),
        "rule bodies must be omitted unless `rules: true`: {body}"
    );
}

#[tokio::test]
#[serial_test::serial(cwd)]
async fn check_validators_reports_an_unknown_probe() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let project_validators = project.path().join(".validators");
    // A validator declaring a probe that is NOT in the catalog.
    write_ruleset(
        &project_validators,
        "bad-probe",
        "*.rs",
        &["not-a-real-probe"],
    );
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("check validators"));
    let result = tool
        .execute(args, &context)
        .await
        .expect("check validators");
    let body = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(
        parsed["ok"],
        json!(false),
        "unknown probe must fail the lint: {body}"
    );
    let errors = parsed["errors"].as_array().unwrap();
    assert!(
        errors.iter().any(|e| e["problem"]
            .as_str()
            .unwrap_or("")
            .contains("not-a-real-probe")),
        "the unknown probe must be reported, got: {body}"
    );
}

#[tokio::test]
#[serial_test::serial(cwd)]
async fn check_validators_reports_a_malformed_validator_and_still_loads_the_valid_one() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let project_validators = project.path().join(".validators");
    // A malformed validator (unparseable frontmatter) alongside a valid one.
    write_malformed_ruleset(&project_validators, "broken-one");
    write_ruleset(&project_validators, "good-one", "*.rs", &["duplicates"]);
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("check validators"));
    let result = tool
        .execute(args, &context)
        .await
        .expect("check validators");
    let body = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

    // The malformed validator is surfaced as an error, not silently dropped.
    assert_eq!(
        parsed["ok"],
        json!(false),
        "a malformed validator must fail the lint: {body}"
    );
    let errors = parsed["errors"].as_array().unwrap();
    assert!(
        errors
            .iter()
            .any(|e| e["path"].as_str().unwrap_or("").contains("broken-one")),
        "the dropped validator's path must be named, got: {body}"
    );
    // The valid validator alongside it still loaded and is counted.
    let mut list_args = serde_json::Map::new();
    list_args.insert("op".to_string(), json!("list validators"));
    let listed = tool
        .execute(list_args, &context)
        .await
        .expect("list validators");
    let listed_body = extract_text(&listed);
    assert!(
        listed_body.contains("good-one"),
        "the valid validator alongside a broken one still loads, got: {listed_body}"
    );
}

#[tokio::test]
#[serial_test::serial(cwd)]
async fn get_validator_returns_rule_bodies_and_probes() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    write_ruleset(
        &project.path().join(".validators"),
        "deduplicate",
        "*.rs",
        &["duplicates"],
    );
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(project.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("get validator"));
    args.insert("name".to_string(), json!("deduplicate"));
    let result = tool.execute(args, &context).await.expect("get validator");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    assert_eq!(parsed["name"], json!("deduplicate"));
    assert_eq!(parsed["source_layer"], json!("project"));
    assert_eq!(parsed["probes"], json!(["duplicates"]));
    let rules = parsed["rules"].as_array().unwrap();
    assert!(
        rules
            .iter()
            .any(|r| r["body"].as_str().unwrap_or("").contains("Check the code")),
        "rule bodies must be returned verbatim: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// dump validators (loader read + one markdown rules file in the temp dir)
// ---------------------------------------------------------------------------

/// Guards for a seeded project the `dump validators` tests run in: an isolated
/// HOME (no user validators leak in), a temp git project carrying
/// `.validators`, and the CWD pinned to that project.
struct SeededProject {
    _home: IsolatedTestEnvironment,
    project: tempfile::TempDir,
    _cwd: CurrentDirGuard,
}

/// Seed a project with one RuleSet per `(name, glob, rule body)` triple.
fn seeded_project(rulesets: &[(&str, &str, &str)]) -> SeededProject {
    let home = IsolatedTestEnvironment::new().expect("isolated env");
    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    let validators_dir = project.path().join(".validators");
    for (name, glob, body) in rulesets {
        write_ruleset_with_rule_body(&validators_dir, name, glob, &[], body);
    }
    let cwd = CurrentDirGuard::new(project.path()).expect("chdir");
    SeededProject {
        _home: home,
        project,
        _cwd: cwd,
    }
}

/// Execute one `dump validators` call against a freshly registered tool rooted
/// at `dir`, returning the parsed JSON response.
async fn dump_validators_call(
    dir: &Path,
    paths: serde_json::Value,
) -> Result<serde_json::Value, rmcp::ErrorData> {
    let mut registry = ToolRegistry::new();
    register_review_tools(&mut registry);
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(dir).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("dump validators"));
    args.insert("paths".to_string(), paths);
    let result = tool.execute(args, &context).await?;
    Ok(serde_json::from_str(&extract_text(&result)).expect("json response"))
}

/// Read the markdown file a `dump validators` response points at, then remove
/// it so test runs do not accumulate temp files.
fn read_and_remove_rules_file(response: &serde_json::Value) -> String {
    let path = response["path"].as_str().expect("response carries a path");
    let markdown = std::fs::read_to_string(path).expect("the rules file exists");
    std::fs::remove_file(path).expect("remove the rules file");
    markdown
}

/// The distinct rust rule body the dump tests count in the produced markdown.
const RUST_RULE_BODY: &str = "Obey the Rust fixture rule.";
/// The distinct typescript rule body the dump tests count in the produced
/// markdown.
const TS_RULE_BODY: &str = "Obey the TS fixture rule.";

/// The name of the `project_types: [rust]` keyed RuleSet the workspace tests
/// seed.
const PROJECT_TYPE_RULESET_NAME: &str = "rust-workspace-rules";
/// The distinct rule body the `project_types`-keyed RuleSet carries.
const PROJECT_TYPE_RULE_BODY: &str = "Obey the rust-workspace fixture rule.";
/// A minimal Cargo manifest — the marker that makes a seeded workspace a
/// detected `rust` project.
const RUST_MANIFEST: (&str, &str) = (
    "Cargo.toml",
    "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
);
/// A minimal Python manifest — the marker that makes a seeded workspace a
/// detected `python` project, and NOT a rust one.
const PYTHON_MANIFEST: (&str, &str) = (
    "pyproject.toml",
    "[project]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
);

/// Seed a workspace whose only project manifest is `manifest`, carrying one
/// `project_types: [rust]` keyed RuleSet, then return the validator names a
/// `dump validators` call over a Rust path in it reports.
async fn dumped_validator_names_in_workspace(manifest: (&str, &str)) -> Vec<String> {
    let (manifest_name, manifest_body) = manifest;
    let _home = IsolatedTestEnvironment::new().expect("isolated env");
    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    std::fs::write(project.path().join(manifest_name), manifest_body).unwrap();
    write_project_type_scoped_ruleset(
        &project.path().join(".validators"),
        PROJECT_TYPE_RULESET_NAME,
        "**/*.rs",
        "rust",
        PROJECT_TYPE_RULE_BODY,
    );
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let response = dump_validators_call(project.path(), json!(["src/lib.rs"]))
        .await
        .expect("dump validators");
    read_and_remove_rules_file(&response);
    response["validators"]
        .as_array()
        .expect("validators is an array")
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}

/// A `project_types`-keyed validator — the shape every tool rule uses — must
/// reach the `dump validators` surface when the session workspace carries that
/// project type, and must stay out of it when the workspace does not.
///
/// Without workspace project-type resolution the keyed set fails closed
/// everywhere, so the rules-up-front dump silently omits every tool rule.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn dump_validators_matches_a_project_types_keyed_validator_only_in_that_workspace() {
    let seeded = PROJECT_TYPE_RULESET_NAME.to_string();

    let in_rust = dumped_validator_names_in_workspace(RUST_MANIFEST).await;
    assert!(
        in_rust.contains(&seeded),
        "a rust workspace must dump the `project_types: [rust]` keyed validator: {in_rust:?}"
    );

    let in_python = dumped_validator_names_in_workspace(PYTHON_MANIFEST).await;
    assert!(
        !in_python.contains(&seeded),
        "a non-rust workspace must not dump the `project_types: [rust]` keyed validator: {in_python:?}"
    );
}

/// Seed the two-extension fixture the dump tests share: one Rust-matching and
/// one TypeScript-matching RuleSet, each with a distinct rule body.
fn seeded_two_extension_project() -> SeededProject {
    seeded_project(&[
        ("rust-rules", "**/*.rs", RUST_RULE_BODY),
        ("ts-rules", "**/*.ts", TS_RULE_BODY),
    ])
}

/// Two `.rs` paths and one `.ts` path: the produced markdown must carry each
/// applicable rule body exactly ONCE — the validator set is deduplicated across
/// input paths, not repeated per path.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn dump_validators_writes_each_applicable_rule_body_once() {
    let seeded = seeded_two_extension_project();

    let response = dump_validators_call(
        seeded.project.path(),
        json!(["src/lib.rs", "src/main.rs", "src/app.ts"]),
    )
    .await
    .expect("dump validators");

    // The builtin layer matches too, so assert fixture-relative invariants:
    // both fixture validators are present, and the set carries no duplicate
    // even though two `.rs` paths matched the same validator.
    let validators: Vec<String> = response["validators"]
        .as_array()
        .expect("validators is an array")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    for name in ["rust-rules", "ts-rules"] {
        assert_eq!(
            validators.iter().filter(|v| *v == name).count(),
            1,
            "`{name}` must appear exactly once: {response}"
        );
    }

    let markdown = read_and_remove_rules_file(&response);
    for body in [RUST_RULE_BODY, TS_RULE_BODY] {
        assert_eq!(
            markdown.matches(body).count(),
            1,
            "the rule body must appear exactly once: {markdown}"
        );
    }

    // Deduplication at the count level: a second path with the SAME extension
    // matches the same validators, so it must not grow the dumped set.
    let single = dump_validators_call(seeded.project.path(), json!(["src/lib.rs"]))
        .await
        .expect("dump validators for one path");
    let doubled = dump_validators_call(seeded.project.path(), json!(["src/lib.rs", "src/main.rs"]))
        .await
        .expect("dump validators for two same-extension paths");
    assert_eq!(
        single["validators"], doubled["validators"],
        "a second same-extension path must not grow the validator set"
    );
    assert_eq!(
        single["rule_count"], doubled["rule_count"],
        "a second same-extension path must not grow the rule count"
    );
    read_and_remove_rules_file(&single);
    read_and_remove_rules_file(&doubled);
}

/// The response pairs each input path with the validator names the engine
/// matcher pairs it with, and lists the distinct extensions of the inputs.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn dump_validators_reports_matched_pairs_and_distinct_extensions() {
    let seeded = seeded_two_extension_project();

    let response = dump_validators_call(seeded.project.path(), json!(["src/lib.rs", "src/app.ts"]))
        .await
        .expect("dump validators");

    // The builtin layer matches too, so assert the fixture pairings: each path
    // carries its own extension's validator and never the other one.
    let matched_names = |path: &str| -> Vec<String> {
        response["matched"][path]
            .as_array()
            .expect("matched carries the path")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    };
    let rs_matched = matched_names("src/lib.rs");
    assert!(
        rs_matched.contains(&"rust-rules".to_string()),
        "the .rs path pairs with the Rust validator: {response}"
    );
    assert!(
        !rs_matched.contains(&"ts-rules".to_string()),
        "the .rs path must not pair with the TypeScript validator: {response}"
    );
    let ts_matched = matched_names("src/app.ts");
    assert!(
        ts_matched.contains(&"ts-rules".to_string()),
        "the .ts path pairs with the TypeScript validator: {response}"
    );
    assert!(
        !ts_matched.contains(&"rust-rules".to_string()),
        "the .ts path must not pair with the Rust validator: {response}"
    );
    assert_eq!(
        response["extensions"],
        json!(["rs", "ts"]),
        "the distinct input extensions: {response}"
    );
    read_and_remove_rules_file(&response);
}

/// A path no validator matches still produces a valid file — the file states
/// that no rules apply and the validator list is empty.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn dump_validators_with_no_match_writes_a_file_and_returns_an_empty_list() {
    let seeded = seeded_project(&[("rust-rules", "**/*.rs", RUST_RULE_BODY)]);

    let response = dump_validators_call(seeded.project.path(), json!(["docs/guide.md"]))
        .await
        .expect("dump validators");

    assert_eq!(
        response["validators"],
        json!([]),
        "no validator matches a .md path: {response}"
    );
    assert_eq!(response["rule_count"], json!(0), "{response}");
    assert_eq!(
        response["matched"]["docs/guide.md"],
        json!([]),
        "the unmatched path still appears in the map: {response}"
    );

    let markdown = read_and_remove_rules_file(&response);
    assert!(
        markdown.contains("No validator matches"),
        "the file must state that no rules apply: {markdown}"
    );
}

/// Forgiving input: a single string path and a stringified JSON array return
/// the same answer as the one-element array carrying that path.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn dump_validators_accepts_a_single_string_path() {
    let seeded = seeded_project(&[("rust-rules", "**/*.rs", RUST_RULE_BODY)]);

    let from_array = dump_validators_call(seeded.project.path(), json!(["src/lib.rs"]))
        .await
        .expect("dump validators with an array path");
    read_and_remove_rules_file(&from_array);

    for (shape, paths) in [
        ("a single string path", json!("src/lib.rs")),
        ("a stringified JSON array", json!("[\"src/lib.rs\"]")),
    ] {
        let response = dump_validators_call(seeded.project.path(), paths)
            .await
            .unwrap_or_else(|e| panic!("dump validators with {shape}: {e}"));
        for key in ["validators", "matched", "extensions", "rule_count"] {
            assert_eq!(
                response[key], from_array[key],
                "{shape} must answer like a one-element array"
            );
        }
        read_and_remove_rules_file(&response);
    }
}

/// An empty `paths` input is an error, not an empty rules file. A blank
/// string and a blank array element carry no path, so they are empty too.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn dump_validators_rejects_empty_paths() {
    let seeded = seeded_project(&[("rust-rules", "**/*.rs", RUST_RULE_BODY)]);

    for (shape, paths) in [
        ("an empty array", json!([])),
        ("a blank string", json!("")),
        ("a blank array element", json!([""])),
        ("a whitespace-only string", json!("  ")),
    ] {
        let error = match dump_validators_call(seeded.project.path(), paths).await {
            Err(error) => error,
            Ok(response) => panic!("{shape} must be rejected, got: {response}"),
        };
        assert!(
            error.message.contains("paths"),
            "the error for {shape} names the `paths` param: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// doctor health checks (`Doctorable::run_health_checks` over `check validators`)
// ---------------------------------------------------------------------------

/// Run the doctor surface against the workspace at `workspace_root`.
///
/// The root is pinned the way the doctor collector pins it, so the lint reads
/// that workspace's project validator layer. The process current directory is
/// left alone on purpose: it is the crate's own repository, which carries its
/// own `.validators`, so a check that reported it would fail these assertions.
fn doctor_checks_for_workspace(
    workspace_root: &Path,
) -> Vec<swissarmyhammer_common::health::HealthCheck> {
    use swissarmyhammer_common::health::Doctorable;

    ReviewTool::new()
        .with_doctor_workspace_root(Some(workspace_root.to_path_buf()))
        .run_health_checks()
}

#[tokio::test]
#[serial_test::serial(cwd)]
async fn doctor_reports_one_ok_when_all_validators_are_valid() {
    use swissarmyhammer_common::health::HealthStatus;

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // A project with a single valid validator (a known probe, a compiling glob,
    // no stray trigger). No malformed validators anywhere in the stack.
    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    write_ruleset(
        &project.path().join(".validators"),
        "deduplicate",
        "*.rs",
        &["duplicates"],
    );

    let checks = doctor_checks_for_workspace(project.path());

    assert_eq!(
        checks.len(),
        1,
        "all-valid validators should yield exactly one OK check, got: {checks:?}"
    );
    let check = &checks[0];
    assert_eq!(check.status, HealthStatus::Ok, "got: {check:?}");
    assert_eq!(check.name, "Validators");
    assert_eq!(check.category, "validators");
    assert!(
        check.message.contains("valid"),
        "the OK message should report all valid, got: {}",
        check.message
    );
}

#[tokio::test]
#[serial_test::serial(cwd)]
async fn doctor_reports_an_error_naming_a_malformed_validator_with_a_fix() {
    use swissarmyhammer_common::health::HealthStatus;

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // A project with a malformed validator: it declares a probe that is not in
    // the engine's probe catalog.
    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    write_ruleset(
        &project.path().join(".validators"),
        "bad-probe",
        "*.rs",
        &["not-a-real-probe"],
    );

    let checks = doctor_checks_for_workspace(project.path());

    let error = checks
        .iter()
        .find(|c| c.status == HealthStatus::Error)
        .unwrap_or_else(|| panic!("a malformed validator must produce an Error, got: {checks:?}"));

    assert_eq!(error.category, "validators");
    assert!(
        error.name.contains("bad-probe") || error.message.contains("bad-probe"),
        "the error must name the offending validator, got: name={:?} message={:?}",
        error.name,
        error.message
    );
    assert!(
        error.message.contains("not-a-real-probe"),
        "the error must describe the problem, got: {}",
        error.message
    );
    assert!(
        error.fix.is_some(),
        "the error must carry a fix suggestion, got: {error:?}"
    );
}

#[tokio::test]
#[serial_test::serial(cwd)]
async fn doctor_reports_an_error_for_a_dropped_malformed_validator() {
    use swissarmyhammer_common::health::HealthStatus;

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // A project with a malformed validator that fails to parse: the loader drops
    // it, but doctor must surface it as an Error rather than reporting all valid.
    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    write_malformed_ruleset(&project.path().join(".validators"), "broken-one");

    let checks = doctor_checks_for_workspace(project.path());

    let error = checks
        .iter()
        .find(|c| c.status == HealthStatus::Error)
        .unwrap_or_else(|| panic!("a dropped validator must produce an Error, got: {checks:?}"));

    assert_eq!(error.category, "validators");
    assert!(
        error.name.contains("broken-one") || error.message.contains("broken-one"),
        "the error must name the dropped validator, got: name={:?} message={:?}",
        error.name,
        error.message
    );
    assert!(
        error.fix.is_some(),
        "the error must carry a fix suggestion, got: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// review working (full pipeline through the registered tool, scripted agent)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_working_through_the_registered_tool_flags_a_planted_duplicate() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // A temp git repo with a committed file, a working-tree change adding a
    // duplicate function, a project validator, a seeded index, and a scripted
    // agent that confirms the planted duplicate.
    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    let result = tool
        .execute(args, &context)
        .await
        .expect("review working dispatch");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    let markdown = parsed["markdown"].as_str().unwrap();
    assert!(
        markdown.contains("- [ ] `src/lib.rs:1`"),
        "the confirmed blocker must be rendered, got: {markdown}"
    );
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert_eq!(parsed["counts"]["confirmed"], json!(1));
}

/// A `review file` op whose `path` climbs out of the repo root (`../…`) must be
/// rejected by the scope-stage containment guard, returning an error result with
/// no findings — the outside file's content is never read into the review agent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_file_with_a_traversal_path_is_rejected() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // A full, runnable pipeline (seeded index + validators + scripted agent) so
    // the request reaches scope resolution rather than failing earlier.
    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    // A secret file just ABOVE the repo dir that a naive join would leak.
    let marker = format!(
        "review_escape_{}.txt",
        repo.path().file_name().unwrap().to_string_lossy()
    );
    let outside = repo.path().parent().unwrap().join(&marker);
    std::fs::write(&outside, "TOP SECRET").unwrap();

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review file"));
    args.insert("path".to_string(), json!(format!("../{marker}")));
    args.insert("backend".to_string(), json!("local"));
    let result = tool.execute(args, &context).await;
    let _ = std::fs::remove_file(&outside);

    let err = result.expect_err("a traversal path must be rejected, never reviewed");
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains(&format!("../{marker}")),
        "the error must carry the full offending path: {rendered}"
    );
    assert!(
        rendered.contains("escapes the repository root"),
        "the error must explain the escape: {rendered}"
    );
}

// The engine's observability lines (`review scope resolved` / `fleet fan-out` /
// `review synthesis complete`) are asserted to surface on the REAL tool path
// under a **process-global** subscriber — the kind `sah serve` installs via
// `set_global_default` — by the dedicated integration binary
// `tests/review_global_subscriber.rs`. That test owns its whole process so the
// global default can be installed safely, and it faithfully reproduces the
// production logging condition. A thread-local *scoped* (`tracing-test`) check
// was deliberately NOT used here: its thread-local capture masked whether the
// `spawn_blocking` pipeline's lines reach the ambient subscriber at all.

// ---------------------------------------------------------------------------
// review working through a real McpServer wired via `set_review_factories`
// (the server-layer injection seam)
// ---------------------------------------------------------------------------

/// Build a temp git repo + seeded index + project validator for a `review
/// working` run that flags a planted duplicate, and return the scripted factory.
///
/// Shared by the bare-registry test above and the McpServer wiring tests below
/// so the fixture is stated once.
fn planted_duplicate_fixture(repo: &TestRepo) -> AgentFactory {
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    let dup = dup_body("compute");
    repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

    write_ruleset(
        &repo.path().join(".validators"),
        "deduplicate",
        "*.rs",
        &["duplicates"],
    );
    seed_on_disk_index(repo.path(), &dup);

    let agent = ScriptedAgent::new(vec![
        (
            "# Validator: deduplicate".to_string(),
            ScriptedReply::Text(findings_json(
                "src/lib.rs",
                "compute duplicates old_compute",
            )),
        ),
        (
            "compute duplicates old_compute".to_string(),
            ScriptedReply::Text(confirm_json()),
        ),
    ]);
    scripted_factory(agent)
}

/// Like [`planted_duplicate_fixture`], plus a second changed file
/// (`src/huge.rs`, ~12 KB) matched by the same "deduplicate" validator but
/// carrying no findings of its own. Used to prove an oversized `batch_size`
/// skip on ONE file does not stop review of the others: the packer excludes
/// `huge.rs` before fan-out, so the batch (and this same scripted script) only
/// ever sees `src/lib.rs`.
fn two_file_fixture_one_oversized(repo: &TestRepo) -> AgentFactory {
    let factory = planted_duplicate_fixture(repo);
    // Untracked/added in the working-tree diff; its RENDERED block is far over
    // [`OVERSIZE_BATCH_SIZE`] while `src/lib.rs`'s still fits.
    repo.write(
        "src/huge.rs",
        &"// filler line of source text\n".repeat(400),
    );
    factory
}

/// Like [`planted_duplicate_fixture`], but the duplicate addition is COMMITTED
/// (not left as a working-tree change) so a `review sha HEAD~1..HEAD` range
/// scope sees it. The on-disk `.code-context` index is seeded AFTER the
/// commit — `TestRepo::commit` stages everything (`git add -A`), so seeding it
/// first would commit the binary index db as a tracked (and undiffable) blob.
fn planted_duplicate_fixture_committed(repo: &TestRepo) -> AgentFactory {
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    let dup = dup_body("compute");
    repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));
    repo.commit("add duplicate");

    write_ruleset(
        &repo.path().join(".validators"),
        "deduplicate",
        "*.rs",
        &["duplicates"],
    );
    seed_on_disk_index(repo.path(), &dup);

    let agent = ScriptedAgent::new(vec![
        (
            "# Validator: deduplicate".to_string(),
            ScriptedReply::Text(findings_json(
                "src/lib.rs",
                "compute duplicates old_compute",
            )),
        ),
        (
            "compute duplicates old_compute".to_string(),
            ScriptedReply::Text(confirm_json()),
        ),
    ]);
    scripted_factory(agent)
}

/// A findings array citing an explicit `line`, unlike [`findings_json`] (which
/// always cites `1`) — needed to plant a finding whose line does not exist in
/// the file at all.
fn findings_json_at_line(file: &str, line: u32, claim: &str) -> String {
    let array = json!([{
        "file": file,
        "line": line,
        "validator": "agent-tagged",
        "rule": "r",
        "claim": claim,
        "evidence": "per file inspection",
        "suggestion": "fix it",
    }]);
    format!("```json\n{array}\n```")
}

/// A single 3-line changed file and a scripted fan-out response that cites a
/// wildly out-of-range line for it (^j4d2613: "review engine reports findings
/// against a stale revision — cited line numbers do not resolve"). The
/// `style` validator declares no probes, so the deterministic fact-probe guard
/// (`GUARD_RULES`) cannot decide this finding either way — only the line-bounds
/// guard can catch it before it ever reaches the adversarial verifier.
///
/// The verify script CONFIRMS the claim if this candidate ever reaches it —
/// deliberately, so a regression in the bounds guard would surface the finding
/// in the report rather than vanishing behind some other refute-by-default
/// path and giving a false-negative "still passes" result.
fn stale_line_fixture(repo: &TestRepo) -> AgentFactory {
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    repo.write("src/lib.rs", "fn placeholder() {}\n\nfn changed() {}\n");

    write_ruleset(&repo.path().join(".validators"), "style", "*.rs", &[]);
    // The `style` validator declares no probes, but the review tool still
    // requires an on-disk code_context index to exist (production contract);
    // an empty schema is enough since no probe ever queries it here.
    let _ = on_disk_index_conn(repo.path());

    let agent = ScriptedAgent::new(vec![
        (
            "# Validator: style".to_string(),
            ScriptedReply::Text(findings_json_at_line(
                "src/lib.rs",
                9999,
                "changed has a stale-line style violation",
            )),
        ),
        (
            "changed has a stale-line style violation".to_string(),
            ScriptedReply::Text(confirm_json()),
        ),
    ]);
    scripted_factory(agent)
}

/// A finding citing a line past the end of the file's current content must
/// never reach the report, even when the (deliberately rigged) adversarial
/// verifier would confirm it if it ever saw it — proving the deterministic
/// line-bounds guard, not luck, is what removes it (^j4d2613).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_working_drops_a_finding_whose_cited_line_is_out_of_bounds() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = stale_line_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    let result = tool
        .execute(args, &context)
        .await
        .expect("review working dispatch");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    let markdown = parsed["markdown"].as_str().unwrap();
    assert!(
        !markdown.contains("stale-line"),
        "a finding citing a line that does not exist in the file must never \
         render, even though the scripted verifier would confirm it if it were \
         ever reached: {markdown}"
    );
    assert_eq!(parsed["counts"]["findings"], json!(0));
    assert_eq!(parsed["counts"]["confirmed"], json!(0));
    assert_eq!(
        parsed["counts"]["refuted"],
        json!(1),
        "the out-of-bounds finding must still be tallied as refuted: {parsed}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn mcp_server_set_review_factories_runs_review_working_end_to_end() {
    use crate::mcp::server::McpServer;
    use swissarmyhammer_templating::TemplateLibrary;

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    // The production-shaped seam: build the real server (registers the bare
    // review tool), then inject the factories at the wiring layer.
    let server =
        McpServer::new_with_work_dir(TemplateLibrary::default(), repo.path().to_path_buf())
            .await
            .expect("server builds");
    server
        .set_review_factories(factory, Some(mock_embedder_factory()), None)
        .await;

    let result = server
        .execute_tool(
            "review",
            json!({ "op": "review working", "backend": "local" }),
        )
        .await
        .expect("review working dispatch");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    let markdown = parsed["markdown"].as_str().unwrap();
    assert!(
        markdown.contains("- [ ] `src/lib.rs:1`"),
        "the confirmed blocker must be rendered through the server, got: {markdown}"
    );
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert_eq!(parsed["counts"]["confirmed"], json!(1));
}

// ---------------------------------------------------------------------------
// `batch_size` modifier: reaches the engine for each of the three review ops,
// bad values behave as documented, and an oversized file is a named gap
// (never a hard error that blocks the rest of the scope). Regression coverage
// for ^3rnvage.
// ---------------------------------------------------------------------------

/// A `batch_size`, in bytes, well under the RENDERED block of the planted
/// `src/lib.rs` fixture (`fn placeholder() {}\n\n<dup body>\n`, ~181 bytes of
/// source that renders to a couple of KB) but well over zero — small enough
/// that only an override this small (not the agent-cap default) could make the
/// packer skip it.
const TINY_BATCH_SIZE: u64 = 50;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_working_batch_size_override_skips_a_file_the_default_would_review() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    args.insert("batch_size".to_string(), json!(TINY_BATCH_SIZE));
    let result = tool
        .execute(args, &context)
        .await
        .expect("an oversized file must be a named gap, never a hard error");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();
    let markdown = parsed["markdown"].as_str().unwrap();

    // The SAME fixture, at the DEFAULT batch_size, is confirmed and reviewed
    // (`review_working_through_the_registered_tool_flags_a_planted_duplicate`).
    // Here it is skipped instead — proof the passed value, not the default,
    // reached the packer.
    assert!(
        markdown.contains("src/lib.rs"),
        "the skipped file must be named: {markdown}"
    );
    assert!(
        markdown.contains(&format!("{TINY_BATCH_SIZE}-byte batch budget")),
        "the report must name THIS run's batch_size, not the default: {markdown}"
    );
    assert_eq!(parsed["counts"]["skipped"], json!(1));
    // The skip is a coverage failure: it surfaces as a CONFIRMED checklist
    // finding and in the structured skipped-file list, not only as a warning
    // line, so a review with an over-cap file can never end clean.
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert!(
        markdown.contains("- [ ] `src/lib.rs:1`"),
        "the skip must render as a checklist finding: {markdown}"
    );
    assert_eq!(parsed["counts"]["skipped_files"], json!(["src/lib.rs"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_file_batch_size_override_skips_a_file_the_default_would_review() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review file"));
    args.insert("path".to_string(), json!("src/lib.rs"));
    args.insert("backend".to_string(), json!("local"));
    args.insert("batch_size".to_string(), json!(TINY_BATCH_SIZE));
    let result = tool
        .execute(args, &context)
        .await
        .expect("an oversized file must be a named gap, never a hard error");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();
    let markdown = parsed["markdown"].as_str().unwrap();

    assert!(
        markdown.contains("src/lib.rs"),
        "the skipped file must be named: {markdown}"
    );
    assert!(
        markdown.contains(&format!("{TINY_BATCH_SIZE}-byte batch budget")),
        "the report must name THIS run's batch_size, not the default: {markdown}"
    );
    assert_eq!(parsed["counts"]["skipped"], json!(1));
    // The skip is a coverage failure: it surfaces as a CONFIRMED checklist
    // finding and in the structured skipped-file list.
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert_eq!(parsed["counts"]["skipped_files"], json!(["src/lib.rs"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_sha_batch_size_override_skips_a_file_the_default_would_review() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture_committed(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review sha"));
    args.insert("sha".to_string(), json!("HEAD~1..HEAD"));
    args.insert("backend".to_string(), json!("local"));
    args.insert("batch_size".to_string(), json!(TINY_BATCH_SIZE));
    let result = tool
        .execute(args, &context)
        .await
        .expect("an oversized file must be a named gap, never a hard error");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();
    let markdown = parsed["markdown"].as_str().unwrap();

    assert!(
        markdown.contains("src/lib.rs"),
        "the skipped file must be named: {markdown}"
    );
    assert!(
        markdown.contains(&format!("{TINY_BATCH_SIZE}-byte batch budget")),
        "the report must name THIS run's batch_size, not the default: {markdown}"
    );
    assert_eq!(parsed["counts"]["skipped"], json!(1));
    // The skip is a coverage failure: it surfaces as a CONFIRMED checklist
    // finding and in the structured skipped-file list.
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert_eq!(parsed["counts"]["skipped_files"], json!(["src/lib.rs"]));
}

/// A genuinely large source file — one whose rendered block fills most of the
/// default batch budget — must review through a normal route: the default
/// budget, no explicit `batch_size`. Regression for ^3rnvage, where an
/// oversized real file was skipped instead of reviewed.
///
/// The oversized file is generated rather than read off disk: no file in this
/// workspace fills that much of the budget, so a real fixture cannot state the
/// size premise. The size is what the batcher acts on, and the run below drives
/// the actual registered `review` tool over a real git repo.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_file_reviews_an_oversized_source_file_under_the_default_budget() {
    use swissarmyhammer_validators::review::fleet::{
        rendered_file_block_bytes, DEFAULT_BATCH_SIZE,
    };
    use swissarmyhammer_validators::review::scope::FileWork;

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let current_default = DEFAULT_BATCH_SIZE;

    // The budget is spent in RENDERED bytes, never raw ones, so the fixture is
    // sized with `rendered_file_block_bytes` — the very cost function the packer
    // budgets on — rather than by its length on disk. The two numbers are far
    // apart: `render_file_block` prints every source line as
    // `{line:>6} | {sha:8} {mark} | {text}`, about 22 fixed bytes PER LINE, so a
    // file of narrow lines renders at several times its raw size. Do not put a
    // raw-byte comparison back here; a raw size states nothing about whether the
    // packer takes the file.
    let measure = |content: &str| {
        rendered_file_block_bytes(&FileWork::new(
            "src/server.rs".to_string(),
            Vec::new(),
            Vec::new(),
            content.to_string(),
            Vec::new(),
        ))
    };

    // Three quarters of the default budget: unmistakably an oversized file, and
    // the remaining quarter covers what this measure leaves out — the run's
    // prompt framing, which `file_payload_budget` subtracts from the cap, and
    // the semantic diff rendered beside the source.
    let target_rendered = current_default / 4 * 3;
    // Wide lines keep the rendered size close to the raw size, so the fixture
    // stays a plausible source file instead of a column of line numbers. Real
    // Rust so the validator fan-out treats it as a `*.rs` file.
    let filler_line = format!("// {}\n", "wide filler comment text ".repeat(15));
    let per_line = measure(&filler_line.repeat(2)) - measure(&filler_line);
    let block_overhead = measure(&filler_line) - per_line;
    let filler_lines = target_rendered.saturating_sub(block_overhead) / per_line;
    let real_content = format!(
        "pub fn generated_filler_function_for_the_oversized_review_fixture() {{}}\n{}",
        filler_line.repeat(filler_lines)
    );

    let rendered = measure(&real_content);
    assert!(
        rendered < current_default,
        "the fixture must render inside the current default budget \
         ({rendered} rendered bytes vs {current_default})"
    );
    assert!(
        rendered > current_default / 2,
        "the fixture must be a genuinely large file, not a token one \
         ({rendered} rendered bytes vs {current_default})"
    );

    let repo = TestRepo::new();
    repo.write("src/lib.rs", "pub fn placeholder() {}\n");
    repo.commit("initial");
    // Untracked/added: the whole oversized file becomes this run's `review file` scope.
    repo.write("src/server.rs", &real_content);
    write_ruleset(&repo.path().join(".validators"), "rust-rules", "*.rs", &[]);
    on_disk_index_conn(repo.path()); // creates the schema; no rows needed here.
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let agent = ScriptedAgent::new(vec![(
        "# Validator: rust-rules".to_string(),
        ScriptedReply::Text("```json\n[]\n```".to_string()),
    )]);

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(scripted_factory(agent))
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review file"));
    args.insert("path".to_string(), json!("src/server.rs"));
    args.insert("backend".to_string(), json!("local"));
    let result = tool
        .execute(args, &context)
        .await
        .expect("an oversized source file must be reviewable under the current default batch_size");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    assert_eq!(
        parsed["counts"]["skipped"],
        json!(0),
        "the file must NOT be skipped as too large under the current default: {parsed}"
    );
    // The exact count depends on how many loaded validators (builtins included)
    // match `*.rs`, which is not this test's concern — only that fan-out
    // actually ran over the file at all, proving it entered a batch rather
    // than being rejected before ever reaching the engine.
    assert!(
        parsed["counts"]["attempted"].as_u64().unwrap_or(0) >= 1,
        "the fan-out must actually run over the file (proves it entered a batch): {parsed}"
    );
}

/// A negative `batch_size` is documented (`usize_arg`) as treated-as-absent,
/// not rejected: the run must fall back to the default budget and review
/// normally, exactly like omitting the modifier.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_batch_size_negative_value_falls_back_to_the_default() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    args.insert("batch_size".to_string(), json!(-1));
    let result = tool
        .execute(args, &context)
        .await
        .expect("a negative batch_size falls back to the default, never errors");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    assert_eq!(
        parsed["counts"]["skipped"],
        json!(0),
        "the default budget comfortably covers the tiny fixture file: {parsed}"
    );
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert_eq!(parsed["counts"]["confirmed"], json!(1));
}

/// A fractional `batch_size` is documented as treated-as-absent, same as
/// negative — falls back to the default budget.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_batch_size_fractional_value_falls_back_to_the_default() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    args.insert("batch_size".to_string(), json!(1.5));
    let result = tool
        .execute(args, &context)
        .await
        .expect("a fractional batch_size falls back to the default, never errors");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    assert_eq!(parsed["counts"]["skipped"], json!(0));
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert_eq!(parsed["counts"]["confirmed"], json!(1));
}

/// `batch_size: 0` is a real (if degenerate) value, not absent — `usize_arg`
/// only treats a NEGATIVE or FRACTIONAL number as absent. Zero is a clean
/// unsigned integer, so it is honored: every file (having at least one byte)
/// exceeds it and is skipped, never a hard error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_batch_size_zero_skips_every_file() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    args.insert("batch_size".to_string(), json!(0));
    let result = tool
        .execute(args, &context)
        .await
        .expect("batch_size: 0 is a gap, never a hard error");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();

    assert_eq!(parsed["counts"]["skipped"], json!(1));
    // The skip is a coverage failure: even the degenerate zero budget yields a
    // CONFIRMED finding and the structured skipped-file list.
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert_eq!(parsed["counts"]["skipped_files"], json!(["src/lib.rs"]));
}

/// Two changed files matched by the same validator: one fits `batch_size`, one
/// does not. The oversized file must not block review of the other — the
/// small file is still reviewed and confirmed, and the report separately names
/// the skipped one.
/// A `batch_size`, in bytes, between the two rendered blocks
/// [`two_file_fixture_one_oversized`] plants: `src/lib.rs` renders to ~2.4 KB
/// and clears it, `src/huge.rs` renders to ~30 KB and does not. The budget is
/// spent in RENDERED bytes, so it is sized against the rendered blocks, never
/// against the raw source.
const OVERSIZE_BATCH_SIZE: u64 = 5_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_working_an_oversized_file_does_not_block_review_of_the_others() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = two_file_fixture_one_oversized(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    args.insert("batch_size".to_string(), json!(OVERSIZE_BATCH_SIZE));
    let result = tool
        .execute(args, &context)
        .await
        .expect("the oversized file must not block review of the other file");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();
    let markdown = parsed["markdown"].as_str().unwrap();

    assert!(
        markdown.contains("- [ ] `src/lib.rs:1`"),
        "the small file must still be reviewed and confirmed: {markdown}"
    );
    assert!(
        markdown.contains("src/huge.rs"),
        "the oversized file must be named as a gap: {markdown}"
    );
    // Two findings: the planted duplicate on the small file, plus the
    // engine-emitted skip finding on the oversized one.
    assert_eq!(parsed["counts"]["findings"], json!(2));
    assert_eq!(parsed["counts"]["confirmed"], json!(2));
    assert_eq!(parsed["counts"]["skipped"], json!(1));
    assert_eq!(parsed["counts"]["skipped_files"], json!(["src/huge.rs"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_tool_with_concurrency_pins_the_pool_worker_count() {
    // The `review.concurrency` override pins the pool worker count regardless of
    // the coarse `backend` choice. A pinned count of 1 must still drive the
    // full pipeline to a confirmed finding (behavioural proof the request runs
    // with the pinned config rather than erroring).
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(factory)
            .with_embedder_factory(mock_embedder_factory())
            .with_concurrency(Some(2)),
    );
    let tool = registry.get_tool("review").unwrap();
    let context = context_at(repo.path()).await;

    // `backend: session` would normally pick the remote default worker count;
    // the pinned concurrency overrides it. The run must still succeed.
    let result = tool
        .execute(
            json!({ "op": "review working", "backend": "session" })
                .as_object()
                .unwrap()
                .clone(),
            &context,
        )
        .await
        .expect("review working dispatch with pinned concurrency");
    let parsed: serde_json::Value = serde_json::from_str(&extract_text(&result)).unwrap();
    assert_eq!(parsed["counts"]["findings"], json!(1));
    assert_eq!(parsed["counts"]["confirmed"], json!(1));
}

// ---------------------------------------------------------------------------
// process-wide pipeline serialization (the parallel-review OOM cap)
// ---------------------------------------------------------------------------

/// Two `run_review_request` calls fired concurrently must NOT overlap: the
/// process-global pipeline gate serializes them so only one corpus + embedder +
/// agent set is ever resident at once. Each run still fans out internally across
/// its `AgentPool`, so this caps the per-run footprint multiplier that OOMed a
/// 512GB box under a full parallel review — it does not serialize the work
/// inside a run.
///
/// The probe is the embedder factory: it records how many runs are inside the
/// gated pipeline body at once (the factory is called only after the permit is
/// acquired). With the gate, the peak is 1; without it, two concurrent runs both
/// enter and the peak is 2.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial(cwd)]
async fn review_pipelines_run_one_at_a_time_process_wide() {
    use super::review_op::{run_review_request, ReviewRequest};
    use swissarmyhammer_validators::review::Scope;

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // One shared repo + seeded index; both runs review it read-only.
    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);

    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let embedder = concurrency_probe_embedder_factory(Arc::clone(&active), Arc::clone(&peak));

    let request = || ReviewRequest::new(Scope::Working).with_backend(Some("local".to_string()));
    let run = || {
        run_review_request(
            request(),
            repo.path(),
            Arc::clone(&embedder),
            Arc::clone(&factory),
            "2026-06-07 12:00",
            None,
        )
    };

    let (a, b) = tokio::join!(run(), run());
    a.expect("first review run");
    b.expect("second review run");

    assert_eq!(
        peak.load(Ordering::SeqCst),
        1,
        "review pipelines must run one at a time process-wide; two overlapped"
    );
}

/// An [`EmbedderFactory`] that records the peak number of review pipelines inside
/// the gated body concurrently, then yields the deterministic mock embedder. The
/// brief sleep widens the overlap window so an ungated pair reliably coincides.
fn concurrency_probe_embedder_factory(
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
) -> EmbedderFactory {
    // The `_observer` arg is the download-progress hook; a mock downloads
    // nothing, so it is ignored.
    Arc::new(move |_observer| {
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        Box::pin(async move {
            // Hold each embedder "active" long enough that two overlapping review
            // pipelines are observable via the peak counter before either releases.
            const CONCURRENCY_PROBE_HOLD_MS: u64 = 150;
            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(CONCURRENCY_PROBE_HOLD_MS)).await;
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(Arc::new(model_embedding::mock::MockEmbedder::new(DIM))
                as Arc<dyn model_embedding::TextEmbedder>)
        })
    })
}

// ---------------------------------------------------------------------------
// scripted-agent + on-disk-index harness
//
// The throwaway git repo (`TestRepo`), the on-disk index builder
// (`on_disk_index_conn`), the row seeder (`seed_chunk`), the function-body
// helper (`dup_body`), and the embedding dimension (`DIM`) are all the SHARED
// review test seam from `swissarmyhammer_validators::review::test_support`,
// imported above rather than re-declared here.
// ---------------------------------------------------------------------------

/// Seed an on-disk code_context index at `<root>/.code-context/index.db` with the
/// duplicate function present in another file, so `find_duplicates` hits.
fn seed_on_disk_index(root: &Path, dup: &str) {
    let conn = on_disk_index_conn(root);
    let emb = dup_emb();
    seed_chunk(&conn, "src/lib.rs", "compute", dup, &emb);
    seed_chunk(&conn, "src/existing.rs", "old_compute", dup, &emb);
}

/// A findings array as a fleet agent emits it (the `validator` field is tagged by
/// the engine, but must be present for the finding to deserialize).
fn findings_json(file: &str, claim: &str) -> String {
    // Built through `serde_json` so any `"`/`\` in `file`/`claim` is escaped
    // correctly — a raw `format!` template would corrupt the JSON.
    let array = json!([{
        "file": file,
        "line": 1,
        "validator": "agent-tagged",
        "rule": "r",
        "claim": claim,
        "evidence": "per `duplicates`: 0.99",
        "suggestion": "extract a helper",
    }]);
    format!("```json\n{array}\n```")
}

/// A confirming verify verdict.
fn confirm_json() -> String {
    "```json\n{\"confirmed\": true, \"reason\": \"the duplicate is real\"}\n```".to_string()
}

#[test]
fn findings_json_escapes_embedded_quotes() {
    // A claim carrying a double quote must round-trip through valid JSON, proving
    // the helper escapes rather than concatenates raw text.
    let claim = r#"the literal "7" is a magic number"#;
    let fenced = findings_json("src/a.rs", claim);
    let body = fenced
        .trim_start_matches("```json")
        .trim_end_matches("```")
        .trim();
    let parsed: serde_json::Value =
        serde_json::from_str(body).expect("findings_json is valid JSON");
    assert_eq!(parsed[0]["claim"], json!(claim));
    assert_eq!(parsed[0]["file"], json!("src/a.rs"));
}

/// An [`EmbedderFactory`] yielding a deterministic mock embedder (no model load).
fn mock_embedder_factory() -> EmbedderFactory {
    // The `_observer` arg is the download-progress hook; a mock downloads
    // nothing, so it is ignored.
    Arc::new(|_observer| {
        Box::pin(async {
            Ok(Arc::new(model_embedding::mock::MockEmbedder::new(DIM))
                as Arc<dyn model_embedding::TextEmbedder>)
        })
    })
}

/// Build an [`AgentFactory`] that mints a fresh in-process scripted ACP agent
/// shaped like a real [`swissarmyhammer_agent::AcpAgentHandle`]: the agent streams
/// its reply onto a backend broadcast, and the handle's `notification_rx` is a
/// `subscribe()` of that same channel — the authoritative stream the driver
/// collects from. The agent also bridges each notification onto the live
/// connection (as `wrap_claude_into_handle`'s `forward_session_notifications`
/// does), so this exercises the production dual-emission shape; the driver must
/// collect it once.
fn scripted_factory(agent: Arc<ScriptedAgent>) -> AgentFactory {
    Arc::new(move || {
        let agent = Arc::clone(&agent);
        Box::pin(async move {
            let (notify_tx, notification_rx) =
                broadcast::channel(SCRIPTED_AGENT_NOTIFY_BUFFER_SIZE);
            // Rebind the shared harness onto this run's broadcast and bridge each
            // reply onto the live connection too (the production dual-emission the
            // driver must collect once).
            let agent = ScriptedAgent::rebind_broadcast(&agent, notify_tx, true);
            let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));
            Ok(AgentHandle::new(dyn_agent, notification_rx))
        })
    })
}
