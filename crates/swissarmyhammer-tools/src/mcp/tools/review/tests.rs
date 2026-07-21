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
    body as dup_body, dup_emb, on_disk_index_conn, seed_chunk, ScriptedAdapter, ScriptedAgent,
    ScriptedReply, TestRepo, DIM,
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
        "get validator",
        "check validators",
    ] {
        assert!(
            op_strings.iter().any(|s| s == expected),
            "op `{expected}` must be advertised, got: {op_strings:?}"
        );
    }
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

/// Write a minimal RuleSet (VALIDATOR.md + one rule) under `base/<name>/`, with
/// the given file glob and probe list.
fn write_ruleset(base: &Path, name: &str, glob: &str, probes: &[&str]) {
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
            "---\nname: {name}\ndescription: {name} ruleset\nmatch:\n  files:\n    - \"{glob}\"\n{probes_yaml}---\n\n# {name}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("rules/check.md"),
        "---\nname: check\ndescription: Check\n---\n\nCheck the code.\n",
    )
    .unwrap();
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
    let agent_config = Arc::new(swissarmyhammer_config::ModelConfig::default());
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
// doctor health checks (`Doctorable::run_health_checks` over `check validators`)
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial(cwd)]
async fn doctor_reports_one_ok_when_all_validators_are_valid() {
    use swissarmyhammer_common::health::{Doctorable, HealthStatus};

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
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let checks = ReviewTool::new().run_health_checks();

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
    use swissarmyhammer_common::health::{Doctorable, HealthStatus};

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
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let checks = ReviewTool::new().run_health_checks();

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
    use swissarmyhammer_common::health::{Doctorable, HealthStatus};

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // A project with a malformed validator that fails to parse: the loader drops
    // it, but doctor must surface it as an Error rather than reporting all valid.
    let project = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    write_malformed_ruleset(&project.path().join(".validators"), "broken-one");
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let checks = ReviewTool::new().run_health_checks();

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
        McpServer::new_with_work_dir(TemplateLibrary::default(), repo.path().to_path_buf(), None)
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
    Arc::new(move || {
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
    Arc::new(|| {
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
