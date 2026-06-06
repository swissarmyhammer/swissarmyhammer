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

use agent_client_protocol::schema::{
    ContentBlock, ContentChunk, InitializeResponse, NewSessionResponse, PromptRequest,
    PromptResponse, SessionNotification, SessionUpdate, TextContent,
};
use agent_client_protocol::{Client, ConnectTo, ConnectionTo, DynConnectTo, Role};
use serde_json::json;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use tokio::sync::broadcast;

use super::review_op::{AgentFactory, AgentHandle, EmbedderFactory};
use super::*;
use crate::mcp::tool_handlers::ToolHandlers;
use crate::mcp::tool_registry::{ToolContext, ToolRegistry};

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
            "---\nname: {name}\ndescription: {name} ruleset\nseverity: error\nmatch:\n  files:\n    - \"{glob}\"\n{probes_yaml}---\n\n# {name}\n"
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
    std::fs::write(
        dir.join("VALIDATOR.md"),
        "---\nseverity: error\nmatch: [unterminated\n",
    )
    .unwrap();
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
        markdown.contains("### Blockers") && markdown.contains("src/lib.rs:1"),
        "the confirmed blocker must be rendered, got: {markdown}"
    );
    assert_eq!(parsed["counts"]["blockers"], json!(1));
    assert_eq!(parsed["counts"]["confirmed"], json!(1));
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
            findings_json("src/lib.rs", "blocker", "compute duplicates old_compute"),
        ),
        ("compute duplicates old_compute".to_string(), confirm_json()),
    ]);
    scripted_factory(agent)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn mcp_server_set_review_factories_runs_review_working_end_to_end() {
    use crate::mcp::server::McpServer;
    use swissarmyhammer_prompts::PromptLibrary;

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    let factory = planted_duplicate_fixture(&repo);
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    // The production-shaped seam: build the real server (registers the bare
    // review tool), then inject the factories at the wiring layer.
    let server =
        McpServer::new_with_work_dir(PromptLibrary::default(), repo.path().to_path_buf(), None)
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
        markdown.contains("### Blockers") && markdown.contains("src/lib.rs:1"),
        "the confirmed blocker must be rendered through the server, got: {markdown}"
    );
    assert_eq!(parsed["counts"]["blockers"], json!(1));
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
    assert_eq!(parsed["counts"]["blockers"], json!(1));
    assert_eq!(parsed["counts"]["confirmed"], json!(1));
}

// ---------------------------------------------------------------------------
// scripted-agent + on-disk-index + git-repo harness
// ---------------------------------------------------------------------------

struct TestRepo {
    dir: tempfile::TempDir,
    repo: git2::Repository,
}

impl TestRepo {
    fn new() -> Self {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test").unwrap();
            cfg.set_str("user.email", "test@example.com").unwrap();
        }
        Self { dir, repo }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, content: &str) {
        let full = self.dir.path().join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    fn commit(&self, message: &str) {
        let mut index = self.repo.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap();
    }
}

/// A function body long enough to clear the default `min_chunk_bytes` (100).
fn dup_body(label: &str) -> String {
    format!(
        "pub fn {label}(input: &[f64]) -> f64 {{\n    let mut total = 0.0;\n    for value in input {{\n        total += value * value;\n    }}\n    total / input.len() as f64\n}}"
    )
}

/// Seed an on-disk code_context index at `<root>/.code-context/index.db` with the
/// duplicate function present in another file, so `find_duplicates` hits.
fn seed_on_disk_index(root: &Path, dup: &str) {
    use swissarmyhammer_code_context::db::{configure_connection, create_schema};
    use swissarmyhammer_code_context::serialize_embedding;

    let ctx_dir = root.join(".code-context");
    std::fs::create_dir_all(&ctx_dir).unwrap();
    let conn = rusqlite::Connection::open(ctx_dir.join("index.db")).unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();

    let emb = vec![1.0_f32, 0.0, 0.0, 0.0];
    for (file, symbol) in [
        ("src/lib.rs", "compute"),
        ("src/existing.rs", "old_compute"),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed, embedded)
             VALUES (?1, X'DEADBEEF', 1024, 1000, 1, 1, 1)",
            rusqlite::params![file],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
             VALUES (?1, 0, ?2, 1, 10, ?3, ?4, ?5)",
            rusqlite::params![file, dup.len() as i64, symbol, dup, serialize_embedding(&emb)],
        )
        .unwrap();
    }
}

/// A findings array as a fleet agent emits it (the `validator` field is tagged by
/// the engine, but must be present for the finding to deserialize).
fn findings_json(file: &str, severity: &str, claim: &str) -> String {
    format!(
        "```json\n[{{\"file\":\"{file}\",\"line\":1,\"validator\":\"agent-tagged\",\
         \"rule\":\"r\",\"severity\":\"{severity}\",\"claim\":\"{claim}\",\
         \"evidence\":\"per `duplicates`: 0.99\",\"suggestion\":\"extract a helper\"}}]\n```"
    )
}

/// A confirming verify verdict.
fn confirm_json() -> String {
    "```json\n{\"confirmed\": true, \"reason\": \"the duplicate is real\"}\n```".to_string()
}

/// Embedding dimension shared by the seeded on-disk index and the mock embedder.
const DIM: usize = 4;

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
            let (notify_tx, notification_rx) = broadcast::channel(64);
            let agent = ScriptedAgent::with_notifier(agent, notify_tx);
            let dyn_agent = DynConnectTo::new(ScriptedAdapter(agent));
            Ok(AgentHandle {
                agent: dyn_agent,
                notification_rx,
            })
        })
    })
}

/// A scripted ACP agent that maps each prompt onto a response by substring match.
struct ScriptedAgent {
    next_session: AtomicUsize,
    script: Vec<(String, String)>,
    /// Backend broadcast the agent streams its reply onto — the same channel the
    /// handle's `notification_rx` is a `subscribe()` of. `None` until the factory
    /// wires it via [`ScriptedAgent::with_notifier`].
    notify_tx: Option<broadcast::Sender<SessionNotification>>,
}

impl ScriptedAgent {
    fn new(script: Vec<(String, String)>) -> Arc<Self> {
        Arc::new(Self {
            next_session: AtomicUsize::new(0),
            script,
            notify_tx: None,
        })
    }

    /// Clone the script into a fresh agent bound to a backend broadcast sender —
    /// the per-connection notifier the handle's `notification_rx` subscribes to.
    fn with_notifier(
        base: Arc<ScriptedAgent>,
        notify_tx: broadcast::Sender<SessionNotification>,
    ) -> Arc<Self> {
        Arc::new(Self {
            next_session: AtomicUsize::new(0),
            script: base.script.clone(),
            notify_tx: Some(notify_tx),
        })
    }

    fn response_for(&self, prompt: &str) -> String {
        for (needle, response) in &self.script {
            if prompt.contains(needle) {
                return response.clone();
            }
        }
        "[]".to_string()
    }
}

struct ScriptedAdapter(Arc<ScriptedAgent>);

impl ConnectTo<Client> for ScriptedAdapter {
    async fn connect_to(
        self,
        client: impl ConnectTo<<Client as Role>::Counterpart>,
    ) -> agent_client_protocol::Result<()> {
        let mock = Arc::clone(&self.0);
        agent_client_protocol::Agent
            .builder()
            .name("scripted-agent")
            .on_receive_request(
                {
                    let mock = Arc::clone(&mock);
                    async move |req: agent_client_protocol::ClientRequest, responder, cx| {
                        dispatch(&mock, req, responder, &cx)
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |_n: agent_client_protocol::ClientNotification, _cx| Ok(()),
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_to(client)
            .await
    }
}

fn dispatch(
    mock: &Arc<ScriptedAgent>,
    request: agent_client_protocol::ClientRequest,
    responder: agent_client_protocol::Responder<serde_json::Value>,
    cx: &ConnectionTo<Client>,
) -> agent_client_protocol::Result<()> {
    use agent_client_protocol::ClientRequest as Req;

    let mock = Arc::clone(mock);
    let cx = cx.clone();
    cx.clone().spawn(async move {
        match request {
            Req::InitializeRequest(_) => responder
                .cast()
                .respond_with_result(Ok(InitializeResponse::new(1.into()))),
            Req::NewSessionRequest(_req) => {
                let n = mock.next_session.fetch_add(1, Ordering::SeqCst);
                let id = agent_client_protocol::schema::SessionId::new(format!("sess-{n}"));
                responder
                    .cast()
                    .respond_with_result(Ok(NewSessionResponse::new(id)))
            }
            Req::PromptRequest(req) => {
                let prompt = prompt_text(&req);
                let text = mock.response_for(&prompt);
                let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(
                    ContentBlock::Text(TextContent::new(text)),
                ));
                let notif = SessionNotification::new(req.session_id.clone(), update);
                // Publish onto the backend broadcast (the driver's
                // `notification_rx`), AND bridge the same notification onto the
                // connection (mirroring a real handle). The driver collects the
                // backend stream once and ignores the connection re-emission.
                if let Some(tx) = &mock.notify_tx {
                    let _ = tx.send(notif.clone());
                }
                let _ = cx.send_notification(notif);
                responder.cast().respond_with_result(Ok(PromptResponse::new(
                    agent_client_protocol::schema::StopReason::EndTurn,
                )))
            }
            _ => responder
                .cast::<serde_json::Value>()
                .respond_with_error(agent_client_protocol::Error::method_not_found()),
        }
    })
}

fn prompt_text(req: &PromptRequest) -> String {
    req.prompt
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}
