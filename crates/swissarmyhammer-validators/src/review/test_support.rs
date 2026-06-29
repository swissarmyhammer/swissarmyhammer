//! Shared test fixtures for the review pipeline's test modules.
//!
//! The pipeline's stages are tested against the same three real seams — a
//! throwaway libgit2 repository, a schema-applied in-memory code_context
//! index, and a deterministically injected validator loader — so the fixtures
//! live here exactly once and the test modules in `scope.rs`, `drive.rs`, and
//! `probes.rs` import them instead of carrying their own copies. The
//! agent-facing test modules (`fleet.rs`, `verify.rs`, `drive.rs`, and the
//! pool tests in `validators/pool.rs`) share the [`new_notifier`] channel
//! fixture and the scripted ACP mock-agent harness ([`ScriptedAgent`] and
//! friends, below) the same way — one implementation, parameterized by
//! [`ScriptedAgentConfig`], instead of per-module copies that drift.
//!
//! Exported behind the `test-support` feature, so downstream crates' tests
//! (notably `swissarmyhammer-tools`' review-tool tests) drive the SAME scripted
//! agent rather than carrying their own. Each consumer — and each in-crate test
//! module — uses a different subset of these fixtures, so `dead_code` is allowed
//! module-wide rather than chasing per-item gates per build configuration.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

use swissarmyhammer_code_context::db::{configure_connection, create_schema};
use swissarmyhammer_code_context::serialize_embedding;

use crate::validators::types::{RuleSet, RuleSetManifest, RuleSetMetadata, ValidatorMatch};
use crate::validators::{Rule, ValidatorLoader, ValidatorSource};

/// Embedding dimension shared by the seeded index and the mock embedder.
pub const DIM: usize = 4;

/// The broadcast-buffer slot count for the pool-backed test notifier. Comfortably
/// exceeds any test's notification volume so the broadcast subscription never
/// lags mid-assertion.
const NOTIFICATION_BUFFER_SIZE: usize = 64;

/// A fresh notification channel for pool-backed tests. The slot buffer
/// ([`NOTIFICATION_BUFFER_SIZE`]) comfortably exceeds any test's notification
/// volume so the broadcast subscription never lags mid-assertion.
pub(crate) fn new_notifier() -> std::sync::Arc<claude_agent::NotificationSender> {
    let (notifier, _) = claude_agent::NotificationSender::new(NOTIFICATION_BUFFER_SIZE);
    std::sync::Arc::new(notifier)
}

/// The LSP `SymbolKind` code for a function — what every [`seed_symbol`] row is.
const LSP_SYMBOL_KIND_FUNCTION: i64 = 12;

/// Per-validator execution budget, in seconds, baked into every fixture
/// [`RuleSet`]'s manifest. A fixed placeholder — the review-test fixtures assert
/// on matching, scoping, and probe wiring, never on timeout enforcement.
const RULESET_TIMEOUT_SECS: u32 = 30;

/// A deterministic embedding two chunks can share so they register as
/// duplicates. The length derives from [`DIM`] so the seeded index and the
/// mock embedder can never drift apart.
pub fn dup_emb() -> Vec<f32> {
    let mut v = vec![0.0; DIM];
    v[0] = 1.0;
    v
}

// ---- git repo fixture -------------------------------------------------

/// A throwaway git repo backed by a [`TempDir`], driven via libgit2 so the
/// pipeline's real `swissarmyhammer-git` reads see real refs/working-tree.
pub struct TestRepo {
    dir: TempDir,
    repo: git2::Repository,
}

impl std::fmt::Debug for TestRepo {
    /// Hand-rolled because `git2::Repository` is not `Debug`; reports the
    /// working-tree path, which is the only useful identity for a test fixture.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestRepo")
            .field("path", &self.dir.path())
            .finish_non_exhaustive()
    }
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip a working-tree-relative path down to only its `Normal` components so a
/// [`Path::join`] onto the repo root can never escape it. Drops a leading `/`
/// (which would otherwise make `join` replace the whole path), `..` climbs, and
/// any prefix/root component. A path that reduces to nothing yields `"."`.
fn confine_relative(rel: &str) -> PathBuf {
    use std::path::Component;
    let confined: PathBuf = Path::new(rel)
        .components()
        .filter_map(|c| match c {
            Component::Normal(seg) => Some(seg),
            _ => None,
        })
        .collect();
    if confined.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        confined
    }
}

impl TestRepo {
    /// Create a fresh test git repository initialized with libgit2, backed by a
    /// throwaway [`TempDir`] and configured with a deterministic test identity so
    /// commits succeed without the machine's global git config.
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test").unwrap();
            cfg.set_str("user.email", "test@example.com").unwrap();
        }
        Self { dir, repo }
    }

    /// The repository's working-tree root (the [`TempDir`] path).
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Write a file to the working tree (no staging).
    ///
    /// `rel` is confined under the repo root: a leading `/` or any `..`/prefix
    /// component is dropped so only `Normal` path segments survive. Without this,
    /// an absolute `rel` would make [`Path::join`] replace the whole path and
    /// escape the [`TempDir`], and a `..` segment would climb out of it.
    pub fn write(&self, rel: &str, content: &str) {
        let full = self.dir.path().join(confine_relative(rel));
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    /// Stage everything and commit, returning the commit sha.
    pub fn commit(&self, message: &str) -> String {
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

// ---- code_context index fixture --------------------------------------

/// Open a real, schema-applied, in-memory code_context index (same shape the
/// probe runner uses in production), seeded deterministically.
pub fn index_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();
    conn
}

/// Open a real, schema-applied, **on-disk** code_context index at
/// `<root>/.code-context/index.db` — exactly the path the production review tool
/// opens read-only. The in-memory [`index_conn`] is what the engine's own probe
/// unit tests drive; the cross-crate tool/e2e tests instead need the index to
/// exist on disk where the tool's `open_index_connection` finds it. Each caller
/// seeds its scenario rows through the shared [`seed_chunk`] / [`seed_symbol`] /
/// [`seed_call_edge`] helpers (or leaves it empty), so the boilerplate — the
/// directory, the connection, the production schema — lives here exactly once.
pub fn on_disk_index_conn(root: &Path) -> Connection {
    let ctx_dir = root.join(".code-context");
    std::fs::create_dir_all(&ctx_dir).unwrap();
    let conn = Connection::open(ctx_dir.join("index.db")).unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();
    conn
}

/// Register a file in `indexed_files`. `ts_chunks` / `lsp_symbols` carry a
/// foreign key onto this table (and `configure_connection` enforces it), so
/// every seeded chunk/symbol needs its file registered first.
pub fn seed_file(conn: &Connection, file_path: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed, embedded)
         VALUES (?1, X'DEADBEEF', 1024, 1000, 1, 1, 1)",
        rusqlite::params![file_path],
    )
    .unwrap();
}

/// Seed a `ts_chunks` row with an embedding so `find_duplicates` /
/// `search_code` (which filter on `embedding IS NOT NULL`) can see it.
pub fn seed_chunk(
    conn: &Connection,
    file_path: &str,
    symbol_path: &str,
    text: &str,
    embedding: &[f32],
) {
    seed_file(conn, file_path);
    let blob = serialize_embedding(embedding);
    conn.execute(
        "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
         VALUES (?1, 0, ?2, 1, 10, ?3, ?4, ?5)",
        rusqlite::params![file_path, text.len() as i64, symbol_path, text, blob],
    )
    .unwrap();
}

/// Seed an `lsp_symbols` row (a function) so the `callers` probe can resolve a
/// symbol.
pub fn seed_symbol(conn: &Connection, id: &str, name: &str, file_path: &str) {
    seed_file(conn, file_path);
    conn.execute(
        "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
         VALUES (?1, ?2, ?3, ?4, 1, 0, 5, 0, NULL)",
        rusqlite::params![id, name, LSP_SYMBOL_KIND_FUNCTION, file_path],
    )
    .unwrap();
}

/// Seed an `lsp_call_edges` row (caller -> callee) for the `callers` probe.
pub fn seed_call_edge(
    conn: &Connection,
    caller_id: &str,
    callee_id: &str,
    caller_file: &str,
    callee_file: &str,
) {
    conn.execute(
        "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, source, from_ranges)
         VALUES (?1, ?2, ?3, ?4, 'lsp', '[]')",
        rusqlite::params![caller_id, callee_id, caller_file, callee_file],
    )
    .unwrap();
}

// ---- validator loader fixture ----------------------------------------

/// A loader carrying one RuleSet named `name` that matches `file_glob` and
/// declares `probes`.
///
/// A shared review-test fixture, re-exported from this crate so downstream test
/// crates (e.g. `swissarmyhammer-agent`'s review e2e) build the same loader.
/// `add_builtin_ruleset` is the deterministic injection seam — no on-disk
/// validators, so a test never depends on what's installed on the machine.
pub fn loader_with(name: &str, file_glob: &str, probes: &[&str]) -> ValidatorLoader {
    let mut loader = ValidatorLoader::new();
    loader.add_builtin_ruleset(ruleset(name, file_glob, probes));
    loader
}

/// A single-rule RuleSet named `name` that matches `file_glob` and declares
/// `probes`.
///
/// A shared review-test fixture, re-exported from this crate for downstream
/// test crates. The rule body is a fixed placeholder — tests assert on matching,
/// scoping, and probe wiring, not on rule prose.
pub fn ruleset(name: &str, file_glob: &str, probes: &[&str]) -> RuleSet {
    RuleSet {
        manifest: RuleSetManifest {
            name: name.to_string(),
            description: format!("{name} test ruleset"),
            metadata: RuleSetMetadata {
                version: "1.0.0".to_string(),
            },
            match_criteria: Some(ValidatorMatch {
                tools: vec![],
                files: vec![file_glob.to_string()],
            }),
            trigger_matcher: None,
            tags: vec![],
                probes: probes.iter().map(|p| p.to_string()).collect(),
                timeout: RULESET_TIMEOUT_SECS,
            once: false,
        },
        rules: vec![Rule {
            name: format!("{name}-rule"),
            description: "rule".to_string(),
            body: "body".to_string(),
            timeout: None,
        }],
        source: ValidatorSource::Builtin,
        base_path: PathBuf::from("/test"),
    }
}

// ---- composed pipeline fixture ----------------------------------------

/// The seeded-duplicate starting point the drive tests share: a repo whose
/// `src/lib.rs` gains an uncommitted `compute` function that duplicates an
/// indexed `old_compute` chunk, plus the schema-applied index seeded with both
/// chunks and a [`MockEmbedder`] over the same [`DIM`].
///
/// Composing it here keeps the seeds (file paths, symbol names, embedding) in
/// one place — a drift in any copy would silently desynchronize the tests.
/// Each test adds only its scenario-specific extras (e.g. a second working
/// file for a second validator).
pub(crate) fn seeded_dup_repo() -> (TestRepo, Connection, model_embedding::mock::MockEmbedder) {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    let dup = body("compute");
    repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

    let conn = index_conn();
    let emb = dup_emb();
    seed_chunk(&conn, "src/lib.rs", "compute", &dup, &emb);
    seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &emb);

    (repo, conn, model_embedding::mock::MockEmbedder::new(DIM))
}

/// Like [`seeded_dup_repo`] but with TWO uncommitted changed files, each gaining
/// its own duplicate function: `src/lib.rs` (`compute` ~ `old_compute`) and
/// `src/other.rs` (`render` ~ `old_render`). Both files' bodies are large enough
/// that a small `batch_size` splits the review into two batches — the fixture the
/// content-budgeted batching tests pack against.
pub(crate) fn seeded_two_file_dup_repo(
) -> (TestRepo, Connection, model_embedding::mock::MockEmbedder) {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.write("src/other.rs", "fn placeholder2() {}\n");
    repo.commit("initial");

    let lib_dup = body("compute");
    let other_dup = body("render");
    repo.write(
        "src/lib.rs",
        &format!("fn placeholder() {{}}\n\n{lib_dup}\n"),
    );
    repo.write(
        "src/other.rs",
        &format!("fn placeholder2() {{}}\n\n{other_dup}\n"),
    );

    let conn = index_conn();
    // Two ORTHOGONAL embeddings so each file duplicates only its own pair: the
    // `lib`/`other` files must not cross-reference each other (a shared embedding
    // makes every file a duplicate of every other, leaking one file's source into
    // the other's probe evidence and across batch boundaries).
    let lib_emb = dup_emb(); // unit vector on axis 0
    let mut other_emb = vec![0.0; DIM];
    other_emb[1] = 1.0; // unit vector on axis 1 — orthogonal to `lib_emb`
    seed_chunk(&conn, "src/lib.rs", "compute", &lib_dup, &lib_emb);
    seed_chunk(&conn, "src/existing.rs", "old_compute", &lib_dup, &lib_emb);
    seed_chunk(&conn, "src/other.rs", "render", &other_dup, &other_emb);
    seed_chunk(
        &conn,
        "src/existing2.rs",
        "old_render",
        &other_dup,
        &other_emb,
    );

    (repo, conn, model_embedding::mock::MockEmbedder::new(DIM))
}

/// A function body long enough to clear the default `min_chunk_bytes` (100).
pub fn body(label: &str) -> String {
    format!(
        "pub fn {label}(input: &[f64]) -> f64 {{\n    let mut total = 0.0;\n    for value in input {{\n        total += value * value;\n    }}\n    total / input.len() as f64\n}}"
    )
}

// ---- scripted ACP mock-agent harness -----------------------------------
//
// THE one scripted ACP agent the review test modules share (`fleet.rs`,
// `verify.rs`, `drive.rs`, and the pool tests in `validators/pool.rs`). It maps
// each incoming prompt onto a scripted [`ScriptedReply`] by substring match,
// delivering the reply text as a streamed `agent_message_chunk` (the shape the
// production agents emit and the pool's collector reads).
//
// Script matching runs against the session's FULL accumulated history, not the
// single request: for an ordinary one-prompt session the two are identical, and
// on a forked session the history includes the inherited prefix PLUS the
// payload — exactly the context a real agent sees. So there is no separate
// "prompt-only" matching mode; the per-module differences are all knobs on
// [`ScriptedAgentConfig`].
//
// The mock also implements the session-fork extension contract
// (`session/fork`, `session/state_status`, `session/pin`) the way the real
// agents do — per-session conversation history, a fork clones the parent's
// history under a fresh id, state-status reports `saved` once a session has
// completed a turn, and pin records/reflects the effective pin state —
// selected by [`ForkMode`] (default: [`ForkMode::Unsupported`], a backend
// without the extension).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::{
    ContentBlock, ContentChunk, InitializeResponse, NewSessionResponse, PromptRequest,
    PromptResponse, ReadTextFileRequest, SessionId, SessionNotification, SessionUpdate, StopReason,
    TextContent,
};
use agent_client_protocol::{Channel, Client, ConnectTo, ConnectionTo, Role};
use agent_client_protocol_extras::PIN_ON_SAVE_META_KEY;
use tokio::sync::broadcast;

use crate::review::fleet::PRIME_HANDOFF;
use crate::validators::{AgentPool, PoolConfig};

/// Prompt-token count the mock reports for every saved prefix state.
pub const MOCK_PREFIX_TOKENS: u64 = 1234;

/// How long a [`ScriptedReply::Stall`] turn sleeps before resolving — far longer
/// than any test's cancel/idle window, so a stalled turn is always abandoned or
/// cancelled by the test long before this elapses.
const STALL_DURATION_SECS: u64 = 60;

/// One scripted reaction, matched in script order by substring needle.
///
/// `Clone` is the cheap shallow clone (a [`ScriptedReply::Sequence`] SHARES its
/// `Arc<Mutex<VecDeque>>` so successive turns on ONE agent keep draining the same
/// queue — that shared advance is what scripts a converging follow-up loop). To
/// hand a SEPARATE agent an independent queue, use [`ScriptedReply::deep_clone`]
/// (what [`ScriptedAgent::rebind_broadcast`] does), not `Clone`.
#[derive(Debug, Clone)]
pub enum ScriptedReply {
    /// Stream this text back as an `agent_message_chunk`, then end the turn.
    Text(String),
    /// Stream successive texts on successive matches of the same needle, then
    /// stick on the last one once the sequence is drained. The follow-up sweep
    /// drives the SAME accumulating session forward with the SAME prompt every
    /// turn, so its turns all match one needle; a sequence lets a single script
    /// entry answer those turns with a different delta each time (e.g. findings,
    /// then findings, then `[]`) — the only way to script convergence when the
    /// prompt text is constant across the loop.
    Sequence(std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>),
    /// Fail the prompt with an internal error.
    Error,
    /// Wedge the turn (sleep far longer than any test window) — the shape of a
    /// hung task, used to hold a fan-out open while a test cancels it.
    Stall,
}

impl ScriptedReply {
    /// A clone that, unlike the shallow `Clone`, gives a [`ScriptedReply::Sequence`]
    /// a FRESH independent queue (the `VecDeque` contents copied into a new
    /// `Arc<Mutex<..>>`) rather than sharing the original's `Arc`.
    ///
    /// `Clone` deliberately shares the `Arc` so successive turns on ONE agent
    /// drain the same sequence (how a converging loop is scripted). But
    /// [`ScriptedAgent::rebind_broadcast`] mints a per-connection rebind the
    /// docstring promises is a "fresh agent": were it to share the `Arc`, two
    /// rebinds would pop deltas from ONE queue and one rebind's consumption would
    /// corrupt the other's, silently breaking test isolation. So a rebind
    /// deep-clones, giving every rebind an independent sequence.
    fn deep_clone(&self) -> Self {
        match self {
            ScriptedReply::Sequence(queue) => {
                let contents = queue.lock().unwrap().clone();
                ScriptedReply::Sequence(std::sync::Arc::new(std::sync::Mutex::new(contents)))
            }
            other => other.clone(),
        }
    }
}

impl ScriptedReply {
    /// A [`ScriptedReply::Sequence`] that yields `replies` in order on
    /// successive matches and then sticks on the final element once drained.
    pub fn sequence(replies: impl IntoIterator<Item = String>) -> Self {
        ScriptedReply::Sequence(std::sync::Arc::new(std::sync::Mutex::new(
            replies.into_iter().collect(),
        )))
    }
}

/// How the mock agent answers the session-fork extension surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForkMode {
    /// Fork/status/pin behave like the llama backend (state + token counts).
    Supported,
    /// `session/fork` is rejected (`fork_parent_state_unavailable`);
    /// status and pin still work.
    RejectFork,
    /// No extension method is implemented at all (`method_not_found`) — a
    /// backend without the fork extension. The default.
    Unsupported,
    /// Forks succeed but attach no parent state (degraded, cold fork).
    DegradedAttach,
}

/// The per-module knobs on the shared scripted agent. The default is the
/// plainest agent: no fork extension, replies emitted over the live
/// connection, no mid-turn agent→client round-trips, and an empty findings
/// array (`[]`) when no script entry matches.
///
/// The fields are deliberately all `pub` and the struct is deliberately NOT
/// `#[non_exhaustive]`. This is a `test-support`-gated fixture, not a public
/// API surface: its consumers — both this crate's test modules and the
/// downstream `swissarmyhammer-tools` review tests — set the knobs they care
/// about by struct literal with `..Default::default()`, across the crate
/// boundary (e.g. `review_fixture.rs`). `#[non_exhaustive]` would forbid exactly
/// that cross-crate struct-literal construction, breaking every consumer for no
/// semver benefit a test fixture can ever owe. New knobs are added here with a
/// `Default`, so existing `..Default::default()` sites keep compiling.
#[derive(Debug, Clone)]
pub struct ScriptedAgentConfig {
    /// How the session-fork extension surface answers.
    pub fork_mode: ForkMode,
    /// The reply when no script entry matches the session context.
    pub default_response: String,
    /// When set, replies are published onto this backend broadcast (the
    /// channel a driver's `notification_rx` subscribes to) instead of the
    /// live connection.
    pub broadcast: Option<broadcast::Sender<SessionNotification>>,
    /// With [`ScriptedAgentConfig::broadcast`] set, ALSO re-emit each reply
    /// over the live connection — reproducing a real handle's
    /// broadcast→connection bridge, which the driver must not collect twice.
    pub bridge_to_connection: bool,
    /// Whether every `prompt` first issues a mid-turn
    /// `session/request_permission` round-trip to the client and blocks on the
    /// answer — exactly as a real `claude` agent does for tool consent.
    pub demand_permission: bool,
    /// When set, every `prompt` first issues an `fs/read_text_file` request
    /// for this path and records the content the client returns (readable via
    /// [`ScriptedAgent::observed_read`]).
    pub read_file: Option<std::path::PathBuf>,
    /// When set, every `prompt` attaches this prompt-cache usage to its
    /// `PromptResponse._meta` (under the `cache_usage` key, as a real claude
    /// agent does), so a fleet test can exercise the warm/cold cache-usage log
    /// path without a live Anthropic backend.
    pub cache_usage: Option<claude_agent::protocol_translator::CacheUsage>,
}

impl Default for ScriptedAgentConfig {
    fn default() -> Self {
        Self {
            fork_mode: ForkMode::Unsupported,
            default_response: "[]".to_string(),
            broadcast: None,
            bridge_to_connection: false,
            demand_permission: false,
            read_file: None,
            cache_usage: None,
        }
    }
}

/// Per-session mock state: the accumulated conversation, whether a turn has
/// completed (what `saved` reports), and the effective pin state.
#[derive(Debug, Default)]
struct SessionState {
    history: String,
    completed_turns: usize,
    pinned: bool,
}

/// The shared scripted ACP agent. Construct with [`ScriptedAgent::new`] (all
/// defaults) or [`ScriptedAgent::with_config`], wire it up with
/// [`ScriptedAdapter`] (or [`with_pool`] for pool-backed tests), and probe what
/// it saw through the accessor methods.
#[derive(Debug)]
pub struct ScriptedAgent {
    next_session: AtomicUsize,
    /// (needles, reply), matched in order against the session's full
    /// conversation history: EVERY needle must be present for the entry to
    /// fire. A single-needle entry (the common case) is a one-element slice;
    /// multi-needle entries let a fan-out script key on both a validator header
    /// and a specific file/claim, which a single contiguous substring can't
    /// express.
    script: Vec<(Vec<String>, ScriptedReply)>,
    config: ScriptedAgentConfig,
    /// Every prompt seen, with the session it ran on (payload-only text for
    /// forked turns).
    seen: Mutex<Vec<(String, String)>>,
    /// Live sessions by id.
    sessions: Mutex<HashMap<String, SessionState>>,
    /// Every `session/pin` call, in order: (session id, requested pin).
    pin_calls: Mutex<Vec<(String, bool)>>,
    /// Sessions whose prefix was born pinned (saved pinned atomically at the
    /// prime turn's completion, via the `_meta` pin-on-save intent) — recorded
    /// at turn time, BEFORE any separate `session/pin` call, so a fleet test can
    /// prove the prefix is pinned through the production prime path rather than
    /// only by the post-turn confirm.
    born_pinned: Mutex<Vec<String>>,
    /// Number of successful `session/fork` calls.
    forks: AtomicUsize,
    /// Content received from `fs/read_text_file` round-trips, in order.
    observed_reads: Mutex<Vec<String>>,
}

impl ScriptedAgent {
    /// A scripted agent matching on a single substring per entry — the common
    /// case (each entry's needle is matched with `contains` against the
    /// session's accumulated context).
    pub fn new(script: impl IntoIterator<Item = (String, ScriptedReply)>) -> Arc<Self> {
        Self::with_config(script, ScriptedAgentConfig::default())
    }

    /// A scripted agent with a custom [`ScriptedAgentConfig`], matching on a
    /// single substring per entry.
    pub fn with_config(
        script: impl IntoIterator<Item = (String, ScriptedReply)>,
        config: ScriptedAgentConfig,
    ) -> Arc<Self> {
        Self::with_script(script.into_iter().map(|(n, r)| (vec![n], r)), config)
    }

    /// A scripted agent whose entries each match a SET of needles (all must be
    /// present). The general form behind [`new`](Self::new) and
    /// [`with_config`](Self::with_config); a fan-out script keys on both a
    /// validator header and a file/claim this way.
    pub fn with_script(
        script: impl IntoIterator<Item = (Vec<String>, ScriptedReply)>,
        config: ScriptedAgentConfig,
    ) -> Arc<Self> {
        Arc::new(Self {
            next_session: AtomicUsize::new(0),
            script: script.into_iter().collect(),
            config,
            seen: Mutex::new(Vec::new()),
            sessions: Mutex::new(HashMap::new()),
            pin_calls: Mutex::new(Vec::new()),
            born_pinned: Mutex::new(Vec::new()),
            forks: AtomicUsize::new(0),
            observed_reads: Mutex::new(Vec::new()),
        })
    }

    /// Clone `base`'s script and config into a fresh agent whose replies stream
    /// onto `broadcast` (and, when `bridge_to_connection`, the live connection
    /// too) — the per-connection rebind a factory needs when it mints one
    /// `AgentHandle` per review run, each with its own broadcast channel that
    /// the handle's `notification_rx` subscribes to.
    pub fn rebind_broadcast(
        base: &Arc<Self>,
        broadcast: broadcast::Sender<SessionNotification>,
        bridge_to_connection: bool,
    ) -> Arc<Self> {
        // Deep-clone each reply so a [`ScriptedReply::Sequence`] gets a FRESH
        // independent queue: the rebind is a "fresh agent", so it must not share
        // sequence consumption with `base` or with a sibling rebind (a shallow
        // `Arc` clone would).
        let script: Vec<_> = base
            .script
            .iter()
            .map(|(needles, reply)| (needles.clone(), reply.deep_clone()))
            .collect();
        Self::with_script(
            script,
            ScriptedAgentConfig {
                broadcast: Some(broadcast),
                bridge_to_connection,
                ..base.config.clone()
            },
        )
    }

    /// Lock `seen` and project each recorded `(session, prompt)` entry through
    /// `selector`, collecting the results in order. The shared lock-iterate-map-
    /// collect body behind [`seen_prompts`](Self::seen_prompts) and
    /// [`prompted_sessions`](Self::prompted_sessions), which differ only by which
    /// tuple element they take.
    fn project_seen<F>(&self, selector: F) -> Vec<String>
    where
        F: Fn(&(String, String)) -> String,
    {
        self.seen.lock().unwrap().iter().map(selector).collect()
    }

    /// The text of every prompt seen, in order.
    pub(crate) fn seen_prompts(&self) -> Vec<String> {
        self.project_seen(|(_, prompt)| prompt.clone())
    }

    /// The session each prompt ran on, in order.
    pub(crate) fn prompted_sessions(&self) -> Vec<String> {
        self.project_seen(|(session, _)| session.clone())
    }

    /// A session's accumulated conversation history — the inherited prefix plus
    /// every prompt run on it. A forward-driven follow-up loop accumulates each
    /// sweep prompt into the next session it forks, so a later sweep's history
    /// carries the sweep prompt more than once; a re-fork of the first pass would
    /// carry it only once. Returns `None` for an id that never ran.
    pub(crate) fn session_history(&self, session_id: &str) -> Option<String> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|state| state.history.clone())
    }

    /// Every `session/pin` call seen, in order, as `(session id, requested pin)`.
    pub(crate) fn pin_calls(&self) -> Vec<(String, bool)> {
        self.pin_calls.lock().unwrap().clone()
    }

    /// Sessions whose prefix was born pinned by the prime turn's `_meta`
    /// pin-on-save intent — recorded at turn completion, before any separate
    /// `session/pin` call.
    pub(crate) fn born_pinned_sessions(&self) -> Vec<String> {
        self.born_pinned.lock().unwrap().clone()
    }

    /// The number of successful `session/fork` calls served so far.
    pub(crate) fn fork_count(&self) -> usize {
        self.forks.load(Ordering::SeqCst)
    }

    /// The most recent `fs/read_text_file` content the client served.
    pub(crate) fn observed_read(&self) -> Option<String> {
        self.observed_reads.lock().unwrap().last().cloned()
    }

    /// The scripted reply for `context` — the first matching script entry, or
    /// the configured default response when nothing matches.
    fn reply_for(&self, context: &str) -> ScriptedReply {
        for (needles, reply) in &self.script {
            if needles.iter().all(|n| context.contains(n.as_str())) {
                return reply.clone();
            }
        }
        ScriptedReply::Text(self.config.default_response.clone())
    }

    /// Record one prompt against its session and return the session's
    /// accumulated conversation context — the inherited prefix PLUS the
    /// payload on a forked session, exactly what a real agent sees.
    fn record_and_context(&self, session_id: &str, prompt: &str) -> String {
        self.seen
            .lock()
            .unwrap()
            .push((session_id.to_string(), prompt.to_string()));
        let mut sessions = self.sessions.lock().unwrap();
        let state = sessions.entry(session_id.to_string()).or_default();
        state.history.push_str(prompt);
        state.history.clone()
    }

    /// Mark one completed turn on the session: it now has saved state. When
    /// `pin_on_save` is set (the prime turn's born-pinned intent, carried in the
    /// prompt's `_meta`), the saved state is born pinned — pinned atomically at
    /// save time, mirroring the llama backend's `insert_inner(.., true)`. This
    /// is what lets a fleet test assert the primed prefix is born pinned through
    /// the production path, before any separate `session/pin` lands.
    fn complete_turn(&self, session_id: &str, pin_on_save: bool) {
        if let Some(state) = self.sessions.lock().unwrap().get_mut(session_id) {
            state.completed_turns += 1;
            if pin_on_save {
                state.pinned = true;
                self.born_pinned
                    .lock()
                    .unwrap()
                    .push(session_id.to_string());
            }
        }
    }

    /// Mint the next sequential session id.
    fn next_session_id(&self) -> String {
        let n = self.next_session.fetch_add(1, Ordering::SeqCst);
        format!("sess-{n}")
    }

    /// Stream `text` back as an assistant chunk, routed per the configured
    /// emit policy: the backend broadcast when one is set (optionally bridged
    /// onto the connection too), the live connection otherwise.
    fn emit_reply(&self, cx: &ConnectionTo<Client>, session_id: &SessionId, text: String) {
        let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
            TextContent::new(text),
        )));
        let notif = SessionNotification::new(session_id.clone(), update);
        match &self.config.broadcast {
            Some(tx) => {
                let _ = tx.send(notif.clone());
                if self.config.bridge_to_connection {
                    let _ = cx.send_notification(notif);
                }
            }
            None => {
                let _ = cx.send_notification(notif);
            }
        }
    }
}

/// Adapter wiring a [`ScriptedAgent`] as an ACP server over a channel.
#[derive(Debug)]
pub struct ScriptedAdapter(Arc<ScriptedAgent>);

impl ScriptedAdapter {
    /// Wrap `agent` so it can be served as an ACP server over a channel.
    pub fn new(agent: Arc<ScriptedAgent>) -> Self {
        Self(agent)
    }

    /// The wrapped scripted agent.
    pub fn agent(&self) -> &Arc<ScriptedAgent> {
        &self.0
    }
}

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

/// Route one incoming ACP request to its handler. Each wire surface lives in
/// its own helper so this stays a flat router.
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
                let id = mock.next_session_id();
                mock.sessions
                    .lock()
                    .unwrap()
                    .insert(id.clone(), SessionState::default());
                responder
                    .cast()
                    .respond_with_result(Ok(NewSessionResponse::new(SessionId::new(id))))
            }
            Req::PromptRequest(req) => handle_prompt(&mock, req, responder, &cx).await,
            Req::ExtMethodRequest(req) => dispatch_ext(&mock, req, responder),
            _ => responder
                .cast::<serde_json::Value>()
                .respond_with_error(agent_client_protocol::Error::method_not_found()),
        }
    })
}

/// Serve one prompt request: the optional mid-turn agent→client round-trips,
/// then the scripted reply matched against the session's accumulated context.
async fn handle_prompt(
    mock: &Arc<ScriptedAgent>,
    req: PromptRequest,
    responder: agent_client_protocol::Responder<serde_json::Value>,
    cx: &ConnectionTo<Client>,
) -> agent_client_protocol::Result<()> {
    // Mid-turn, a real claude agent asks the client for tool consent via
    // `session/request_permission` and blocks on the answer before finishing
    // the turn. If the client registers no `on_receive_request` handler, this
    // never returns and the whole prompt deadlocks — the production hang the
    // drive tests reproduce.
    if mock.config.demand_permission && demand_permission(cx, &req).await.is_err() {
        return responder
            .cast::<serde_json::Value>()
            .respond_with_error(agent_client_protocol::Error::internal_error());
    }
    if let Some(path) = mock.config.read_file.clone() {
        record_fs_read(mock, cx, &req, path).await;
    }

    let prompt = prompt_text(&req);
    let session_key = req.session_id.to_string();
    let context = mock.record_and_context(&session_key, &prompt);

    // The prime turn replies "OK", as the handoff instructs. Detection keys on
    // the request's own text (a fork's payload never re-sends the handoff,
    // though its inherited context contains it).
    let reply = if prompt.contains(PRIME_HANDOFF) {
        ScriptedReply::Text("OK".to_string())
    } else {
        mock.reply_for(&context)
    };
    // `None` means the reply was [`ScriptedReply::Error`]: fail the prompt.
    let Some(text) = resolve_reply_text(mock, reply).await else {
        return responder
            .cast::<PromptResponse>()
            .respond_with_error(agent_client_protocol::Error::internal_error());
    };
    mock.emit_reply(cx, &req.session_id, text);
    mock.complete_turn(&session_key, pin_on_save(&req));
    responder
        .cast()
        .respond_with_result(Ok(build_prompt_response(mock)))
}

/// Resolve a matched [`ScriptedReply`] to the text the turn streams back.
/// Returns `None` for [`ScriptedReply::Error`] — the caller fails the prompt.
/// A [`ScriptedReply::Stall`] sleeps far past any test window before resolving.
async fn resolve_reply_text(mock: &ScriptedAgent, reply: ScriptedReply) -> Option<String> {
    match reply {
        ScriptedReply::Sequence(queue) => {
            let mut q = queue.lock().unwrap();
            // Yield the next scripted delta, sticking on the last once drained,
            // so a sequence longer than the actual turn count is safe and a
            // sequence shorter than it keeps answering with its final element.
            let text = if q.len() > 1 {
                q.pop_front().unwrap()
            } else {
                q.front()
                    .cloned()
                    .unwrap_or_else(|| mock.config.default_response.clone())
            };
            Some(text)
        }
        ScriptedReply::Error => None,
        ScriptedReply::Stall => {
            tokio::time::sleep(std::time::Duration::from_secs(STALL_DURATION_SECS)).await;
            Some(mock.config.default_response.clone())
        }
        ScriptedReply::Text(text) => Some(text),
    }
}

/// Whether a prompt carries the prime turn's born-pinned save intent in its
/// `_meta` (`PIN_ON_SAVE_META_KEY`). When set, the saved prefix is pinned
/// atomically at save time — the production prime→pin race close — rather than
/// relying on a separate post-turn pin.
fn pin_on_save(req: &PromptRequest) -> bool {
    req.meta
        .as_ref()
        .and_then(|m| m.get(PIN_ON_SAVE_META_KEY))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Build the turn's [`PromptResponse`], attaching prompt-cache usage to `_meta`
/// exactly as a real claude agent does (`agent_prompt_handling::build_streaming_response`)
/// when the config carries it, so a fleet test can drive the warm/cold
/// cache-usage log path off this mock.
fn build_prompt_response(mock: &ScriptedAgent) -> PromptResponse {
    let mut response = PromptResponse::new(StopReason::EndTurn);
    if let Some(usage) = mock.config.cache_usage {
        let mut meta = serde_json::Map::new();
        meta.insert("cache_usage".to_string(), usage.to_meta_json());
        response = response.meta(meta);
    }
    response
}

/// Issue the mid-turn `session/request_permission` round-trip a real `claude`
/// agent performs for tool consent, blocking until the client answers.
async fn demand_permission(
    cx: &ConnectionTo<Client>,
    req: &PromptRequest,
) -> Result<(), agent_client_protocol::Error> {
    use agent_client_protocol::schema::{
        RequestPermissionRequest, ToolCallUpdate, ToolCallUpdateFields,
    };
    let tool_call_update = ToolCallUpdate::new(
        agent_client_protocol::schema::ToolCallId::new("tool-read"),
        ToolCallUpdateFields::new(),
    );
    let permission_request =
        RequestPermissionRequest::new(req.session_id.clone(), tool_call_update, vec![]);
    cx.send_request(permission_request)
        .block_task()
        .await
        .map(|_| ())
        .map_err(|_| agent_client_protocol::Error::internal_error())
}

/// Issue a mid-turn `fs/read_text_file` round-trip for `path` and record the
/// content the client serves.
async fn record_fs_read(
    mock: &ScriptedAgent,
    cx: &ConnectionTo<Client>,
    req: &PromptRequest,
    path: std::path::PathBuf,
) {
    let read_request = ReadTextFileRequest::new(req.session_id.clone(), path);
    if let Ok(resp) = cx.send_request(read_request).block_task().await {
        mock.observed_reads.lock().unwrap().push(resp.content);
    }
}

/// Route one session-fork extension request ([`ForkMode::Unsupported`] answers
/// `method_not_found` for everything), each wire method in its own helper.
fn dispatch_ext(
    mock: &Arc<ScriptedAgent>,
    req: agent_client_protocol::schema::ExtRequest,
    responder: agent_client_protocol::Responder<serde_json::Value>,
) -> agent_client_protocol::Result<()> {
    use agent_client_protocol_extras::{
        SESSION_FORK_METHOD, SESSION_PIN_METHOD, SESSION_STATE_STATUS_METHOD,
    };

    if mock.config.fork_mode == ForkMode::Unsupported {
        return responder.respond_with_error(agent_client_protocol::Error::method_not_found());
    }
    let params: serde_json::Value = serde_json::from_str(req.params.get()).unwrap_or_default();
    let session_param = params["sessionId"].as_str().unwrap_or_default().to_string();

    match req.method.as_ref() {
        SESSION_STATE_STATUS_METHOD => handle_state_status(mock, &session_param, responder),
        SESSION_PIN_METHOD => handle_pin(mock, &params, &session_param, responder),
        SESSION_FORK_METHOD => handle_fork(mock, &params, responder),
        _ => responder.respond_with_error(agent_client_protocol::Error::method_not_found()),
    }
}

/// `session/state_status`: `saved` once the session has completed a turn,
/// with the mock's fixed token count.
fn handle_state_status(
    mock: &ScriptedAgent,
    session_param: &str,
    responder: agent_client_protocol::Responder<serde_json::Value>,
) -> agent_client_protocol::Result<()> {
    let sessions = mock.sessions.lock().unwrap();
    match sessions.get(session_param) {
        Some(state) if state.completed_turns > 0 => {
            responder.respond_with_result(Ok(serde_json::json!({
                "saved": true,
                "promptTokens": MOCK_PREFIX_TOKENS,
                "pinned": state.pinned,
            })))
        }
        Some(_) => responder.respond_with_result(Ok(serde_json::json!({
            "saved": false,
            "pinned": false,
        }))),
        None => responder.respond_with_error(state_not_found()),
    }
}

/// `session/pin`: record the call and reflect the effective pin state (a
/// session without a completed turn cannot be pinned).
fn handle_pin(
    mock: &ScriptedAgent,
    params: &serde_json::Value,
    session_param: &str,
    responder: agent_client_protocol::Responder<serde_json::Value>,
) -> agent_client_protocol::Result<()> {
    let requested = params["pinned"].as_bool().unwrap_or_default();
    mock.pin_calls
        .lock()
        .unwrap()
        .push((session_param.to_string(), requested));
    let mut sessions = mock.sessions.lock().unwrap();
    match sessions.get_mut(session_param) {
        Some(state) => {
            state.pinned = requested && state.completed_turns > 0;
            responder.respond_with_result(Ok(serde_json::json!({"pinned": state.pinned})))
        }
        None => responder.respond_with_error(state_not_found()),
    }
}

/// `session/fork`: clone the parent's history under a fresh session id,
/// reporting attachment per the configured [`ForkMode`].
fn handle_fork(
    mock: &ScriptedAgent,
    params: &serde_json::Value,
    responder: agent_client_protocol::Responder<serde_json::Value>,
) -> agent_client_protocol::Result<()> {
    use agent_client_protocol_extras::{FORK_PARENT_NOT_FOUND, FORK_PARENT_STATE_UNAVAILABLE};

    if mock.config.fork_mode == ForkMode::RejectFork {
        return responder.respond_with_error(
            agent_client_protocol::Error::invalid_params()
                .data(serde_json::json!({"error": FORK_PARENT_STATE_UNAVAILABLE})),
        );
    }
    let parent = params["parentSessionId"].as_str().unwrap_or_default();
    let parent_history = {
        let sessions = mock.sessions.lock().unwrap();
        match sessions.get(parent) {
            Some(state) => state.history.clone(),
            None => {
                return responder.respond_with_error(
                    agent_client_protocol::Error::invalid_params()
                        .data(serde_json::json!({"error": FORK_PARENT_NOT_FOUND})),
                )
            }
        }
    };
    let child = mock.next_session_id();
    mock.sessions.lock().unwrap().insert(
        child.clone(),
        SessionState {
            history: parent_history,
            completed_turns: 0,
            pinned: false,
        },
    );
    mock.forks.fetch_add(1, Ordering::SeqCst);
    let attached = mock.config.fork_mode == ForkMode::Supported;
    let mut value = serde_json::json!({
        "sessionId": child,
        "stateAttached": attached,
    });
    if attached {
        value["prefixTokens"] = serde_json::json!(MOCK_PREFIX_TOKENS);
    }
    responder.respond_with_result(Ok(value))
}

/// The `session_state_not_found` extension error the contract specifies.
fn state_not_found() -> agent_client_protocol::Error {
    use agent_client_protocol_extras::SESSION_STATE_NOT_FOUND;
    agent_client_protocol::Error::invalid_params()
        .data(serde_json::json!({"error": SESSION_STATE_NOT_FOUND}))
}

/// The concatenated text blocks of one prompt request.
pub fn prompt_text(req: &PromptRequest) -> String {
    req.prompt
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Run `body` against an [`AgentPool`] backed by the scripted agent.
pub(crate) async fn with_pool<F, Fut, R>(
    agent: Arc<ScriptedAgent>,
    config: PoolConfig,
    body: F,
) -> R
where
    F: FnOnce(AgentPool) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let notifier = new_notifier();
    let notifier_body = Arc::clone(&notifier);
    let (channel_a, channel_b) = Channel::duplex();

    let agent_task = tokio::spawn(async move {
        let _ = ScriptedAdapter::new(agent).connect_to(channel_a).await;
    });

    let notifier_for_handler = Arc::clone(&notifier);
    let result = Client
        .builder()
        .name("scripted-test-client")
        .on_receive_notification(
            async move |notif: SessionNotification, _cx| {
                let _ = notifier_for_handler.send_update(notif).await;
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_with(channel_b, async move |conn: ConnectionTo<_>| {
            let pool = AgentPool::new(conn, notifier_body, config);
            Ok(body(pool).await)
        })
        .await
        .expect("client connect_with failed");

    agent_task.abort();
    let _ = agent_task.await;
    result
}

/// A findings array as an agent would emit it, fenced in prose.
///
/// Binary pass/fail: a finding carries no severity field, matching the fan-out
/// output contract.
pub(crate) fn findings_json(file: &str, line: u32, rule: &str, claim: &str) -> String {
    // Built through `serde_json` so a `"` or `\` in any interpolated field is
    // escaped correctly rather than corrupting the array.
    let array = serde_json::json!([{
        "file": file,
        "line": line,
        "validator": "ignored-by-agent",
        "rule": rule,
        "claim": claim,
        "evidence": "per `duplicates`: 0.94",
        "suggestion": "extract a helper",
    }]);
    format!("Here are my findings:\n\n```json\n{array}\n```\n")
}

/// A verify verdict object as the verifier agent would emit it, fenced in
/// prose (`confirmed: true` keeps the finding, `false` refutes it).
pub(crate) fn verdict_json(confirmed: bool, reason: &str) -> String {
    // Built through `serde_json` so a `"` or `\` in `reason` is escaped correctly
    // rather than corrupting the object.
    let object = serde_json::json!({
        "confirmed": confirmed,
        "reason": reason,
    });
    format!("After trying to disprove the claim:\n\n```json\n{object}\n```\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `TestRepo::write` must confine writes under its temp dir even when handed
    /// an absolute path: `PathBuf::join` would otherwise let an absolute `rel`
    /// replace the whole path and escape the repo root.
    #[test]
    fn write_confines_an_absolute_rel_under_the_repo_root() {
        let repo = TestRepo::new();
        // A marker unique to this repo's temp dir so the escape assertion can
        // never collide with a leftover or a parallel test.
        let outside = std::env::temp_dir().join(format!(
            "test_support_escape_{}.txt",
            repo.path().file_name().unwrap().to_string_lossy()
        ));
        let _ = std::fs::remove_file(&outside);

        // An absolute path — naive `join` would write straight to `outside`.
        repo.write(outside.to_str().unwrap(), "leaked");

        assert!(
            !outside.exists(),
            "an absolute rel must not escape the repo root and write to {}",
            outside.display()
        );
    }

    /// `TestRepo::write` must reject `..` components so a relative-but-climbing
    /// path cannot escape the repo root: the climb is dropped and the file lands
    /// under the root instead of in (or above) its parent.
    #[test]
    fn write_confines_a_dotdot_rel_under_the_repo_root() {
        let repo = TestRepo::new();
        // A marker unique to this repo's temp dir, so a leftover or a parallel
        // test can never make the parent-escape assertion below flaky.
        let marker = format!(
            "escape_marker_{}.txt",
            repo.path().file_name().unwrap().to_string_lossy()
        );
        repo.write(&format!("../{marker}"), "leaked");

        // The `..` is stripped, so the file lives under the root...
        assert!(
            repo.path().join(&marker).exists(),
            "the `..` climb should be dropped, landing the file under the root"
        );
        // ...and the climb did NOT write into the temp dir's parent.
        let above = repo.path().parent().unwrap().join(&marker);
        assert!(
            !above.exists(),
            "a `..` rel must not climb out of the repo root to {}",
            above.display()
        );
    }

    /// A `claim` containing a double quote must still produce a parseable findings
    /// array — raw interpolation would corrupt the JSON.
    #[test]
    fn findings_json_escapes_a_quote_in_the_claim() {
        let raw = findings_json("a.rs", 7, "r", r#"a "quoted" claim"#);
        let json = raw
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .expect("fenced json block")
            .trim();
        let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON array");
        assert_eq!(parsed[0]["claim"], r#"a "quoted" claim"#);
    }

    /// A `reason` containing a double quote and a backslash must still produce a
    /// parseable verdict object.
    #[test]
    fn verdict_json_escapes_quote_and_backslash_in_the_reason() {
        let raw = verdict_json(false, r#"path C:\x is not a "real" bug"#);
        let json = raw
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .expect("fenced json block")
            .trim();
        let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON object");
        assert_eq!(parsed["confirmed"], false);
        assert_eq!(parsed["reason"], r#"path C:\x is not a "real" bug"#);
    }
}
