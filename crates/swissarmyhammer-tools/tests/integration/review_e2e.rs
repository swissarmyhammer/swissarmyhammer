//! End-to-end acceptance test for the local multi-agent review feature.
//!
//! This mirrors `tests/integration/semantic_search_e2e.rs` (real indexer → real
//! query → real result) for the review tool: a **real temp git repo** with a diff
//! that PLANTS specific defects on purpose, a **real on-disk code_context index**
//! (production schema), and the **registered production `review` tool** driven
//! over a deterministic/playback ACP agent and a mock embedder so CI needs no live
//! model. It asserts pipeline BEHAVIOR — scope → fan-out → guard → verify →
//! synthesize through the registered tool — not canned output strings.
//!
//! ## What the planted diff exercises
//!
//! | # | Plant                                              | Caught by      | Severity | Verdict                 |
//! |---|----------------------------------------------------|----------------|----------|-------------------------|
//! | 1 | copy-pasted block duplicating an existing function | `duplication`  | blocker  | confirmed (`duplicates`)|
//! | 2 | helper reimplementing an existing shared util      | `reuse`        | warning  | confirmed (`similar`)   |
//! | 3 | hardcoded if-chain over a known set (→ table)      | `data-driven`  | warning  | confirmed (in-file)     |
//! | 4 | new function with zero inbound callers              | `dead-code`    | blocker  | confirmed (`callers`)   |
//! | 5 | a planted secret                                   | `no-secrets`   | blocker  | confirmed (in-file)     |
//! | 6 | an agent red herring (looks buggy, is correct)     | `rust`         | warning  | REFUTED by the agent    |
//! | 7 | a guard red herring (claimed dead, but IS called)  | `dead-code`    | blocker  | REFUTED by the guard    |
//! | 8 | a Rust idiom issue                                 | `rust`         | warning  | confirmed (language)    |
//!
//! Items 1–5 + 8 must be confirmed at the right validator + severity; item 6 must
//! be refuted by the adversarial verifier (the agent runs, says "disproven"); item
//! 7 must be refuted by the deterministic guard (the `callers` fact contradicts the
//! "dead" claim) WITHOUT any verifier agent ever seeing it — proving the two-layer
//! verify. The driver runs across `review working`, `review sha`, and `review
//! file`, all of which share the same dispatch → driver → engine path.

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
use swissarmyhammer_kanban::{
    board::InitBoard, task::AddTask, task::GetTask, Execute, KanbanContext,
};
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::review::review_op::{
    AgentFactory, AgentHandle, EmbedderFactory,
};
use swissarmyhammer_tools::mcp::tools::review::ReviewTool;
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// The planted source.
//
// Every file is `.rs`, so it matches all the builtin source-code validators
// (`duplication`, `reuse`, `data-driven`, `dead-code`, `no-secrets`, `rust`). The
// defects are spread across four files (≤ the default fan-out batch size of 4) so
// each validator's files fit one batch — one fan-out task per validator, so the
// scripted agent fires each validator response exactly once.
// ---------------------------------------------------------------------------

const FILE_PAYMENTS: &str = "src/payments.rs";
const FILE_REUSE: &str = "src/util_reuse.rs";
const FILE_ORPHAN: &str = "src/orphan.rs";
const FILE_LIVE: &str = "src/live.rs";

/// An existing indexed file whose function the duplicate (item 1) copies. It is
/// only in the index, never in the diff.
const FILE_EXISTING: &str = "src/existing_total.rs";
/// An existing indexed util the reuse helper (item 2) reimplements.
const FILE_UTIL: &str = "src/shared_util.rs";

/// Embedding dimension shared by the seeded index and the mock embedder.
const DIM: usize = 4;

/// The function body item 1 copy-pastes verbatim from `existing_total`. Long
/// enough to clear the index `min_chunk_bytes` (100).
fn duplicated_total_body() -> String {
    "pub fn sum_amounts(input: &[f64]) -> f64 {\n    let mut total = 0.0;\n    for value in input {\n        total += value * value;\n    }\n    total / input.len() as f64\n}".to_string()
}

/// The reuse helper body item 2 reimplements (semantically the shared util).
fn reimplemented_util_body() -> String {
    "pub fn my_mean_squared(values: &[f64]) -> f64 {\n    let mut acc = 0.0;\n    for v in values {\n        acc += v * v;\n    }\n    acc / values.len() as f64\n}".to_string()
}

/// The dead orphan body (item 4) — a new function nothing calls.
fn dead_orphan_body() -> String {
    "pub fn orphan_never_called(input: &[f64]) -> f64 {\n    let mut s = 0.0;\n    for x in input {\n        s += x;\n    }\n    s\n}".to_string()
}

/// The "claimed dead but actually called" body (item 7).
fn live_called_body() -> String {
    "pub fn claimed_dead_but_called(input: &[f64]) -> f64 {\n    let mut s = 1.0;\n    for x in input {\n        s *= x;\n    }\n    s\n}".to_string()
}

/// The whole `src/payments.rs` after-content: hosts the duplicate (item 1), the
/// hardcoded if-chain (item 3), the planted secret (item 5), and the
/// correct-but-looks-buggy code (item 6).
fn payments_after() -> String {
    format!(
        "//! Payment helpers.\nuse std::fmt;\n\n\
         // item 5: a planted secret committed to code.\n\
         const STRIPE_KEY: &str = \"sk_live_PLACEHOLDER_not_a_real_key_for_tests\";\n\n\
         {dup}\n\n\
         // item 3: a hardcoded if-chain over a known set that should be a table.\n\
         pub fn fee_for_tier(tier: &str) -> f64 {{\n\
         \x20   if tier == \"bronze\" {{\n        0.03\n    }} else if tier == \"silver\" {{\n        0.02\n    }} else if tier == \"gold\" {{\n        0.01\n    }} else {{\n        0.05\n    }}\n}}\n\n\
         // item 6: looks like an off-by-one but is correct (inclusive range is intended).\n\
         pub fn last_index(len: usize) -> usize {{\n    len.saturating_sub(1)\n}}\n",
        dup = duplicated_total_body(),
    )
}

/// Write the planted working-tree diff into `repo`: a committed baseline, then the
/// four changed `.rs` files added on top.
fn plant_diff(repo: &TestRepo) {
    repo.write("src/lib.rs", "pub fn placeholder() {}\n");
    repo.commit("baseline");

    repo.write(FILE_PAYMENTS, &payments_after());
    repo.write(
        FILE_REUSE,
        &format!(
            "//! A util that reinvents the shared one.\n\n{}\n",
            reimplemented_util_body()
        ),
    );
    repo.write(
        FILE_ORPHAN,
        &format!("//! An orphan module.\n\n{}\n", dead_orphan_body()),
    );
    repo.write(
        FILE_LIVE,
        &format!(
            "//! A function the guard will find a caller for.\n\n{}\n",
            live_called_body()
        ),
    );
}

// ---------------------------------------------------------------------------
// On-disk code_context index (production schema, deterministic rows).
//
// Built at `<repo>/.code-context/index.db` — exactly the path the production
// review tool opens read-only — using the real `create_schema`, then seeded so
// the engine-run probes hit deterministically:
//   - `duplicates` finds `existing_total.rs` for the payments copy (item 1),
//   - `similar` finds `shared_util.rs` for the reuse helper (item 2),
//   - `callers` finds an inbound caller for `claimed_dead_but_called` (item 7),
//     and finds NONE for `orphan_never_called` (item 4).
// ---------------------------------------------------------------------------

/// The constant vector the [`model_embedding::mock::MockEmbedder`] returns for
/// every text. Chunks seeded with this vector are maximally similar to any query.
fn mock_vec() -> Vec<f32> {
    vec![0.1_f32; DIM]
}

/// Seed the on-disk index used by the production review tool path.
fn seed_on_disk_index(root: &Path) {
    use swissarmyhammer_code_context::db::{configure_connection, create_schema};
    use swissarmyhammer_code_context::serialize_embedding;

    let ctx_dir = root.join(".code-context");
    std::fs::create_dir_all(&ctx_dir).unwrap();
    let conn = rusqlite::Connection::open(ctx_dir.join("index.db")).unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();

    let seed_file = |file: &str| {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed, embedded)
             VALUES (?1, X'DEADBEEF', 1024, 1000, 1, 1, 1)",
            rusqlite::params![file],
        )
        .unwrap();
    };
    let seed_chunk = |file: &str, symbol: &str, text: &str, emb: &[f32]| {
        seed_file(file);
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
             VALUES (?1, 0, ?2, 1, 10, ?3, ?4, ?5)",
            rusqlite::params![file, text.len() as i64, symbol, text, serialize_embedding(emb)],
        )
        .unwrap();
    };
    let seed_symbol = |id: &str, name: &str, file: &str| {
        seed_file(file);
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
             VALUES (?1, ?2, 12, ?3, 1, 0, 5, 0, NULL)",
            rusqlite::params![id, name, file],
        )
        .unwrap();
    };
    let seed_call_edge = |caller_id: &str,
                          callee_id: &str,
                          caller_file: &str,
                          callee_file: &str| {
        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, source, from_ranges)
             VALUES (?1, ?2, ?3, ?4, 'lsp', '[]')",
            rusqlite::params![caller_id, callee_id, caller_file, callee_file],
        )
        .unwrap();
    };

    // item 1: the duplicate's chunk in the changed file + the same block in an
    // existing indexed file → `find_duplicates` on payments.rs hits existing.
    let total = duplicated_total_body();
    let dup_emb = vec![1.0_f32, 0.0, 0.0, 0.0];
    seed_chunk(FILE_PAYMENTS, "sum_amounts", &total, &dup_emb);
    seed_chunk(FILE_EXISTING, "compute_total", &total, &dup_emb);

    // item 2: an existing shared util with the mock embedder's query vector, so
    // `search code` ranks it as a reuse candidate for the reimplemented helper.
    seed_chunk(
        FILE_UTIL,
        "mean_squared_error",
        &reimplemented_util_body(),
        &mock_vec(),
    );

    // item 7: `claimed_dead_but_called` HAS an inbound caller → `callers` fact has
    // rows → the guard refutes the "dead" claim deterministically.
    seed_symbol("callee-live", "claimed_dead_but_called", FILE_LIVE);
    seed_symbol("caller-live", "invoke_claimed", "src/caller.rs");
    seed_call_edge("caller-live", "callee-live", "src/caller.rs", FILE_LIVE);

    // item 4: `orphan_never_called` is intentionally absent from lsp_symbols /
    // lsp_call_edges → `callers` returns no rows → a real dead-code signal.
}

// ---------------------------------------------------------------------------
// The scripted ACP agent.
//
// Maps each prompt onto a response by matching ALL of a set of substrings, so a
// response can require both a validator header and a specific file (fan-out) or a
// specific claim (verify). The guard-vs-agent distinction (items 6 vs 7) is then
// asserted from the report's confirmed/refuted counts and the absence of each
// refuted claim — both observable through the production tool's response.
// ---------------------------------------------------------------------------

/// One scripted rule: every needle in `needles` must be present in the prompt for
/// `response` to fire.
struct Rule {
    needles: Vec<String>,
    response: String,
}

struct ScriptedAgent {
    next_session: AtomicUsize,
    rules: Vec<Rule>,
    notify_tx: Option<broadcast::Sender<SessionNotification>>,
}

impl ScriptedAgent {
    fn new(rules: Vec<Rule>) -> Arc<Self> {
        Arc::new(Self {
            next_session: AtomicUsize::new(0),
            rules,
            notify_tx: None,
        })
    }

    fn with_notifier(
        base: Arc<ScriptedAgent>,
        notify_tx: broadcast::Sender<SessionNotification>,
    ) -> Arc<Self> {
        Arc::new(Self {
            next_session: AtomicUsize::new(0),
            rules: base
                .rules
                .iter()
                .map(|r| Rule {
                    needles: r.needles.clone(),
                    response: r.response.clone(),
                })
                .collect(),
            notify_tx: Some(notify_tx),
        })
    }

    fn response_for(&self, prompt: &str) -> String {
        for rule in &self.rules {
            if rule.needles.iter().all(|n| prompt.contains(n.as_str())) {
                return rule.response.clone();
            }
        }
        // No rule matched: an empty findings array for fan-out, which also parses
        // as "no verdict" for verify → refute by default (harmless here).
        "```json\n[]\n```".to_string()
    }
}

/// A fan-out rule: fire `findings` when the prompt is the fan-out task for
/// `validator` AND mentions `file` (so batching can never double-fire it).
fn fanout(validator: &str, file: &str, findings: &str) -> Rule {
    Rule {
        needles: vec![
            format!("# Validator: {validator}"),
            format!("## File: {file}"),
        ],
        response: findings.to_string(),
    }
}

/// A verify rule: fire `verdict` when the adversarial prompt carries `claim`.
fn verify(claim: &str, verdict: &str) -> Rule {
    Rule {
        needles: vec!["# Adversarial verification".to_string(), claim.to_string()],
        response: verdict.to_string(),
    }
}

/// A findings JSON array (fenced) with one finding. `validator` is overwritten by
/// the engine, but must be present to deserialize.
fn finding(file: &str, line: u32, severity: &str, claim: &str) -> String {
    format!(
        "```json\n[{{\"file\":\"{file}\",\"line\":{line},\"validator\":\"agent-tagged\",\
         \"rule\":\"r\",\"severity\":\"{severity}\",\"claim\":\"{claim}\",\
         \"evidence\":\"per probe evidence\",\"suggestion\":\"fix it\"}}]\n```"
    )
}

/// Two findings in one array (for a validator that flags two files in one batch).
fn two_findings(a: (&str, u32, &str, &str), b: (&str, u32, &str, &str)) -> String {
    let obj = |file: &str, line: u32, severity: &str, claim: &str| {
        format!(
            "{{\"file\":\"{file}\",\"line\":{line},\"validator\":\"agent-tagged\",\
             \"rule\":\"r\",\"severity\":\"{severity}\",\"claim\":\"{claim}\",\
             \"evidence\":\"per probe evidence\",\"suggestion\":\"fix it\"}}"
        )
    };
    format!(
        "```json\n[{},{}]\n```",
        obj(a.0, a.1, a.2, a.3),
        obj(b.0, b.1, b.2, b.3),
    )
}

/// A confirming verify verdict.
fn confirm() -> String {
    "```json\n{\"confirmed\": true, \"reason\": \"substantiated by the evidence\"}\n```".to_string()
}

/// A refuting verify verdict (the agent disproves the red herring).
fn refute() -> String {
    "```json\n{\"confirmed\": false, \"reason\": \"the code is correct; the claim is disproven\"}\n```".to_string()
}

// ---- unique claim strings (keys shared by fan-out emission and verify) -------

const CLAIM_DUP: &str = "copy-pasted sum_amounts duplicates compute_total";
const CLAIM_REUSE: &str = "my_mean_squared reimplements the shared mean_squared_error util";
const CLAIM_DATA: &str = "fee_for_tier hardcodes a tier if-chain that should be a table";
const CLAIM_DEAD_ORPHAN: &str = "orphan_never_called has no inbound callers and is dead";
const CLAIM_SECRET: &str = "STRIPE_KEY is a hardcoded live secret";
const CLAIM_RED_HERRING: &str = "last_index looks like an off-by-one bug";
const CLAIM_GUARD_HERRING: &str = "claimed_dead_but_called appears to be dead code";
const CLAIM_RUST_IDIOM: &str = "fee_for_tier returns a bare f64 where a typed Money would be safer";

/// The full scripted agent for the planted diff: each validator's fan-out emits
/// its planted finding(s); each confirmable claim confirms on verify; the two red
/// herrings are refuted (item 6 by the agent here, item 7 by the guard — no verify
/// rule, the guard intercepts it first).
fn planted_agent() -> Arc<ScriptedAgent> {
    ScriptedAgent::new(vec![
        // ---- fan-out: one rule per (validator, file) -------------------------
        fanout(
            "duplication",
            FILE_PAYMENTS,
            &finding(FILE_PAYMENTS, 8, "blocker", CLAIM_DUP),
        ),
        fanout(
            "reuse",
            FILE_REUSE,
            &finding(FILE_REUSE, 3, "warning", CLAIM_REUSE),
        ),
        fanout(
            "data-driven",
            FILE_PAYMENTS,
            &finding(FILE_PAYMENTS, 16, "warning", CLAIM_DATA),
        ),
        fanout(
            "no-secrets",
            FILE_PAYMENTS,
            &finding(FILE_PAYMENTS, 5, "blocker", CLAIM_SECRET),
        ),
        // dead-code flags BOTH the real orphan (item 4) and the red-herring it
        // wrongly believes is dead (item 7). One fan-out task, two findings.
        fanout(
            "dead-code",
            FILE_ORPHAN,
            &two_findings(
                (FILE_ORPHAN, 3, "blocker", CLAIM_DEAD_ORPHAN),
                (FILE_LIVE, 3, "blocker", CLAIM_GUARD_HERRING),
            ),
        ),
        // rust flags a real idiom (item 8) and a correct-but-looks-buggy red
        // herring (item 6) that the verifier will refute.
        fanout(
            "rust",
            FILE_PAYMENTS,
            &two_findings(
                (FILE_PAYMENTS, 16, "warning", CLAIM_RUST_IDIOM),
                (FILE_PAYMENTS, 22, "warning", CLAIM_RED_HERRING),
            ),
        ),
        // ---- verify: confirm the real findings, refute the agent red herring --
        verify(CLAIM_DUP, &confirm()),
        verify(CLAIM_REUSE, &confirm()),
        verify(CLAIM_DATA, &confirm()),
        verify(CLAIM_SECRET, &confirm()),
        verify(CLAIM_DEAD_ORPHAN, &confirm()),
        verify(CLAIM_RUST_IDIOM, &confirm()),
        // item 6: looks buggy, is correct → the adversarial verifier refutes it.
        verify(CLAIM_RED_HERRING, &refute()),
        // item 7 deliberately has NO verify rule: the guard refutes it first, so a
        // verify prompt for it must never be generated.
    ])
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
            .name("scripted-review-agent")
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

/// Build an [`AgentFactory`] minting a fresh scripted agent that shares one
/// `seen`-log via the captured `Arc`, shaped like a real `AcpAgentHandle`: it
/// streams onto a backend broadcast (the handle's `notification_rx`) AND bridges
/// the same notification onto the live connection.
fn scripted_factory(agent: Arc<ScriptedAgent>) -> AgentFactory {
    Arc::new(move || {
        let agent = Arc::clone(&agent);
        Box::pin(async move {
            let (notify_tx, notification_rx) = broadcast::channel(256);
            let agent = ScriptedAgent::with_notifier(agent, notify_tx);
            let dyn_agent = DynConnectTo::new(ScriptedAdapter(agent));
            Ok(AgentHandle {
                agent: dyn_agent,
                notification_rx,
            })
        })
    })
}

/// An [`EmbedderFactory`] yielding the deterministic mock embedder (no model load).
fn mock_embedder_factory() -> EmbedderFactory {
    Arc::new(|| {
        Box::pin(async {
            Ok(Arc::new(model_embedding::mock::MockEmbedder::new(DIM))
                as Arc<dyn model_embedding::TextEmbedder>)
        })
    })
}

// ---------------------------------------------------------------------------
// git repo fixture (libgit2, real refs).
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

    fn commit(&self, message: &str) -> String {
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
        let oid = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap();
        oid.to_string()
    }
}

// ---------------------------------------------------------------------------
// harness: register the tool, run an op, parse the report.
// ---------------------------------------------------------------------------

/// Build a [`ToolContext`] rooted at `dir`.
async fn context_at(dir: &Path) -> ToolContext {
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(swissarmyhammer_config::ModelConfig::default());
    let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = Some(dir.to_path_buf());
    ctx
}

/// Extract the JSON text body of a tool result.
fn extract_text(result: &rmcp::model::CallToolResult) -> String {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text content"),
    }
}

/// Run one review op through the registered production tool and return the parsed
/// `{markdown, counts}` response.
async fn run_review_op(
    repo: &TestRepo,
    args: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(scripted_factory(planted_agent()))
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").expect("review tool registered");
    let context = context_at(repo.path()).await;
    let result = tool
        .execute(args, &context)
        .await
        .expect("review op dispatch");
    serde_json::from_str(&extract_text(&result)).expect("review response is JSON")
}

/// Args for `review working`, forcing the local single-worker backend so the run
/// is deterministic.
fn working_args() -> serde_json::Map<String, serde_json::Value> {
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    args
}

/// Whether the rendered markdown contains a confirmed finding whose claim matches.
fn report_has_claim(markdown: &str, claim_fragment: &str) -> bool {
    markdown.contains(claim_fragment)
}

// ---------------------------------------------------------------------------
// The acceptance tests.
// ---------------------------------------------------------------------------

/// `review working` over the planted diff: items 1–5 + 8 are confirmed at the
/// right severity, item 6 is agent-refuted, item 7 is guard-refuted, and neither
/// refuted finding appears in the report.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_e2e_working_confirms_real_defects_and_refutes_both_red_herrings() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    plant_diff(&repo);
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let parsed = run_review_op(&repo, working_args()).await;
    let markdown = parsed["markdown"].as_str().expect("markdown string");

    // --- the dated GFM checklist format renders ---
    assert!(
        markdown.contains("## Review Findings ("),
        "report must render the dated GFM section header: {markdown}"
    );

    // --- items 1–5 + 8: confirmed at the right severity ---
    assert!(
        markdown.contains("### Blockers"),
        "blockers section: {markdown}"
    );
    assert!(
        markdown.contains("### Warnings"),
        "warnings section: {markdown}"
    );

    assert!(
        report_has_claim(markdown, CLAIM_DUP),
        "item 1 duplication: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_SECRET),
        "item 5 secret: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_DEAD_ORPHAN),
        "item 4 dead orphan: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_REUSE),
        "item 2 reuse: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_DATA),
        "item 3 data-driven: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_RUST_IDIOM),
        "item 8 rust idiom: {markdown}"
    );

    // The three confirmed blockers (1, 4, 5) render under Blockers, before Warnings.
    let blockers_at = markdown.find("### Blockers").unwrap();
    let warnings_at = markdown.find("### Warnings").unwrap();
    assert!(
        blockers_at < warnings_at,
        "blockers render before warnings: {markdown}"
    );
    for blocker_claim in [CLAIM_DUP, CLAIM_DEAD_ORPHAN, CLAIM_SECRET] {
        let at = markdown.find(blocker_claim).unwrap();
        assert!(
            at > blockers_at && at < warnings_at,
            "blocker `{blocker_claim}` must render in the Blockers section: {markdown}"
        );
    }

    // --- item 6: agent-refuted → NOT in the report ---
    assert!(
        !report_has_claim(markdown, CLAIM_RED_HERRING),
        "item 6 (agent red herring) must be refuted and absent: {markdown}"
    );
    // --- item 7: guard-refuted → NOT in the report ---
    assert!(
        !report_has_claim(markdown, CLAIM_GUARD_HERRING),
        "item 7 (guard red herring) must be refuted and absent: {markdown}"
    );

    // --- counts: confirmed vs refuted, blockers vs warnings ---
    let counts = &parsed["counts"];
    assert_eq!(
        counts["blockers"],
        json!(3),
        "items 1,4,5 are confirmed blockers: {counts}"
    );
    assert_eq!(
        counts["warnings"],
        json!(3),
        "items 2,3,8 are confirmed warnings: {counts}"
    );
    assert_eq!(
        counts["confirmed"],
        json!(6),
        "six confirmed findings (1,2,3,4,5,8): {counts}"
    );
    assert_eq!(
        counts["refuted"],
        json!(2),
        "two refuted findings (item 6 by agent, item 7 by guard): {counts}"
    );
}

/// `review sha <range>` over the same planted change committed as a range — the
/// committed-scope path shares the dispatch → driver → engine path with `working`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_e2e_sha_range_confirms_the_same_defects() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    plant_diff(&repo);
    // Commit the planted change so a range scope sees it as committed work.
    repo.commit("plant the reviewable defects");
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review sha"));
    args.insert("sha".to_string(), json!("HEAD~1..HEAD"));
    args.insert("backend".to_string(), json!("local"));

    let parsed = run_review_op(&repo, args).await;
    let markdown = parsed["markdown"].as_str().expect("markdown string");

    assert!(
        report_has_claim(markdown, CLAIM_DUP),
        "duplication via sha: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_DEAD_ORPHAN),
        "dead-code via sha: {markdown}"
    );
    assert!(
        !report_has_claim(markdown, CLAIM_GUARD_HERRING),
        "guard refutation holds via sha: {markdown}"
    );
    assert!(
        !report_has_claim(markdown, CLAIM_RED_HERRING),
        "agent refutation holds via sha: {markdown}"
    );
    assert_eq!(
        parsed["counts"]["refuted"],
        json!(2),
        "two refuted via sha: {parsed}"
    );
}

/// `review file <glob>` over the planted files — the file/glob scope shares the
/// same dispatch → driver → engine path and confirms its scoped defects.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_e2e_file_glob_confirms_scoped_defects() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    plant_diff(&repo);
    repo.commit("plant the reviewable defects");
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review file"));
    args.insert("path".to_string(), json!("src/*.rs"));
    args.insert("backend".to_string(), json!("local"));

    let parsed = run_review_op(&repo, args).await;
    let markdown = parsed["markdown"].as_str().expect("markdown string");

    // The glob whole-content scope still drives the engine to confirmed findings.
    assert!(
        report_has_claim(markdown, CLAIM_DUP) || report_has_claim(markdown, CLAIM_DEAD_ORPHAN),
        "file/glob scope must surface at least one confirmed defect: {markdown}"
    );
    // Refutations are scope-independent: the guard still kills item 7.
    assert!(
        !report_has_claim(markdown, CLAIM_GUARD_HERRING),
        "guard refutation holds via file/glob: {markdown}"
    );
}

/// Skill write-path contract: the report's `markdown` (the dated GFM section the
/// `review` tool returns) lands verbatim on a kanban task — the range-mode
/// tracking-task write the review skill performs (`builtin/skills/review/SKILL.md`
/// step: "embed the report's `markdown`"). The skill itself runs in the agent
/// harness, but its data contract — engine report markdown → task description,
/// byte-for-byte, in the documented dated format — is exercised here against a
/// real file-backed kanban board.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_e2e_report_lands_on_a_kanban_task_in_the_dated_gfm_format() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    plant_diff(&repo);
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    // 1. Drive the production review tool to a report.
    let parsed = run_review_op(&repo, working_args()).await;
    let markdown = parsed["markdown"]
        .as_str()
        .expect("markdown string")
        .to_string();
    assert!(
        markdown.contains("## Review Findings ("),
        "precondition: the engine produced the dated section: {markdown}"
    );

    // 2. Write it onto a kanban task exactly as the skill's range-mode path does:
    //    a tracking task in the board, with `Scope: ...\n\n<report.markdown>` as
    //    the description.
    let kanban_dir = repo.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);
    InitBoard::new("Review E2E")
        .execute(&ctx)
        .await
        .into_result()
        .expect("init board");

    let description = format!("Scope: working\n\n{markdown}");
    let added = AddTask::new("Review of working")
        .with_description(&description)
        .execute(&ctx)
        .await
        .into_result()
        .expect("add tracking task");
    let task_id = added["id"].as_str().expect("new task id").to_string();

    // 3. Read it back: the dated section and a confirmed checklist item must have
    //    landed verbatim, and the refuted herrings must not be present.
    let task = GetTask::new(task_id)
        .execute(&ctx)
        .await
        .into_result()
        .expect("get tracking task");
    let stored = task["description"].as_str().expect("task description");

    assert!(
        stored.contains("## Review Findings ("),
        "the dated GFM section must land on the task verbatim: {stored}"
    );
    assert!(
        stored.contains("### Blockers"),
        "the severity checklist must land on the task: {stored}"
    );
    assert!(
        report_has_claim(stored, CLAIM_DUP),
        "a confirmed finding must land on the task: {stored}"
    );
    assert!(
        !report_has_claim(stored, CLAIM_GUARD_HERRING)
            && !report_has_claim(stored, CLAIM_RED_HERRING),
        "refuted findings must not land on the task: {stored}"
    );
}
