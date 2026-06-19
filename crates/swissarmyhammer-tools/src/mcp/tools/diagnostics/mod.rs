//! Operation-based `diagnostics` MCP tool.
//!
//! The pull side of the diagnostics feature, mirroring the `review` tool's
//! structure: a single op-dispatched tool that maps `op` → action, resolves the
//! scope's files, drives [`swissarmyhammer_diagnostics::diagnose`], and
//! serializes the [`DiagnosticsReport`]. No analysis logic lives here — it lives
//! in `swissarmyhammer-diagnostics` (the settle engine + sharp report) and
//! `swissarmyhammer-lsp` (the session/supervisor).
//!
//! ## Ops
//!
//! | Op | Purpose |
//! |----|---------|
//! | `check working` | Diagnose files changed vs `HEAD`. |
//! | `check file` | Diagnose a file path or glob. |
//! | `check sha` | Diagnose files in/since a commit or range. |
//! | `list servers` | Read the LSP supervisor's per-daemon status. |
//! | `get server` | One server's status, by command name. |
//!
//! The session and the code-context index (for blast radius) are obtained the
//! same way the `code_context` tool obtains them — the shared
//! [`LSP_SUPERVISOR`](crate::mcp::tools::code_context) daemon sessions and the
//! workspace DB — so this tool reuses those helpers rather than re-deriving them.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use swissarmyhammer_common::utils::find_git_repository_root_from;
use swissarmyhammer_diagnostics::{
    diagnose_with_outcome, is_diagnosable, BlastRadiusDependents, Dependents, DiagnoseOutcome,
    DiagnosticSeverity, DiagnosticsConfig, DiagnosticsReport, PrecomputedDependents, TokioTimer,
};
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_operations::{
    generate_mcp_schema, Operation, ParamMeta, ParamType, SchemaConfig,
};

use crate::mcp::op_tool_helpers::{json_result, string_arg};
use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};
use crate::mcp::tools::code_context::{
    any_lsp_session, lsp_session_for_file, open_workspace, LSP_SUPERVISOR,
};

// ---------------------------------------------------------------------------
// Shared modifier parameters (spliced into each `check` op's parameter list).
// ---------------------------------------------------------------------------

/// Severity level names, ordered most-severe first. The single source of truth
/// for the floor names: `SEVERITY_PARAM`'s `allowed_values` enum and the test
/// assertion both derive from this, and `SEVERITY_FLOOR_ORDER` pairs each of
/// these names with its [`DiagnosticSeverity`]. Adding or removing a level is a
/// one-line edit here (plus the matching `SEVERITY_FLOOR_ORDER` row).
const SEVERITY_ERROR: &str = "error";
const SEVERITY_WARNING: &str = "warning";
const SEVERITY_INFO: &str = "info";
const SEVERITY_HINT: &str = "hint";

/// The closed set of severity floor names, derived from the per-level constants.
const SEVERITY_LEVELS: &[&str] = &[
    SEVERITY_ERROR,
    SEVERITY_WARNING,
    SEVERITY_INFO,
    SEVERITY_HINT,
];

/// `severity?` — the minimum-severity floor, the enum param that carries
/// `allowed_values`.
const SEVERITY_PARAM: ParamMeta = ParamMeta::new("severity")
    .description(
        "Minimum severity to report (floor): everything at this level or worse. Defaults to `warning`.",
    )
    .param_type(ParamType::String)
    .allowed_values(SEVERITY_LEVELS);

/// `settle_ms?` — override the quiescence window in milliseconds.
const SETTLE_MS_PARAM: ParamMeta = ParamMeta::new("settle_ms")
    .description(
        "Quiescence window in milliseconds to wait for diagnostics to settle before reporting.",
    )
    .param_type(ParamType::Integer);

/// `dependents?` — whether to fold broken one-hop dependents into the report.
const DEPENDENTS_PARAM: ParamMeta = ParamMeta::new("dependents")
    .description("Fold broken one-hop dependents into the report (default true).")
    .param_type(ParamType::Boolean);

// ---------------------------------------------------------------------------
// Operations (verb + noun + parameter metadata) — schema + CLI generation.
// ---------------------------------------------------------------------------

/// `check working` — diagnose files changed vs `HEAD`.
#[derive(Debug, Default)]
pub struct CheckWorking;

static CHECK_WORKING_PARAMS: &[ParamMeta] = &[SEVERITY_PARAM, SETTLE_MS_PARAM, DEPENDENTS_PARAM];

impl Operation for CheckWorking {
    fn verb(&self) -> &'static str {
        "check"
    }
    fn noun(&self) -> &'static str {
        "working"
    }
    fn description(&self) -> &'static str {
        "Diagnose the files changed vs HEAD (uncommitted working-tree changes)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        CHECK_WORKING_PARAMS
    }
}

/// `check file` — diagnose an explicit file path or glob.
#[derive(Debug, Default)]
pub struct CheckFile;

static CHECK_FILE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("path")
        .description("A file path or glob to diagnose.")
        .param_type(ParamType::String)
        .required(),
    SEVERITY_PARAM,
    SETTLE_MS_PARAM,
    DEPENDENTS_PARAM,
];

impl Operation for CheckFile {
    fn verb(&self) -> &'static str {
        "check"
    }
    fn noun(&self) -> &'static str {
        "file"
    }
    fn description(&self) -> &'static str {
        "Diagnose an explicit file path or glob"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        CHECK_FILE_PARAMS
    }
}

/// `check sha` — diagnose files touched in/since a commit or range.
#[derive(Debug, Default)]
pub struct CheckSha;

static CHECK_SHA_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("sha")
        .description(
            "A commit or range (e.g. `HEAD~3..HEAD` or a single ref treated as `ref..HEAD`).",
        )
        .param_type(ParamType::String)
        .required(),
    SEVERITY_PARAM,
    SETTLE_MS_PARAM,
    DEPENDENTS_PARAM,
];

impl Operation for CheckSha {
    fn verb(&self) -> &'static str {
        "check"
    }
    fn noun(&self) -> &'static str {
        "sha"
    }
    fn description(&self) -> &'static str {
        "Diagnose the files touched in/since a commit or range"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        CHECK_SHA_PARAMS
    }
}

/// `list servers` — read the LSP supervisor's per-daemon status.
#[derive(Debug, Default)]
pub struct ListServers;

impl Operation for ListServers {
    fn verb(&self) -> &'static str {
        "list"
    }
    fn noun(&self) -> &'static str {
        "servers"
    }
    fn description(&self) -> &'static str {
        "List the managed LSP servers and their status (no analysis)"
    }
}

/// `get server` — one server's status, by command name.
#[derive(Debug, Default)]
pub struct GetServer;

static GET_SERVER_PARAMS: &[ParamMeta] = &[ParamMeta::new("command")
    .description("The LSP server command name to report (e.g. `rust-analyzer`).")
    .param_type(ParamType::String)
    .required()];

impl Operation for GetServer {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "server"
    }
    fn description(&self) -> &'static str {
        "Get one LSP server's status by command name (no analysis)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_SERVER_PARAMS
    }
}

static CHECK_WORKING: Lazy<CheckWorking> = Lazy::new(CheckWorking::default);
static CHECK_FILE: Lazy<CheckFile> = Lazy::new(CheckFile::default);
static CHECK_SHA: Lazy<CheckSha> = Lazy::new(CheckSha::default);
static LIST_SERVERS: Lazy<ListServers> = Lazy::new(ListServers::default);
static GET_SERVER: Lazy<GetServer> = Lazy::new(GetServer::default);

/// Every operation the `diagnostics` tool exposes, in dispatch order.
pub static DIAGNOSTICS_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*CHECK_WORKING as &dyn Operation,
        &*CHECK_FILE as &dyn Operation,
        &*CHECK_SHA as &dyn Operation,
        &*LIST_SERVERS as &dyn Operation,
        &*GET_SERVER as &dyn Operation,
    ]
});

// ---------------------------------------------------------------------------
// The tool.
// ---------------------------------------------------------------------------

/// The scope of a `check` op: which files to diagnose.
enum Scope {
    /// Files changed vs `HEAD` in the working tree.
    Working,
    /// An explicit file path or glob.
    File(String),
    /// Files touched in/since a commit or range.
    Sha(String),
}

/// The operation-based `diagnostics` MCP tool.
#[derive(Debug, Default)]
pub struct DiagnosticsTool;

impl DiagnosticsTool {
    /// Build the tool.
    pub fn new() -> Self {
        Self
    }

    /// Resolve the repository root from the session work-dir (never a stray
    /// `current_dir()` when a work-dir is set), matching the `review` tool.
    ///
    /// The result is **canonicalized** (symlinks resolved). The absolute paths
    /// this root produces (via `absolutize`) are keyed against the diagnostics
    /// the LSP server publishes, and a server like `rust-analyzer` canonicalizes
    /// its `file://` uris (on macOS `/var` → `/private/var`). If the root were the
    /// raw symlink path, `repo.join(rel)` would not match the server's canonical
    /// uri and a target file's own diagnostics would silently never be folded in.
    /// Canonicalizing here keeps both path spaces in the server's canonical form.
    /// Falls back to the non-canonical root if canonicalization fails (e.g. the
    /// path does not exist).
    fn repo_root(&self, context: &ToolContext) -> PathBuf {
        let working_dir = context
            .working_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let root = find_git_repository_root_from(&working_dir).unwrap_or(working_dir);
        std::fs::canonicalize(&root).unwrap_or(root)
    }

    /// Resolve a scope to the **absolute** diagnosable files it covers (deduped,
    /// in a stable order). Non-diagnosable files (`.md`, `.txt`, …) are dropped.
    ///
    /// Paths are normalised to absolute (against `repo`) because that is the
    /// space the LSP side needs: the server publishes diagnostics under absolute
    /// `file://` URIs, `diagnose` reads files from disk, and the report keys
    /// records by the URI-derived path. Git returns repo-relative paths, so they
    /// must be joined onto the repo root here.
    fn resolve_paths(&self, scope: Scope, repo: &Path) -> Result<Vec<String>, rmcp::ErrorData> {
        let files = match scope {
            Scope::Working => {
                let git = open_git(repo)?;
                let status = git.get_status().map_err(git_err)?;
                // Modified/new/renamed files can carry diagnostics; deletions
                // cannot, so they are intentionally excluded.
                let mut files = Vec::new();
                files.extend(status.staged_modified);
                files.extend(status.unstaged_modified);
                files.extend(status.untracked);
                files.extend(status.staged_new);
                files.extend(status.renamed);
                files
            }
            Scope::Sha(range) => {
                let git = open_git(repo)?;
                git.get_changed_files_from_range(&range).map_err(git_err)?
            }
            Scope::File(target) => expand_file_target(&target, repo),
        };

        let mut seen = HashSet::new();
        Ok(files
            .into_iter()
            .map(|f| absolutize(&f, repo))
            // Confine every resolved path to the repository: a `check file`
            // target like `../../etc/x.rs` must not escape the repo and have its
            // contents read. The check is lexical (resolves `..` without touching
            // the filesystem), so it is symlink-safe and needs no `canonicalize`.
            .filter(|f| is_within_repo(f, repo))
            .filter(|f| seen.insert(f.clone()))
            .filter(|f| is_diagnosable(f))
            .collect())
    }

    /// Dispatch a `check` op: resolve files, drive `diagnose`, serialize.
    async fn execute_check(
        &self,
        scope: Scope,
        args: &serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let config = config_from_args(args);
        let repo = self.repo_root(context);
        let paths = self.resolve_paths(scope, &repo)?;

        let outcome = produce_outcome(&paths, &repo, context, &config).await;
        json_result(&outcome.report)
    }
}

/// Resolve the live session and blast-radius dependents for `paths`, then drive
/// [`diagnose_with_outcome`] — the shared report-producing core used by both the
/// `diagnostics` tool's `check` ops and the inline-on-edit fold-in.
///
/// `paths` must already be **absolute** diagnosable paths (the caller does the
/// scope resolution / `is_diagnosable` filtering). `repo` is the repository root
/// the code-context index keys symbols against. When nothing is diagnosable or no
/// language server is available, returns a settled empty outcome rather than an
/// error.
///
/// The blast radius is resolved up front and its DB handle dropped before the
/// `.await`: a `rusqlite::Connection` is `!Sync` and must not be held across an
/// await. The code-context index keys symbols by **repo-relative** path while
/// `diagnose` works in **absolute** space, so each target is relativised for the
/// lookup and the returned dependents absolutised back.
pub(crate) async fn produce_outcome(
    paths: &[String],
    repo: &Path,
    context: &ToolContext,
    config: &DiagnosticsConfig,
) -> DiagnoseOutcome {
    if paths.is_empty() {
        return settled_empty();
    }
    let session = paths
        .iter()
        .find_map(|p| lsp_session_for_file(p))
        .or_else(any_lsp_session);
    let Some(session) = session else {
        return settled_empty();
    };

    let dependents = if config.include_dependents {
        match open_workspace(context) {
            Ok(workspace) => {
                let db = workspace.db();
                let resolver = BlastRadiusDependents::new(&db);
                let map = paths
                    .iter()
                    .map(|abs| {
                        let deps = resolver
                            .one_hop(&relativize(abs, repo))
                            .iter()
                            .map(|rel| absolutize(rel, repo))
                            .collect();
                        (abs.clone(), deps)
                    })
                    .collect();
                PrecomputedDependents::new(map)
            }
            Err(_) => PrecomputedDependents::default(),
        }
    } else {
        PrecomputedDependents::default()
    };

    diagnose_with_outcome(&session, paths, config, &dependents, &TokioTimer).await
}

/// A settled outcome carrying an empty report (nothing diagnosable / no server).
fn settled_empty() -> DiagnoseOutcome {
    DiagnoseOutcome {
        report: DiagnosticsReport::new(Vec::new()),
        pending: false,
    }
}

crate::impl_default_doctorable!(DiagnosticsTool);
crate::impl_empty_initializable!(DiagnosticsTool);

#[async_trait]
impl McpTool for DiagnosticsTool {
    fn name(&self) -> &'static str {
        "diagnostics"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        let config = SchemaConfig::new(
            "LSP diagnostics over working/file/sha scopes, plus server status, dispatched by `op`.",
        );
        generate_mcp_schema(&DIAGNOSTICS_OPERATIONS, config)
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("diagnostics")
    }

    fn operations(&self) -> &'static [&'static dyn Operation] {
        let ops: &[&'static dyn Operation] = &DIAGNOSTICS_OPERATIONS;
        ops
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        // A missing or empty `op` defaults to `check working`.
        let op_str = arguments
            .get("op")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("check working");

        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            "check working" => self.execute_check(Scope::Working, &args, context).await,
            "check file" => {
                let path = string_arg(&args, "path").ok_or_else(|| {
                    rmcp::ErrorData::invalid_params(
                        "`check file` requires a `path` (a file path or glob)",
                        None,
                    )
                })?;
                self.execute_check(Scope::File(path), &args, context).await
            }
            "check sha" => {
                let sha = string_arg(&args, "sha").ok_or_else(|| {
                    rmcp::ErrorData::invalid_params(
                        "`check sha` requires a `sha` (a commit or range)",
                        None,
                    )
                })?;
                self.execute_check(Scope::Sha(sha), &args, context).await
            }
            "list servers" => json_result(&server_statuses().await),
            "get server" => {
                let command = string_arg(&args, "command").ok_or_else(|| {
                    rmcp::ErrorData::invalid_params(
                        "`get server` requires a `command` (the LSP server command name)",
                        None,
                    )
                })?;
                let found = server_statuses()
                    .await
                    .into_iter()
                    .find(|s| s.command == command);
                match found {
                    Some(status) => json_result(&status),
                    None => Err(rmcp::ErrorData::invalid_params(
                        format!("no managed LSP server with command '{command}'"),
                        None,
                    )),
                }
            }
            other => {
                let valid = DIAGNOSTICS_OPERATIONS
                    .iter()
                    .map(|op| op.op_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(rmcp::ErrorData::invalid_params(
                    format!("Unknown operation '{other}'. Valid operations: {valid}"),
                    None,
                ))
            }
        }
    }
}

/// Snapshot every managed LSP server's status (empty when the supervisor has not
/// been initialised, e.g. in a bare test registry).
async fn server_statuses() -> Vec<swissarmyhammer_lsp::DaemonStatus> {
    match LSP_SUPERVISOR.get() {
        Some(supervisor) => supervisor.lock().await.status(),
        None => Vec::new(),
    }
}

/// Open a [`GitOperations`] rooted at `repo`.
fn open_git(repo: &Path) -> Result<GitOperations, rmcp::ErrorData> {
    GitOperations::with_work_dir(repo).map_err(|e| {
        rmcp::ErrorData::internal_error(format!("failed to open git repository: {e}"), None)
    })
}

/// Map a git error into an MCP error.
fn git_err(e: impl std::fmt::Display) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(format!("git scoping failed: {e}"), None)
}

/// Expand a `check file` target: a glob (when it has glob metacharacters) is
/// matched relative to the repo root; otherwise the literal path is used.
fn expand_file_target(target: &str, repo: &Path) -> Vec<String> {
    if !target.contains(['*', '?', '[']) {
        return vec![target.to_string()];
    }
    let pattern = if Path::new(target).is_absolute() {
        target.to_string()
    } else {
        repo.join(target).to_string_lossy().into_owned()
    };
    match glob::glob(&pattern) {
        Ok(paths) => paths
            .filter_map(Result::ok)
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Make `path` a clean absolute path against `repo`.
///
/// `diagnose` and the LSP server work in absolute-path space; git scoping yields
/// repo-relative paths, so they are joined onto the repo root. `..`/`.` are
/// resolved lexically so the result is the canonical form the server publishes
/// diagnostics under (a stray `src/../src/x.rs` would otherwise not match). An
/// already-absolute `path` replaces the root (standard `Path::join` semantics).
fn absolutize(path: &str, repo: &Path) -> String {
    lexically_normalize(&repo.join(path))
        .to_string_lossy()
        .into_owned()
}

/// Express an absolute `path` relative to `repo`, the space the code-context
/// index keys symbols by. A path that is not under `repo` is returned unchanged.
fn relativize(path: &str, repo: &Path) -> String {
    Path::new(path)
        .strip_prefix(repo)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

/// Whether `path` lies within `repo` after lexically resolving `.`/`..`
/// components. Purely lexical — no filesystem access and no symlink resolution —
/// so it cannot be fooled by a non-existent path and won't reject a legitimately
/// symlinked repo root.
fn is_within_repo(path: &str, repo: &Path) -> bool {
    lexically_normalize(Path::new(path)).starts_with(lexically_normalize(repo))
}

/// Resolve `.`/`..` components in `path` without touching the filesystem.
fn lexically_normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Build a [`DiagnosticsConfig`] from the shared modifier args, leaving every
/// unset knob at its default.
fn config_from_args(args: &serde_json::Map<String, serde_json::Value>) -> DiagnosticsConfig {
    let mut config = DiagnosticsConfig::default();
    if let Some(floor) = string_arg(args, "severity") {
        config.severities = severities_at_or_above(&floor);
    }
    if let Some(ms) = args.get("settle_ms").and_then(|v| v.as_u64()) {
        config.settle_window = Duration::from_millis(ms);
    }
    if let Some(include) = args.get("dependents").and_then(|v| v.as_bool()) {
        config.include_dependents = include;
    }
    config
}

/// Severity floors, ordered most-severe first. A floor selects itself and every
/// severity above it — i.e. the prefix of this table up to and including the
/// named floor — so the "what does this floor include" rule is data, not branches.
const SEVERITY_FLOOR_ORDER: &[(&str, DiagnosticSeverity)] = &[
    (SEVERITY_ERROR, DiagnosticSeverity::Error),
    (SEVERITY_WARNING, DiagnosticSeverity::Warning),
    (SEVERITY_INFO, DiagnosticSeverity::Info),
    (SEVERITY_HINT, DiagnosticSeverity::Hint),
];

/// The floor used when the requested one is unrecognised (`warning`).
const DEFAULT_FLOOR_INDEX: usize = 1;

/// Expand a minimum-severity floor into the set of severities at or above it.
///
/// Unknown values fall back to the default (`error` + `warning`).
fn severities_at_or_above(floor: &str) -> Vec<DiagnosticSeverity> {
    let floor = floor.to_ascii_lowercase();
    let cutoff = SEVERITY_FLOOR_ORDER
        .iter()
        .position(|(name, _)| *name == floor)
        .unwrap_or(DEFAULT_FLOOR_INDEX);
    SEVERITY_FLOOR_ORDER[..=cutoff]
        .iter()
        .map(|(_, severity)| *severity)
        .collect()
}

/// Register the operation-based `diagnostics` tool with the registry.
pub fn register_diagnostics_tools(registry: &mut ToolRegistry) {
    registry.register(DiagnosticsTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::mcp::tool_handlers::ToolHandlers;
    use crate::mcp::tool_registry::ToolContext;

    fn tool() -> DiagnosticsTool {
        DiagnosticsTool::new()
    }

    /// A minimal context (the tool reads only `working_dir`; it builds its own
    /// `GitOperations` and never touches `git_ops`).
    fn context() -> ToolContext {
        let git_ops = Arc::new(tokio::sync::Mutex::new(None));
        let tool_handlers = Arc::new(ToolHandlers::new());
        let agent_config = Arc::new(swissarmyhammer_config::ModelConfig::default());
        ToolContext::new(tool_handlers, git_ops, agent_config)
    }

    fn args(pairs: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        pairs.as_object().unwrap().clone()
    }

    /// A context whose `working_dir` is `dir`.
    fn context_in(dir: PathBuf) -> ToolContext {
        let git_ops = Arc::new(tokio::sync::Mutex::new(None));
        let tool_handlers = Arc::new(ToolHandlers::new());
        let agent_config = Arc::new(swissarmyhammer_config::ModelConfig::default());
        ToolContext::new(tool_handlers, git_ops, agent_config).with_working_dir(dir)
    }

    #[test]
    fn repo_root_canonicalizes_a_symlinked_working_dir() {
        // The diagnostics tool keys blast-radius absolutes against the LSP
        // server's canonicalized `file://` uris. A repo root reached through a
        // symlink (the macOS tempdir case: `/var` -> `/private/var`) would make
        // `repo.join(rel)` mismatch the server's canonical uri and silently drop
        // a target's own diagnostics. `repo_root` must therefore canonicalize.
        let real = tempfile::tempdir().expect("real repo dir");
        let real_root = real.path();
        // Mark it a git repo root so find_git_repository_root_from stops here.
        std::fs::create_dir_all(real_root.join(".git")).unwrap();

        // A symlink that points at the real repo root, standing in for a
        // non-canonical path the server would resolve through.
        let link_parent = tempfile::tempdir().expect("link parent");
        let link = link_parent.path().join("repo-link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(real_root, &link).unwrap();
        #[cfg(not(unix))]
        std::os::windows::fs::symlink_dir(real_root, &link).unwrap();

        let resolved = tool().repo_root(&context_in(link.clone()));

        // The resolved root must be the canonical (symlink-free) path — equal to
        // canonicalizing the real root, and never the raw symlink path.
        assert_eq!(
            resolved,
            std::fs::canonicalize(real_root).unwrap(),
            "repo_root must resolve symlinks to the server's canonical form"
        );
        assert_ne!(
            resolved, link,
            "repo_root must not return the raw symlink path"
        );
    }

    #[test]
    fn tool_advertises_all_five_ops() {
        let mut registry = ToolRegistry::new();
        register_diagnostics_tools(&mut registry);
        let registered = registry
            .get_tool("diagnostics")
            .expect("diagnostics tool registered");
        let ops: Vec<String> = registered
            .operations()
            .iter()
            .map(|o| o.op_string())
            .collect();
        for expected in [
            "check working",
            "check file",
            "check sha",
            "list servers",
            "get server",
        ] {
            assert!(ops.iter().any(|s| s == expected), "missing op `{expected}`");
        }
    }

    #[tokio::test]
    async fn unknown_op_is_rejected() {
        let err = tool()
            .execute(args(serde_json::json!({"op": "frobnicate"})), &context())
            .await
            .expect_err("unknown op must error");
        assert!(err.message.contains("Unknown operation"));
    }

    #[tokio::test]
    async fn check_file_without_path_errors() {
        let err = tool()
            .execute(args(serde_json::json!({"op": "check file"})), &context())
            .await
            .expect_err("check file needs a path");
        assert!(err.message.contains("path"));
    }

    #[tokio::test]
    async fn check_sha_without_sha_errors() {
        let err = tool()
            .execute(args(serde_json::json!({"op": "check sha"})), &context())
            .await
            .expect_err("check sha needs a sha");
        assert!(err.message.contains("sha"));
    }

    #[tokio::test]
    async fn get_server_without_command_errors() {
        let err = tool()
            .execute(args(serde_json::json!({"op": "get server"})), &context())
            .await
            .expect_err("get server needs a command");
        assert!(err.message.contains("command"));
    }

    #[tokio::test]
    async fn list_servers_dispatches_to_an_empty_report() {
        // Routes to the list-servers arm and returns a result (empty without a
        // supervisor), not an "unknown operation" error.
        let result = tool()
            .execute(args(serde_json::json!({"op": "list servers"})), &context())
            .await
            .expect("list servers should succeed");
        assert!(!result.is_error.unwrap_or(false));
    }

    #[test]
    fn schema_exposes_all_ops_and_severity_enum() {
        let schema = tool().schema();
        let ops = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op enum");
        for expected in [
            "check working",
            "check file",
            "check sha",
            "list servers",
            "get server",
        ] {
            assert!(ops.iter().any(|v| v == expected), "missing op {expected}");
        }
        // The severity modifier carries the allowed_values enum — derived from
        // the floor-order table so this assertion can never drift from the
        // single source of truth for the level names.
        let expected_levels: Vec<&str> =
            SEVERITY_FLOOR_ORDER.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            schema["properties"]["severity"]["enum"],
            serde_json::json!(expected_levels)
        );
    }

    #[test]
    fn severity_floor_widens_downward() {
        use DiagnosticSeverity::{Error, Hint, Info, Warning};
        assert_eq!(severities_at_or_above("error"), vec![Error]);
        assert_eq!(severities_at_or_above("warning"), vec![Error, Warning]);
        assert_eq!(severities_at_or_above("info"), vec![Error, Warning, Info]);
        assert_eq!(
            severities_at_or_above("hint"),
            vec![Error, Warning, Info, Hint]
        );
        // Unknown floors fall back to the default.
        assert_eq!(severities_at_or_above("bogus"), vec![Error, Warning]);
    }

    #[test]
    fn config_from_args_applies_overrides() {
        // The settle override fed as input and asserted as the expected
        // `Duration` — one constant keeps the two synchronized.
        const TEST_SETTLE_MS: u64 = 1500;
        let args = serde_json::json!({
            "severity": "error",
            "settle_ms": TEST_SETTLE_MS,
            "dependents": false
        })
        .as_object()
        .unwrap()
        .clone();
        let config = config_from_args(&args);
        assert_eq!(config.severities, vec![DiagnosticSeverity::Error]);
        assert_eq!(config.settle_window, Duration::from_millis(TEST_SETTLE_MS));
        assert!(!config.include_dependents);
    }

    #[test]
    fn config_from_args_defaults_when_absent() {
        let args = serde_json::Map::new();
        let config = config_from_args(&args);
        assert_eq!(config, DiagnosticsConfig::default());
    }

    #[test]
    fn expand_file_target_passes_through_plain_path() {
        let repo = Path::new("/repo");
        assert_eq!(
            expand_file_target("src/main.rs", repo),
            vec!["src/main.rs".to_string()]
        );
    }

    #[test]
    fn absolutize_and_relativize_bridge_the_path_spaces() {
        let repo = Path::new("/repo");
        // Relative -> absolute for the LSP side; absolute is left alone.
        assert_eq!(absolutize("src/main.rs", repo), "/repo/src/main.rs");
        assert_eq!(absolutize("/already/abs.rs", repo), "/already/abs.rs");
        // Absolute -> repo-relative for the code-context index; outside-repo
        // paths pass through.
        assert_eq!(relativize("/repo/src/main.rs", repo), "src/main.rs");
        assert_eq!(relativize("/outside/x.rs", repo), "/outside/x.rs");
        // Round trip.
        assert_eq!(
            relativize(&absolutize("src/lib.rs", repo), repo),
            "src/lib.rs"
        );
    }

    #[test]
    fn resolve_paths_file_scope_is_absolute_and_diagnosable_filtered() {
        let repo = Path::new("/repo");
        // A diagnosable relative path is absolutized against the repo root.
        let paths = tool()
            .resolve_paths(Scope::File("src/main.rs".into()), repo)
            .expect("resolve");
        assert_eq!(paths, vec!["/repo/src/main.rs".to_string()]);
        // A non-diagnosable file is dropped.
        let none = tool()
            .resolve_paths(Scope::File("README.md".into()), repo)
            .expect("resolve");
        assert!(none.is_empty());
    }

    #[test]
    fn resolve_paths_rejects_traversal_outside_the_repo() {
        let repo = Path::new("/repo");
        // `..` escaping the repo is dropped even for a diagnosable extension.
        let escaped = tool()
            .resolve_paths(Scope::File("../../etc/evil.rs".into()), repo)
            .expect("resolve");
        assert!(
            escaped.is_empty(),
            "traversal outside the repo must be dropped"
        );
        // An absolute path outside the repo is also dropped.
        let outside = tool()
            .resolve_paths(Scope::File("/etc/evil.rs".into()), repo)
            .expect("resolve");
        assert!(
            outside.is_empty(),
            "out-of-repo absolute path must be dropped"
        );
        // A normal in-repo path with a redundant `..` still resolves and stays.
        let inside = tool()
            .resolve_paths(Scope::File("src/../src/main.rs".into()), repo)
            .expect("resolve");
        assert_eq!(inside, vec!["/repo/src/main.rs".to_string()]);
    }

    #[test]
    fn is_within_repo_is_lexical() {
        let repo = Path::new("/repo");
        assert!(is_within_repo("/repo/src/main.rs", repo));
        assert!(is_within_repo("/repo/a/../b.rs", repo));
        assert!(!is_within_repo("/repo/../etc/passwd", repo));
        assert!(!is_within_repo("/etc/passwd", repo));
    }

    #[tokio::test]
    async fn list_servers_is_empty_without_a_supervisor() {
        // No supervisor initialised in a bare test process → empty, not an error.
        let statuses = server_statuses().await;
        assert!(statuses.is_empty());
    }
}
