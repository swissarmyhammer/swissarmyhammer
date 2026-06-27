//! Engine driver — wire a live ACP agent into the `expect` pipeline.
//!
//! `expect` borrows an agent to *reason about and act on* the system under test
//! (`ideas/expect.md` §"Delegation over ACP"), while the verdict stays
//! deterministic and inside `expect`. This module owns the one piece of
//! choreography that needs a live connection — standing an
//! [`AgentPool`](swissarmyhammer_validators::AgentPool) up over an ACP agent and
//! driving one scoped subagent per expectation goal — and is the mirror of
//! `swissarmyhammer-validators`' `review::drive::run_review_over_agent`. The
//! review machinery is reused, not re-derived: the same [`AgentPool`], the same
//! `TolerantResponseRouter`, and the same tolerant
//! [`extract_json_value`](swissarmyhammer_validators::review::extract_json_value)
//! structured-output extractor.
//!
//! [`run_expect_over_agent`] takes the two halves of an ACP agent handle (the
//! [`DynConnectTo<Client>`] component and a `broadcast::Receiver` of the agent's
//! streamed `session/update` notifications), so this crate constructs no agent
//! itself — the tool layer injects a ready handle behind its pipeline gate. The
//! engine therefore stays agent-construction-free.
//!
//! # Single notification path
//!
//! The pool's per-prompt collectors are fed from exactly ONE source: the agent's
//! own `notification_rx` broadcast, drained by [`forward_notifications`] into the
//! pool's [`NotificationSender`](claude_agent::NotificationSender). For a real
//! `swissarmyhammer_agent::AcpAgentHandle`, `notification_rx` is a `subscribe()`
//! of the backend's broadcast channel that the handle ALSO bridges onto the
//! connection. Because that bridge re-emits the very same notifications onto the
//! connection, the driver must NOT also forward what the connection re-emits —
//! doing so delivers every streamed chunk twice and
//! [`collect_response_content`](claude_agent::collect_response_content) would
//! concatenate the reply twice, corrupting the JSON the structured-output parser
//! reads. Forwarding solely from `notification_rx` keeps delivery single-path.
//!
//! # Tamper-resistance
//!
//! The driving agent may read repo files (confined under the repo root) and is
//! auto-granted permission, but it MUST NOT edit the ledger it is being graded
//! against. [`answer_agent_request`] therefore DENIES any `fs/write_text_file`
//! that resolves under the repo's `.expect/` directory — specs, goldens, and
//! received fixtures are off-limits — while acking writes elsewhere.

use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::{
    AgentRequest, ClientCapabilities, FileSystemCapabilities, InitializeRequest,
    PermissionOptionId, ReadTextFileResponse, RequestPermissionOutcome, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionNotification, WriteTextFileResponse,
};
use agent_client_protocol::{Client, ConnectionTo, DynConnectTo, Responder};
use agent_client_protocol_extras::TolerantResponseRouter;
use tokio::sync::broadcast;

use swissarmyhammer_validators::review::extract_json_value;
use swissarmyhammer_validators::{AgentPool, PoolConfig};

use crate::config::EXPECT_DIR;
use crate::error::ExpectError;
use crate::observe::{observe, ObserveConfig};
use crate::spec::{parse_criterion, Expectation, Section};
use crate::surface::SurfaceAdapter;
use crate::types::Observation;

/// The set of goals to drive, one scoped subagent per goal.
///
/// This is the resolved-scope input to [`run_expect_over_agent`]: each goal is
/// the prompt that drives one expectation's subagent (open a scoped session,
/// send the goal, drain `session/update`, capture the forced structured output).
/// The richer scope-resolution that maps expectation specs onto these goals
/// lands with the observe-over-agent pipeline; this seam takes the goals it is
/// handed.
#[derive(Debug, Clone, Default)]
pub struct ExpectScope {
    /// The per-expectation goals to drive, in order.
    pub goals: Vec<String>,
}

/// The structured capture from one driven expectation subagent.
///
/// ACP's prompt turn returns only a control signal (`stopReason`), not a
/// payload, so the structured result is assembled here from the subagent's
/// reply: [`extract_json_value`](swissarmyhammer_validators::review::extract_json_value)
/// strips any fences and the JSON object is parsed into [`Self::structured`].
#[derive(Debug, Clone)]
pub struct DrivenObservation {
    /// The goal the subagent was driven with (its identity in the scope).
    pub goal: String,
    /// The tolerant-extracted structured JSON the subagent produced.
    pub structured: serde_json::Value,
}

/// The preamble that frames a withheld-criteria goal for the driving subagent.
///
/// It states the discipline explicitly: the subagent is given the intent and the
/// `Given`/`When` steps but **not** the acceptance criteria, so it cannot
/// optimize to the rubric (`ideas/expect.md` §"The Check Loop", hardening rule 1
/// — the SWE-bench held-out-test discipline).
const DRIVER_GOAL_PREAMBLE: &str = "You are driving a system under test toward a goal. \
Explore and act to accomplish the intended behavior described below. You are deliberately NOT \
given the acceptance criteria — focus on achieving the behavior itself, not on passing a checklist.";

/// The forced structured-output instruction appended to every driver goal.
///
/// ACP's prompt turn carries no structured payload, so the `StructuredOutput`
/// contract is implemented as a prompt-for-JSON: the subagent is asked to end its
/// turn with a single JSON object, which [`drive_scope`] recovers with the
/// tolerant [`extract_json_value`] even when the model fences or prefaces it.
const DRIVER_STRUCTURED_OUTPUT_INSTRUCTION: &str =
    "When you are finished, report your result as a \
single JSON object and nothing else, for example {\"summary\": \"<what you did>\", \"actions\": \
[\"<each concrete action you took>\"]}.";

/// The [`Trajectory`](crate::types::Trajectory) step prefix recording the driving
/// subagent's structured self-report.
///
/// The captured JSON is a *claim* — the subagent's own account of what it did —
/// kept for triage, never the verdict source (the adapter's authoritative
/// checkpoints remain ground truth, per `ideas/expect.md` §"The Check Loop",
/// hardening rule 2).
const CLAIM_STEP_PREFIX: &str = "claim: ";

/// Assemble the goal prompt that drives one expectation's subagent, **withholding
/// the acceptance criteria**.
///
/// This is the prompt-assembly split that is the main defense against
/// reward-hacking (`ideas/expect.md` §"The Check Loop", hardening rule 1): the
/// goal carries the body's stated intent plus the `## Given` and `## When`
/// sections, but the `## Then` checklist — and any stray GFM criterion item — and
/// the `## Notes` right-reason text are routed to the grader, never to the driver.
/// An agent that cannot see the rubric cannot optimize to it.
///
/// The split is enforced by construction: this function never reads
/// [`Expectation::criteria`] or [`Expectation::notes`], and it drops the
/// `Then`/`Notes` sections (and any checklist line) while walking the body, so a
/// criterion can only reach the driver if it is neither in a `Then`/`Notes`
/// section nor formatted as a checklist item. The result is framed with
/// [`DRIVER_GOAL_PREAMBLE`] and closed with the forced
/// [`DRIVER_STRUCTURED_OUTPUT_INSTRUCTION`].
pub fn build_driver_goal(expectation: &Expectation) -> String {
    let body = withhold_criteria(&expectation.intent);
    format!("{DRIVER_GOAL_PREAMBLE}\n\n{body}\n\n{DRIVER_STRUCTURED_OUTPUT_INSTRUCTION}")
}

/// Return the expectation body with the `Then` checklist and `Notes` section
/// withheld — the intent narrative plus the `Given`/`When` sections only.
///
/// Walks the body once, reusing [`Section`] heading detection and
/// [`parse_criterion`] so the withholding rule matches the parser's own
/// section/criteria recognition rather than re-deriving it. Every line inside a
/// `Then` or `Notes` section is dropped (including the heading), and any GFM
/// checklist item is dropped wherever it appears, so a spec that lists criteria
/// without a `## Then` header still has them withheld.
fn withhold_criteria(body: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut section = Section::None;
    for line in body.lines() {
        if let Some(heading) = Section::from_heading(line) {
            section = heading;
            // Keep the Given/When headings; drop the withheld Then/Notes headings.
            if !matches!(section, Section::Then | Section::Notes) {
                kept.push(line);
            }
            continue;
        }
        // Withhold everything inside a Then/Notes section, and any checklist item
        // (the acceptance criteria) wherever it appears in the body.
        if matches!(section, Section::Then | Section::Notes) || parse_criterion(line).is_some() {
            continue;
        }
        kept.push(line);
    }
    kept.join("\n").trim().to_string()
}

/// Drive every goal in `scope` against a live ACP agent and return each
/// subagent's structured capture.
///
/// This is the engine entry point the MCP `expect` tool calls. It owns the
/// agent-pool choreography:
///
/// 1. Drain the agent's `notification_rx` broadcast into a fresh
///    [`NotificationSender`](claude_agent::NotificationSender) the pool's workers
///    subscribe to — the single source of streamed `session/update` content (see
///    the module docs on why the connection re-emission is NOT also forwarded).
/// 2. Stand up `Client.builder().connect_with(agent, ...)` to obtain a typed
///    [`ConnectionTo<Agent>`] and build the shared [`AgentPool`] over it, sized
///    by `pool_config`.
/// 3. Run [`InitializeRequest`] ONCE per connection, then submit one prompt per
///    goal and collect each structured reply.
///
/// `agent` and `notification_rx` are the two halves of an ACP agent handle,
/// supplied by the tool so this crate stays free of any agent-construction
/// dependency. `repo_root` is resolved by the caller from the MCP session
/// work-dir (never `current_dir()`); the agent's `fs/read_text_file` reads are
/// confined under it and `fs/write_text_file` under its `.expect/` ledger is
/// refused.
///
/// # Errors
///
/// Returns [`ExpectError::Agent`] when the ACP connection fails to stand up, the
/// pool drops a turn, a driven prompt fails, or a subagent's reply is not JSON.
pub async fn run_expect_over_agent(
    agent: DynConnectTo<Client>,
    notification_rx: broadcast::Receiver<SessionNotification>,
    scope: ExpectScope,
    repo_root: &Path,
    pool_config: PoolConfig,
) -> Result<Vec<DrivenObservation>, ExpectError> {
    // A fresh notifier whose broadcast the pool's workers subscribe to, fed by a
    // single forwarding task draining the agent's `notification_rx`. This is the
    // ONLY feed into the notifier (see the module docs on double-feeding).
    let (notifier, forward_task) = build_pool_notifier(notification_rx);

    // The repo root the agent's `fs` requests are resolved under. Owned so the
    // `'static` request handler can keep it for the connection's life.
    let repo_root: Arc<PathBuf> = Arc::new(repo_root.to_path_buf());

    let connect_result = Client
        .builder()
        .name("swissarmyhammer-expect")
        // An abandoned turn (the pool's per-turn liveness dropped its
        // `block_task` receiver) must fail that turn only: route the agent's
        // late response into the void instead of killing the dispatch loop and
        // the whole run with it.
        .with_handler(TolerantResponseRouter)
        .on_receive_request(
            {
                let repo_root = Arc::clone(&repo_root);
                move |req: AgentRequest,
                      responder: Responder<serde_json::Value>,
                      cx: ConnectionTo<agent_client_protocol::Agent>| {
                    let repo_root = Arc::clone(&repo_root);
                    async move {
                        answer_agent_request(req, responder, &cx, &repo_root);
                        Ok(())
                    }
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, {
            let notifier = Arc::clone(&notifier);
            move |cx: ConnectionTo<agent_client_protocol::Agent>| {
                run_pipeline_in_connection(cx, notifier, pool_config, scope)
            }
        })
        .await;

    forward_task.abort();

    match connect_result {
        Ok(observations) => observations,
        Err(e) => Err(ExpectError::Agent(format!(
            "expect agent connection failed: {e}"
        ))),
    }
}

/// The seam that delegates one expectation's withheld-criteria goal to a scoped
/// subagent and returns the subagent's structured self-report (a *claim*).
///
/// `observe_with_driver` depends on this trait, not on the concrete ACP wiring,
/// so the deterministic engine stays testable with a stub driver while the
/// production path uses [`AcpGoalDriver`]. The returned JSON is the driver's own
/// account of what it did — recorded in the [`Trajectory`](crate::types::Trajectory)
/// for triage, never trusted as the observation (`ideas/expect.md` §"The Check
/// Loop", hardening rule 2: the adapter's authoritative read is ground truth).
///
/// The method is expressed as `-> impl Future` rather than `async fn` so the
/// trait carries no implicit `Send` bound on the returned future — the ACP driver
/// future is `!Send` and runs on a current-thread runtime (the same reason the
/// tool layer drives the pipeline under `spawn_blocking`).
pub trait GoalDriver {
    /// Drive `goal` through one scoped subagent and return its structured claim.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Agent`] when the subagent cannot be driven or its
    /// reply is not recoverable JSON.
    fn drive_goal(
        &self,
        goal: &str,
    ) -> impl Future<Output = Result<serde_json::Value, ExpectError>>;
}

/// The two halves of a ready ACP agent handle the [`AcpGoalDriver`] consumes: the
/// [`DynConnectTo<Client>`] component and the broadcast receiver of the agent's
/// streamed `session/update` notifications.
///
/// This is the same shape the tool layer already mints for [`run_expect_over_agent`];
/// it is named here so [`AcpGoalDriver`] owns a single value rather than two loose
/// fields.
pub struct DriverHandle {
    /// The agent component the driver runs as the ACP server side.
    pub agent: DynConnectTo<Client>,
    /// The receiver of the agent's streamed notifications.
    pub notification_rx: broadcast::Receiver<SessionNotification>,
}

/// A [`GoalDriver`] backed by a live ACP agent: the goal is driven through **one
/// scoped session** via [`run_expect_over_agent`], and the subagent's structured
/// self-report is captured (recovered with the tolerant [`extract_json_value`]).
///
/// `run_expect_over_agent` consumes the agent handle when it stands up the
/// connection, so an [`AcpGoalDriver`] drives a single expectation — one scoped
/// session per expectation, which is exactly the abstraction this seam provides.
/// The handle is held in a [`Mutex`] and taken on first use; a second
/// [`drive_goal`](GoalDriver::drive_goal) returns an error rather than silently
/// reusing a spent connection. The tool layer mints a fresh handle (and a fresh
/// driver) per expectation.
pub struct AcpGoalDriver {
    /// The agent handle, taken on first drive (single scoped session per driver).
    handle: Mutex<Option<DriverHandle>>,
    /// The repo root the subagent's `fs` reads are confined under.
    repo_root: PathBuf,
    /// The pool sizing for the single scoped session.
    pool_config: PoolConfig,
}

impl AcpGoalDriver {
    /// Build a driver that drives one scoped session over `handle`, confining the
    /// subagent's reads under `repo_root` and sizing its pool by `pool_config`.
    pub fn new(
        handle: DriverHandle,
        repo_root: impl Into<PathBuf>,
        pool_config: PoolConfig,
    ) -> Self {
        Self {
            handle: Mutex::new(Some(handle)),
            repo_root: repo_root.into(),
            pool_config,
        }
    }
}

impl GoalDriver for AcpGoalDriver {
    fn drive_goal(
        &self,
        goal: &str,
    ) -> impl Future<Output = Result<serde_json::Value, ExpectError>> {
        // Take the handle synchronously so the std `Mutex` guard is never held
        // across the `await` below (the connection is single-use per driver).
        let taken = self
            .handle
            .lock()
            .expect("AcpGoalDriver handle mutex poisoned")
            .take();
        let scope = ExpectScope {
            goals: vec![goal.to_string()],
        };
        let repo_root = self.repo_root.clone();
        let pool_config = self.pool_config;
        async move {
            let handle = taken.ok_or_else(|| {
                ExpectError::Agent(
                    "AcpGoalDriver drives a single scoped session and was already used".to_string(),
                )
            })?;
            let mut observations = run_expect_over_agent(
                handle.agent,
                handle.notification_rx,
                scope,
                &repo_root,
                pool_config,
            )
            .await?;
            observations
                .pop()
                .map(|observation| observation.structured)
                .ok_or_else(|| {
                    ExpectError::Agent(
                        "the driving subagent produced no structured capture".to_string(),
                    )
                })
        }
    }
}

/// Observe `expectation` against its surface, delegating to a scoped subagent
/// **only** for the steps the adapter cannot resolve mechanically, and recording
/// the subagent's structured self-report as a claim in the trajectory.
///
/// This is the agent-fallback half of `ideas/expect.md` §"The Check Loop". The
/// three roles stay separate:
///
/// - The **adapter** always reads the authoritative state: the returned
///   [`Observation`]'s checkpoints come from [`observe`], never from the claim.
/// - The **driver** (the scoped subagent) is consulted only when a `When` step
///   does not [`resolve_mechanically`](SurfaceAdapter::resolves_mechanically). On
///   a deterministic surface — every cli step is a concrete argv — no step needs
///   the agent, so `driver` is never invoked and the mechanical path stands alone.
/// - When the agent *is* invoked, it is driven with the withheld-criteria goal
///   from [`build_driver_goal`], and its structured reply is appended to the
///   trajectory as a [`CLAIM_STEP_PREFIX`]-tagged claim — kept for triage, never
///   the verdict source.
///
/// # Errors
///
/// Returns [`ExpectError`] when the adapter cannot provision/drive/observe/tear
/// down the SUT, when the driver fails, or when the claim cannot be serialized.
pub async fn observe_with_driver<A, D>(
    expectation: &Expectation,
    adapter: &A,
    config: &ObserveConfig,
    driver: &D,
) -> Result<Observation, ExpectError>
where
    A: SurfaceAdapter,
    D: GoalDriver,
{
    // Decide whether any `When` step needs interpretation before driving the SUT,
    // so a deterministic run never stands up the agent at all.
    let needs_agent = expectation
        .when
        .iter()
        .any(|step| !adapter.resolves_mechanically(step));

    // The subagent supplies the action the adapter could not resolve; capture its
    // claim first, then let the adapter read the authoritative checkpoints.
    let claim = if needs_agent {
        let goal = build_driver_goal(expectation);
        Some(driver.drive_goal(&goal).await?)
    } else {
        None
    };

    let mut observation = observe(expectation, adapter, config)?;

    if let Some(claim) = claim {
        observation
            .trajectory
            .steps
            .push(format_claim_step(&claim)?);
    }
    Ok(observation)
}

/// Render the subagent's structured claim as a [`CLAIM_STEP_PREFIX`]-tagged
/// trajectory step.
///
/// # Errors
///
/// Returns [`ExpectError::Json`] when the claim cannot be serialized.
fn format_claim_step(claim: &serde_json::Value) -> Result<String, ExpectError> {
    Ok(format!(
        "{CLAIM_STEP_PREFIX}{}",
        serde_json::to_string(claim)?
    ))
}

/// Buffer size for the pool's notification broadcast channel.
const NOTIFY_BUFFER: usize = 256;

/// Build the pool's notifier and spawn the single task that feeds it from the
/// agent's `notification_rx` broadcast.
///
/// This is the engine's one and only notification path: the per-prompt
/// collectors subscribe to the returned
/// [`NotificationSender`](claude_agent::NotificationSender), and exactly one
/// [`forward_notifications`] task copies each incoming agent notification into
/// it. The caller aborts the returned [`JoinHandle`](tokio::task::JoinHandle)
/// once the pipeline is done. Keeping this the sole feed is what guarantees a
/// real handle's reply is collected once rather than twice (see the module docs).
fn build_pool_notifier(
    notification_rx: broadcast::Receiver<SessionNotification>,
) -> (
    Arc<claude_agent::NotificationSender>,
    tokio::task::JoinHandle<()>,
) {
    let (notifier, _seed_rx) = claude_agent::NotificationSender::new(NOTIFY_BUFFER);
    let notifier = Arc::new(notifier);
    let forward_task = tokio::spawn(forward_notifications(
        notification_rx,
        Arc::clone(&notifier),
    ));
    (notifier, forward_task)
}

/// Copy every notification from the agent's stream into the pool's notifier
/// until the source channel closes.
async fn forward_notifications(
    mut rx: broadcast::Receiver<SessionNotification>,
    notifier: Arc<claude_agent::NotificationSender>,
) {
    loop {
        match rx.recv().await {
            Ok(notif) => {
                let _ = notifier.send_update(notif).await;
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Answer a request the agent sends back to the `expect` client mid-prompt.
///
/// A real agent, during a prompt turn, issues nested agent→client requests and
/// blocks on their responses before the turn can finish. The client MUST answer
/// them or the prompt deadlocks and the whole run hangs.
///
/// Each variant is handled and a response is ALWAYS sent:
///
/// - `session/request_permission` → auto-approve (`Selected("allow")`). The run
///   is unattended; there is no human to prompt for tool consent.
/// - `fs/read_text_file` → read the file from disk **confined under `repo_root`**
///   (honoring the optional 1-based `line` and `limit`) and return its content.
/// - `fs/write_text_file` → **DENY** any write that resolves under the repo's
///   `.expect/` ledger (tamper-resistance: the driving agent must not edit the
///   specs/goldens/fixtures it is graded against); ack any other write WITHOUT
///   writing (the system under test is driven via its surface adapter, not ACP
///   fs, so an ack keeps the agent from hanging without mutating the repo).
/// - anything else → method-not-found error.
///
/// The work is dispatched via [`ConnectionTo::spawn`] so it runs OFF the
/// connection's single dispatch loop, keeping that loop free to route responses.
fn answer_agent_request(
    request: AgentRequest,
    responder: Responder<serde_json::Value>,
    cx: &ConnectionTo<agent_client_protocol::Agent>,
    repo_root: &Arc<PathBuf>,
) {
    let repo_root = Arc::clone(repo_root);
    let _ = cx.clone().spawn(async move {
        match request {
            AgentRequest::RequestPermissionRequest(_req) => {
                let outcome = RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    PermissionOptionId::new("allow"),
                ));
                responder
                    .cast()
                    .respond_with_result(Ok(RequestPermissionResponse::new(outcome)))
            }
            AgentRequest::ReadTextFileRequest(req) => {
                let result = read_text_file_under_repo(&repo_root, &req)
                    .map(ReadTextFileResponse::new)
                    .map_err(|e| agent_client_protocol::Error::invalid_params().data(e));
                responder.cast().respond_with_result(result)
            }
            AgentRequest::WriteTextFileRequest(req) => {
                // Refuse a ledger write; ack any other write without touching disk.
                let result = refuse_ledger_write(&repo_root, &req.path)
                    .map(|()| WriteTextFileResponse::new())
                    .map_err(|reason| {
                        tracing::warn!("expect client denied a ledger write: {reason}");
                        agent_client_protocol::Error::invalid_params().data(reason)
                    });
                responder.cast().respond_with_result(result)
            }
            other => {
                tracing::warn!(
                    "expect client received unsupported agent request: {}",
                    other.method()
                );
                responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found())
            }
        }
    });
}

/// Read a text file the agent requested, **confined under `repo_root`**, honoring
/// the optional 1-based `line` start and `limit` line count.
///
/// The agent names the path, so it is untrusted: an absolute path could point
/// anywhere and a relative path can carry `..` segments. The read is confined —
/// the canonicalized target must live inside the canonicalized `repo_root` or the
/// request is refused. The boundary is location, not shape: an absolute path that
/// genuinely resolves under `repo_root` is still honored.
///
/// Returns the (possibly sliced) content, or an error string when the file
/// cannot be read or resolves outside the repository (mapped to `invalid_params`
/// by the caller).
fn read_text_file_under_repo(
    repo_root: &Path,
    req: &agent_client_protocol::schema::ReadTextFileRequest,
) -> Result<String, String> {
    let path = confine_under_repo(repo_root, &req.path)?;

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    // No slice requested: return the whole file.
    if req.line.is_none() && req.limit.is_none() {
        return Ok(content);
    }

    let lines: Vec<&str> = content.lines().collect();
    let start = req.line.map(|l| (l.max(1) - 1) as usize).unwrap_or(0);
    let end = req
        .limit
        .map(|l| start + l as usize)
        .unwrap_or(lines.len())
        .min(lines.len());

    if start >= lines.len() {
        return Ok(String::new());
    }
    Ok(lines[start..end].join("\n"))
}

/// Resolve an agent-requested **read** path to a concrete on-disk path confined
/// under `repo_root`, rejecting any target that escapes the repository.
///
/// A relative path is joined onto `repo_root`; an absolute path is taken as-is.
/// Both the candidate and `repo_root` are canonicalized (resolving `..` and
/// symlinks) and the canonical candidate must `starts_with` the canonical repo
/// root — so a `..`-escape or out-of-repo absolute path is refused even when the
/// target exists.
///
/// Returns an error string (mapped to `invalid_params` by the caller) when the
/// repo root or the target cannot be canonicalized, or when the target lies
/// outside the repository.
fn confine_under_repo(repo_root: &Path, requested: &Path) -> Result<PathBuf, String> {
    let canonical_root = repo_root
        .canonicalize()
        .map_err(|e| format!("failed to resolve repo root {}: {e}", repo_root.display()))?;

    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        canonical_root.join(requested)
    };

    let canonical = candidate.canonicalize().map_err(|e| {
        format!(
            "failed to resolve requested path {}: {e}",
            candidate.display()
        )
    })?;

    if !canonical.starts_with(&canonical_root) {
        return Err(format!(
            "requested path {} is outside the repository {}",
            canonical.display(),
            canonical_root.display()
        ));
    }

    Ok(canonical)
}

/// Refuse a **write** that resolves under the repo's `.expect/` ledger.
///
/// The tamper-resistance guard: the driving agent may act on the system under
/// test but must never edit the ledger it is graded against (specs, goldens,
/// received fixtures all live under [`EXPECT_DIR`]). A write target's final
/// components may not exist yet, so — unlike [`confine_under_repo`], which can
/// canonicalize an existing read target whole — this resolves the path's longest
/// *existing* ancestor through the filesystem (folding away symlinks, the
/// `/var`→`/private/var` class of escape) and only normalizes the non-existent
/// tail lexically, then checks the result against the canonical ledger prefix.
///
/// Returns `Err` with a refusal message (mapped to `invalid_params` by the
/// caller) when the resolved target lies under `<repo_root>/.expect/`, and
/// `Ok(())` for every write outside the ledger.
fn refuse_ledger_write(repo_root: &Path, requested: &Path) -> Result<(), String> {
    let canonical_root = repo_root
        .canonicalize()
        .map_err(|e| format!("failed to resolve repo root {}: {e}", repo_root.display()))?;
    let ledger = canonical_root.join(EXPECT_DIR);

    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        canonical_root.join(requested)
    };
    let resolved = resolve_existing_prefix(&candidate);

    if resolved.starts_with(&ledger) {
        return Err(format!(
            "refusing to write {} under the {}/ ledger: the driving agent must not edit \
             specs, goldens, or received fixtures",
            resolved.display(),
            EXPECT_DIR
        ));
    }
    Ok(())
}

/// Resolve `path` to a concrete location by canonicalizing its longest existing
/// ancestor (resolving symlinks and `..` in the part that is on disk) and
/// re-appending the lexically-normalized remainder.
///
/// A write target's final components typically do not exist yet, so a whole-path
/// [`Path::canonicalize`] would fail. Canonicalizing the existing prefix instead
/// closes the symlinked-prefix escape (e.g. macOS's `/var` → `/private/var`,
/// where a purely lexical check would not match a `/private/var`-rooted ledger),
/// while [`normalize_lexically`] folds any `..` in the not-yet-existing tail.
fn resolve_existing_prefix(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Ok(canonical) = ancestor.canonicalize() {
            // `ancestor` is always a prefix of `path`, so this strip cannot fail.
            let tail = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return normalize_lexically(&canonical.join(tail));
        }
    }
    normalize_lexically(path)
}

/// Normalize a path lexically: resolve `.` and `..` components by string
/// manipulation, without consulting the filesystem (so it works on the
/// not-yet-existing tail of a write target).
fn normalize_lexically(path: &Path) -> PathBuf {
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

/// Build the pool inside the live connection and drive the scope to a result.
///
/// Split out so the `connect_with` closure body has a single typed future to
/// return. The pool is dropped at the end of this scope, winding its workers down
/// before the connection tears down.
async fn run_pipeline_in_connection(
    cx: ConnectionTo<agent_client_protocol::Agent>,
    notifier: Arc<claude_agent::NotificationSender>,
    pool_config: PoolConfig,
    scope: ExpectScope,
) -> agent_client_protocol::Result<Result<Vec<DrivenObservation>, ExpectError>> {
    // ACP `initialize` is a ONCE-per-connection handshake. Do it here, before the
    // pool's workers issue any prompts, rather than per prompt: the pool shares
    // this single connection across N workers, so initializing per prompt would
    // race N concurrent handshakes at the one agent process and wedge it.
    //
    // Advertise the filesystem capabilities the request handler backs: both
    // `fs/read_text_file` (served from disk under the repo root) and
    // `fs/write_text_file` (handled — ledger writes refused, others acked) so the
    // agent's capability view matches `answer_agent_request`.
    cx.send_request(
        InitializeRequest::new(1.into()).client_capabilities(
            ClientCapabilities::new().fs(FileSystemCapabilities::new()
                .read_text_file(true)
                .write_text_file(true)),
        ),
    )
    .block_task()
    .await?;

    let pool = AgentPool::new(cx, notifier, pool_config);
    Ok(drive_scope(&pool, scope).await)
}

/// Submit one prompt per goal, then collect each subagent's structured reply in
/// submission order.
///
/// Submission is non-blocking, so all goals are queued first (pipelining across
/// the pool's workers) and then awaited. Each reply is parsed through the shared
/// tolerant [`extract_json_value`](swissarmyhammer_validators::review::extract_json_value)
/// extractor.
async fn drive_scope(
    pool: &AgentPool,
    scope: ExpectScope,
) -> Result<Vec<DrivenObservation>, ExpectError> {
    let pending: Vec<(String, _)> = scope
        .goals
        .into_iter()
        .map(|goal| {
            let rx = pool.submit(goal.clone());
            (goal, rx)
        })
        .collect();

    let mut observations = Vec::with_capacity(pending.len());
    for (goal, rx) in pending {
        let collected = rx
            .await
            .map_err(|e| {
                ExpectError::Agent(format!("the agent pool dropped the turn for `{goal}`: {e}"))
            })?
            .map_err(|e| ExpectError::Agent(format!("driving `{goal}` failed: {e}")))?;

        let json = extract_json_value(&collected.content, '{', '}');
        let structured: serde_json::Value = serde_json::from_str(json).map_err(|e| {
            ExpectError::Agent(format!(
                "the subagent's reply for `{goal}` was not structured JSON: {e}"
            ))
        })?;
        observations.push(DrivenObservation { goal, structured });
    }
    Ok(observations)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    use acp_conformance::test_utils::{numbered_session_response, MockAgent, MockAgentAdapter};
    use agent_client_protocol::schema::{
        ContentBlock, ContentChunk, NewSessionRequest, NewSessionResponse, PromptRequest,
        PromptResponse, SessionId, SessionUpdate, StopReason, TextContent,
    };
    use futures::future::BoxFuture;
    use tempfile::TempDir;

    use crate::spec::Setup;
    use crate::types::SurfaceState;

    /// How long a wedged pipeline may run before the test fails instead of
    /// hanging CI.
    const PIPELINE_TIMEOUT: Duration = Duration::from_secs(30);

    /// Capacity of the scripted backend's broadcast channel — the channel the
    /// driver's `notification_rx` subscribes to. It comfortably exceeds any
    /// test's notification volume so a slow subscriber never lags chunks away.
    const BACKEND_BROADCAST_CAPACITY: usize = 64;

    /// Capacity for the single-stream invariant test, whose channels must hold
    /// EVERY chunk sent before any collector subscribes and drains.
    const PRELOADED_STREAM_CAPACITY: usize = 256;

    /// A representative structured reply: the JSON object shape a driven subagent
    /// emits and [`drive_scope`] captures via [`extract_json_value`].
    const STRUCTURED_REPLY: &str = r#"{"path": "src/checkout/coupon", "verdict": "pass"}"#;

    // ---- a temp repo fixture --------------------------------------------

    /// The repo-relative path of the file [`temp_repo`] plants, and a substring
    /// of its content the read assertions check.
    const FIXTURE_FILE: &str = "src/lib.rs";
    const FIXTURE_NEEDLE: &str = "pub fn compute";

    /// A throwaway repo with one readable source file under `src/`.
    fn temp_repo() -> TempDir {
        let dir = TempDir::new().expect("temp repo");
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join(FIXTURE_FILE),
            "pub fn compute() -> u32 { 42 }\n",
        )
        .unwrap();
        dir
    }

    // ---- single-path notification invariant (the double-feed guard) -----

    /// Split `text` into `parts` roughly equal chunks, one `AgentMessageChunk`
    /// notification per chunk. Streaming the reply across several chunks (as a
    /// real backend does) is what makes double-delivery corrupt: a duplicated,
    /// interleaved chunk stream cannot be reassembled back into the original JSON.
    fn chunked_notifications(
        session: &SessionId,
        text: &str,
        parts: usize,
    ) -> Vec<SessionNotification> {
        let bytes = text.as_bytes();
        let step = bytes.len().div_ceil(parts).max(1);
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < bytes.len() {
            let mut end = (start + step).min(bytes.len());
            while !text.is_char_boundary(end) {
                end += 1;
            }
            let piece = &text[start..end];
            let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(piece.to_string()),
            )));
            chunks.push(SessionNotification::new(session.clone(), update));
            start = end;
        }
        chunks
    }

    /// Collect a multi-chunk streamed reply through the pool's notifier, exactly
    /// as a pool worker does: subscribe to the notifier's broadcast, reassemble
    /// the streamed text for `session`, and return the collected string.
    async fn collect_through_notifier(
        notifier: &Arc<claude_agent::NotificationSender>,
        session: SessionId,
    ) -> String {
        let (collector, collected_text, notification_count, _matched) =
            claude_agent::spawn_notification_collector(notifier.sender().subscribe(), session);
        let prompt_response = PromptResponse::new(StopReason::EndTurn);
        claude_agent::collect_response_content(
            collector,
            collected_text,
            notification_count,
            &prompt_response,
        )
        .await
    }

    /// The driver feeds the pool's collectors from EXACTLY ONE source: the
    /// agent's `notification_rx`, drained by the single [`forward_notifications`]
    /// task [`build_pool_notifier`] spawns. This pins both halves of the
    /// invariant deterministically:
    ///
    /// 1. The single-feed seam reassembles the streamed reply EXACTLY once
    ///    (byte-for-byte equal to the original).
    /// 2. A second feed of the same stream — the dual-path bug — doubles every
    ///    chunk, so the collected text is twice as long and no longer the
    ///    original. The doubling holds for every interleaving, so the
    ///    discriminating assertion is not flaky.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn notification_rx_is_the_pools_single_collected_stream() {
        let session = SessionId::new("sess-single".to_string());
        let reply = STRUCTURED_REPLY.to_string();
        let stream = chunked_notifications(&session, &reply, 6);

        // --- (1) the driver's actual single-feed path collects the reply once ---
        let (notify_tx, notification_rx) =
            broadcast::channel::<SessionNotification>(PRELOADED_STREAM_CAPACITY);
        let (single_notifier, single_forward) = build_pool_notifier(notification_rx);
        for notif in &stream {
            let _ = notify_tx.send(notif.clone());
        }
        let collected_single = collect_through_notifier(&single_notifier, session.clone()).await;
        single_forward.abort();

        assert_eq!(
            collected_single, reply,
            "the driver's single feed must reassemble the agent reply exactly once"
        );

        // --- (2) the dual-feed shape doubles the same stream -------------------
        let (dual_tx, dual_rx_a) =
            broadcast::channel::<SessionNotification>(PRELOADED_STREAM_CAPACITY);
        let dual_rx_b = dual_tx.subscribe();
        let (dual_notifier, _seed) = claude_agent::NotificationSender::new(NOTIFY_BUFFER);
        let dual_notifier = Arc::new(dual_notifier);
        let fwd_a = tokio::spawn(forward_notifications(dual_rx_a, Arc::clone(&dual_notifier)));
        let fwd_b = tokio::spawn(forward_notifications(dual_rx_b, Arc::clone(&dual_notifier)));
        for notif in &stream {
            let _ = dual_tx.send(notif.clone());
        }
        let collected_dual = collect_through_notifier(&dual_notifier, session).await;
        fwd_a.abort();
        fwd_b.abort();

        assert_ne!(
            collected_dual, reply,
            "a dual feed must NOT reassemble the original reply — this is the bug the \
             single-path driver fixes"
        );
        assert_eq!(
            collected_dual.len(),
            reply.len() * 2,
            "a dual feed doubles every chunk, doubling the collected length and corrupting \
             the JSON; the single-feed driver avoids this"
        );
    }

    // ---- read confinement (read_text_file_under_repo) -------------------

    /// Build a `fs/read_text_file` request for `path` (relative or absolute).
    fn read_request(
        path: impl Into<PathBuf>,
    ) -> agent_client_protocol::schema::ReadTextFileRequest {
        agent_client_protocol::schema::ReadTextFileRequest::new(
            SessionId::new("sess-read".to_string()),
            path.into(),
        )
    }

    #[test]
    fn read_text_file_under_repo_serves_an_in_repo_relative_path() {
        let repo = temp_repo();
        let content = read_text_file_under_repo(repo.path(), &read_request(FIXTURE_FILE))
            .expect("an in-repo relative read must succeed");
        assert!(
            content.contains(FIXTURE_NEEDLE),
            "the in-repo read must return the real file content, got: {content}"
        );
    }

    #[test]
    fn read_text_file_under_repo_rejects_a_dotdot_escape() {
        let repo = temp_repo();
        // Plant a readable target in the repo's PARENT so the read must still be
        // refused on location, not on absence.
        let parent = repo
            .path()
            .parent()
            .expect("temp repo has a parent dir")
            .to_path_buf();
        std::fs::write(parent.join("secret.txt"), "top secret").unwrap();

        let err = read_text_file_under_repo(repo.path(), &read_request("../secret.txt"))
            .expect_err("a ..-escape must be rejected");
        assert!(
            err.contains("outside the repository"),
            "the rejection must name the confinement boundary, got: {err}"
        );
    }

    #[test]
    fn read_text_file_under_repo_rejects_an_absolute_outside_path() {
        let repo = temp_repo();
        let err = read_text_file_under_repo(repo.path(), &read_request("/etc/passwd"))
            .expect_err("an absolute outside path must be rejected");
        assert!(
            err.contains("outside the repository"),
            "the rejection must name the confinement boundary, got: {err}"
        );
    }

    #[test]
    fn read_text_file_under_repo_serves_an_absolute_in_repo_path() {
        let repo = temp_repo();
        let abs = repo.path().join(FIXTURE_FILE);
        let content = read_text_file_under_repo(repo.path(), &read_request(abs))
            .expect("an absolute in-repo read must succeed");
        assert!(
            content.contains(FIXTURE_NEEDLE),
            "an absolute in-repo read must return the real content, got: {content}"
        );
    }

    // ---- write tamper-resistance (refuse_ledger_write) ------------------

    #[test]
    fn refuse_ledger_write_denies_a_relative_write_under_the_ledger() {
        let repo = temp_repo();
        let err = refuse_ledger_write(repo.path(), Path::new(".expect/goldens/coupon.golden.json"))
            .expect_err("a relative write under .expect/ must be refused");
        assert!(
            err.contains(EXPECT_DIR),
            "the refusal must name the ledger boundary, got: {err}"
        );
    }

    #[test]
    fn refuse_ledger_write_denies_an_absolute_write_under_the_ledger() {
        let repo = temp_repo();
        let abs = repo.path().join(".expect").join("received").join("x.json");
        let err = refuse_ledger_write(repo.path(), &abs)
            .expect_err("an absolute write under .expect/ must be refused");
        assert!(err.contains(EXPECT_DIR), "got: {err}");
    }

    #[test]
    fn refuse_ledger_write_denies_a_dotdot_climb_back_into_the_ledger() {
        let repo = temp_repo();
        // A path dressed up to climb out of `src/` and back into the ledger must
        // still be refused after lexical normalization.
        let err = refuse_ledger_write(repo.path(), Path::new("src/../.expect/config.toml"))
            .expect_err("a ..-dressed ledger write must be refused");
        assert!(err.contains(EXPECT_DIR), "got: {err}");
    }

    #[test]
    fn refuse_ledger_write_allows_a_write_outside_the_ledger() {
        let repo = temp_repo();
        refuse_ledger_write(repo.path(), Path::new("src/generated_output.txt"))
            .expect("a write outside the .expect/ ledger must be allowed");
    }

    // ---- end-to-end over a stub ACP agent -------------------------------

    /// A minimal stub ACP agent: every prompt streams `reply` onto the backend
    /// broadcast (the channel the driver's `notification_rx` subscribes to) under
    /// the prompt's own session id, then ends the turn — the real-handle shape
    /// `run_expect_over_agent` collects from.
    struct EchoAgent {
        next_session: AtomicUsize,
        notify_tx: broadcast::Sender<SessionNotification>,
        reply: String,
    }

    impl MockAgent for EchoAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            numbered_session_response(&self.next_session, "sess")
        }

        fn prompt<'a>(
            &'a self,
            request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            Box::pin(async move {
                let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(
                    ContentBlock::Text(TextContent::new(self.reply.clone())),
                ));
                let _ = self
                    .notify_tx
                    .send(SessionNotification::new(request.session_id.clone(), update));
                Ok(PromptResponse::new(StopReason::EndTurn))
            })
        }
    }

    /// `run_expect_over_agent` connects over ACP, initializes once, drives the
    /// scope's goal over the pool, and captures the subagent's structured reply —
    /// the full seam end to end against a stub agent.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_expect_over_agent_drives_a_goal_over_a_stub_agent() {
        let repo = temp_repo();
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = Arc::new(EchoAgent {
            next_session: AtomicUsize::new(0),
            notify_tx,
            reply: STRUCTURED_REPLY.to_string(),
        });
        let dyn_agent = DynConnectTo::new(MockAgentAdapter(Arc::clone(&agent)));

        const GOAL: &str = "observe src/checkout/coupon";
        let scope = ExpectScope {
            goals: vec![GOAL.to_string()],
        };

        let observations = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            run_expect_over_agent(
                dyn_agent,
                notification_rx,
                scope,
                repo.path(),
                PoolConfig::remote(1),
            ),
        )
        .await
        .expect("the run must not hang")
        .expect("the pipeline must produce observations");

        assert_eq!(observations.len(), 1, "exactly one goal was driven");
        assert_eq!(observations[0].goal, GOAL, "the goal identity is preserved");
        assert_eq!(
            observations[0].structured["verdict"], "pass",
            "the subagent's structured reply is captured: {:?}",
            observations[0].structured
        );
        // Exactly one `session/new` was minted for the one driven expectation —
        // the scoped-session-per-expectation contract — and the turn ran to its
        // `stopReason` (the pipeline returned, so the session was drained and the
        // pool tore it down).
        assert_eq!(
            agent.next_session.load(Ordering::SeqCst),
            1,
            "one session/new per expectation"
        );
    }

    // ---- criteria-withholding prompt assembly (build_driver_goal) -------

    /// A spec whose intent narrative, `Given`, `When`, `Then`, and `Notes` each
    /// carry a unique marker, so the withholding split can be asserted exactly:
    /// the driver goal must contain the first three markers and none of the last
    /// two.
    const WITHHOLD_SPEC: &str = r#"---
description: a one-line description
surface: cli
---

# Title line

NARRATIVE_INTENT the behavior the driver must accomplish.

## Given
- GIVEN_PRECONDITION an arranged state

## When
- WHEN_ACTION the action to perform

## Then
- [ ] THEN_CRITERION the rubric the grader checks

## Notes
NOTES_RIGHT_REASON the right-reason text routed to the grader.
"#;

    /// Parse [`WITHHOLD_SPEC`] into an [`Expectation`] addressed under `/repo`.
    fn withhold_expectation() -> Expectation {
        Expectation::parse(
            WITHHOLD_SPEC,
            Path::new("/repo/feature.expect.md"),
            Path::new("/repo"),
        )
        .expect("parse withhold spec")
    }

    /// The driver goal carries the intent narrative and the `Given`/`When` steps,
    /// but NOT the `Then` checklist or the `Notes` right-reason text — the
    /// SWE-bench held-out discipline that defends against reward-hacking.
    #[test]
    fn driver_goal_carries_intent_given_when_and_withholds_then_and_notes() {
        let goal = build_driver_goal(&withhold_expectation());

        for present in ["NARRATIVE_INTENT", "GIVEN_PRECONDITION", "WHEN_ACTION"] {
            assert!(
                goal.contains(present),
                "the driver goal must carry `{present}` (intent + Given + When): {goal}"
            );
        }
        for withheld in ["THEN_CRITERION", "NOTES_RIGHT_REASON"] {
            assert!(
                !goal.contains(withheld),
                "the driver goal must withhold `{withheld}` (Then criterion / Notes): {goal}"
            );
        }
    }

    // ---- observe-with-driver integration --------------------------------

    /// A stub surface adapter whose checkpoints are a fixed authoritative read,
    /// with a controllable [`SurfaceAdapter::resolves_mechanically`] gate so a
    /// test can force the agent-fallback path (`resolves = false`) or the
    /// deterministic path (`resolves = true`).
    struct StubAdapter {
        resolves: bool,
        state: SurfaceState,
    }

    impl SurfaceAdapter for StubAdapter {
        type ProvisionedSut = ();

        fn provision(&self, _setup: Option<&Setup>, _repo_root: &Path) -> Result<(), ExpectError> {
            Ok(())
        }

        fn drive(&self, _sut: &mut (), _when_step: &str) -> Result<(), ExpectError> {
            Ok(())
        }

        fn observe(&self, _sut: &()) -> Result<SurfaceState, ExpectError> {
            Ok(self.state.clone())
        }

        fn teardown(&self, _sut: ()) -> Result<(), ExpectError> {
            Ok(())
        }

        fn resolves_mechanically(&self, _when_step: &str) -> bool {
            self.resolves
        }
    }

    /// A [`GoalDriver`] that records whether it was invoked and never drives a
    /// real agent — used to prove the deterministic path skips the driver.
    struct RecordingDriver {
        invoked: AtomicBool,
    }

    impl GoalDriver for RecordingDriver {
        fn drive_goal(
            &self,
            _goal: &str,
        ) -> impl Future<Output = Result<serde_json::Value, ExpectError>> {
            self.invoked.store(true, Ordering::SeqCst);
            async move { Ok(serde_json::json!({})) }
        }
    }

    /// A reply where the structured JSON is prefaced by prose and wrapped in a
    /// ```json fence — the malformed-then-fenced shape `extract_json_value`
    /// recovers.
    const FENCED_CLAIM_REPLY: &str =
        "Sure! Here is what I did:\n```json\n{\"summary\": \"drove the SUT\", \"verdict\": \"done\"}\n```\nLet me know if you need anything else.";

    /// A marker planted in the stub adapter's checkpoint state, asserted to be the
    /// source of the observation's checkpoints (never the agent's claim).
    const GROUND_TRUTH_MARKER: &str = "GROUND_TRUTH";

    /// The agent-fallback path: a subagent returns malformed-then-fenced JSON,
    /// `extract_json_value` recovers it, the recovered claim lands in the
    /// trajectory, and the checkpoints come from the adapter's authoritative read
    /// — not from the claim.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn agent_fallback_records_the_recovered_claim_while_checkpoints_come_from_the_adapter() {
        let repo = temp_repo();
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = Arc::new(EchoAgent {
            next_session: AtomicUsize::new(0),
            notify_tx,
            reply: FENCED_CLAIM_REPLY.to_string(),
        });
        let driver = AcpGoalDriver::new(
            DriverHandle {
                agent: DynConnectTo::new(MockAgentAdapter(agent)),
                notification_rx,
            },
            repo.path(),
            PoolConfig::remote(1),
        );

        // `resolves = false` forces the When step through the scoped subagent; the
        // adapter still reads the authoritative ground-truth state.
        let adapter = StubAdapter {
            resolves: false,
            state: SurfaceState::Json {
                body: serde_json::json!({ "observed": GROUND_TRUTH_MARKER }),
            },
        };
        let expectation = withhold_expectation();
        let config = ObserveConfig::new(repo.path());

        let observation = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            observe_with_driver(&expectation, &adapter, &config, &driver),
        )
        .await
        .expect("the run must not hang")
        .expect("observe_with_driver must produce an observation");

        // Checkpoints are the adapter's authoritative read — one per When step
        // plus the final — and carry the ground-truth marker, not the claim.
        assert!(
            !observation.checkpoints.is_empty(),
            "the adapter produced checkpoints"
        );
        for checkpoint in &observation.checkpoints {
            match &checkpoint.state {
                SurfaceState::Json { body } => assert_eq!(
                    body["observed"], GROUND_TRUTH_MARKER,
                    "checkpoints come from the adapter, not the agent's claim"
                ),
                other => panic!("expected the stub adapter's json state, got {other:?}"),
            }
        }

        // The recovered claim is recorded in the trajectory as a claim step,
        // never as a checkpoint.
        let claim_step = observation
            .trajectory
            .steps
            .iter()
            .find(|step| step.starts_with(CLAIM_STEP_PREFIX))
            .expect("the subagent's recovered claim is recorded in the trajectory");
        let claim_json = claim_step
            .strip_prefix(CLAIM_STEP_PREFIX)
            .expect("the claim step is prefixed");
        let claim: serde_json::Value =
            serde_json::from_str(claim_json).expect("the recorded claim is valid JSON");
        assert_eq!(
            claim["verdict"], "done",
            "the malformed-then-fenced reply was recovered into the claim: {claim:?}"
        );
        assert_eq!(
            claim["summary"], "drove the SUT",
            "the full claim is captured"
        );
        assert!(
            !claim_json.contains(GROUND_TRUTH_MARKER),
            "the claim is the agent's self-report, distinct from the adapter's state"
        );
    }

    /// The cli adapter resolves every step mechanically by default, so a cli
    /// expectation is deterministic and never reaches the agent fallback.
    #[test]
    fn cli_adapter_resolves_every_step_mechanically() {
        let adapter = crate::surface::cli::CliAdapter::default();
        assert!(
            adapter.resolves_mechanically("any concrete argv"),
            "cli is a deterministic surface: every step is a concrete argv"
        );
    }

    /// On a deterministic cli expectation, `observe_with_driver` drives the SUT
    /// mechanically and NEVER invokes the agent — the mechanical path stays the
    /// default.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn deterministic_cli_expectation_does_not_invoke_the_agent() {
        use std::os::unix::fs::PermissionsExt;

        let repo = TempDir::new().expect("temp repo");
        let script = repo.path().join("echo.sh");
        std::fs::write(&script, "#!/bin/sh\necho \"got $1\"\n").unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let spec = "---\ndescription: a deterministic cli run\nsurface: cli\nsetup: \"./echo.sh\"\n---\n\nRun the echo command and observe its output.\n\n## When\n- hello\n";
        let expectation =
            Expectation::parse(spec, &repo.path().join("feature.expect.md"), repo.path())
                .expect("parse cli spec");

        let adapter = crate::surface::cli::CliAdapter::default();
        let driver = RecordingDriver {
            invoked: AtomicBool::new(false),
        };
        let config = ObserveConfig::new(repo.path());

        let observation = observe_with_driver(&expectation, &adapter, &config, &driver)
            .await
            .expect("the deterministic cli run must succeed");

        assert!(
            !driver.invoked.load(Ordering::SeqCst),
            "a deterministic cli expectation must not invoke the agent"
        );
        // And the mechanical path produced authoritative checkpoints.
        let final_state = &observation
            .checkpoints
            .last()
            .expect("a final checkpoint")
            .state;
        match final_state {
            SurfaceState::Cli(cli) => {
                assert_eq!(cli.stdout, "got hello\n", "the cli SUT ran mechanically")
            }
            other => panic!("expected cli state, got {other:?}"),
        }
    }
}
