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
    SelectedPermissionOutcome, SessionNotification, StopReason, WriteTextFileResponse,
};
use agent_client_protocol::{Client, ConnectionTo, DynConnectTo, Responder};
use agent_client_protocol_extras::TolerantResponseRouter;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use swissarmyhammer_validators::review::extract_json_value;
use swissarmyhammer_validators::{AgentPool, PoolConfig};

use crate::config::EXPECT_DIR;
use crate::error::ExpectError;
use crate::evaluate::evaluate_spec;
use crate::observe::{observe, ObserveConfig};
use crate::replay::{ReplayCache, ReplayKey, ReplaySource, ResolvedAction};
use crate::spec::{parse_criterion, Expectation, Section};
use crate::surface::SurfaceAdapter;
use crate::types::{ExpectationVerdict, Observation, Surface};

/// The ACP protocol version advertised in the `initialize` handshake.
///
/// This is the single, named source for the version the client negotiates with
/// the agent in [`run_pipeline_in_connection`]'s once-per-connection
/// `initialize` handshake, rather than a bare `1` embedded at the call site.
const ACP_PROTOCOL_VERSION: u16 = 1;

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
    /// Whether the subagent declared the goal reached this turn — ACP's
    /// `stopReason: end_turn`, the **soft stop** (`ideas/expect.md`
    /// §"Stop conditions"). It is the agent's self-declaration that it is done,
    /// re-validated and never trusted as the verdict (hardening rule 2): a turn
    /// cut short by a hard cap (`max_turn_requests` / `max_tokens`) reports
    /// `false`, so the driver loop can tell a converged turn from a truncated
    /// one.
    pub goal_reached: bool,
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

/// The harness-imposed budget on agentic turns for one driven expectation — the
/// **max-prompt-turns hard cap** of `ideas/expect.md` §"Stop conditions".
///
/// Anchored on the surveyed agentic-loop defaults (Claude computer-use 10,
/// LangChain 15, LangGraph 25, Skyvern 10/25/50): a budget an honest goal
/// reaches well within, but a runaway agent should not exceed. `expect` owns
/// this cap because the model APIs document none, and imposes it on the driver
/// by stating it in the goal prompt ([`build_driver_goal`]) — the channel the
/// pool transport carries. The deterministic guarantee `expect` enforces is the
/// terminal one ([`drive_for_goal`]): a scoped session that ends WITHOUT the
/// agent declaring the goal reached (`stopReason` other than `end_turn` — the
/// agent hit its turn/token budget or stopped early) is a clear error, never
/// mistaken for success, so a non-converging turn ends the drive rather than
/// hanging it.
const MAX_PROMPT_TURNS: usize = 15;

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
    // The imposed turn budget is the only way the pool transport carries the
    // max-prompt-turns cap to the agent (see [`MAX_PROMPT_TURNS`]); `expect`
    // still enforces the terminal check deterministically in [`drive_for_goal`].
    format!(
        "{DRIVER_GOAL_PREAMBLE}\n\n{body}\n\nYou have at most {MAX_PROMPT_TURNS} prompt turns to \
         reach the goal; finish within them.\n\n{DRIVER_STRUCTURED_OUTPUT_INSTRUCTION}"
    )
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
    // No external cancel handle: a token nobody cancels never fires, so the pool's
    // turns are bounded only by their idle/ceiling deadlines. The cancellable
    // path is reached through [`AcpGoalDriver`] (see [`drive_and_revalidate`]).
    run_expect_over_agent_with_cancel(
        agent,
        notification_rx,
        scope,
        repo_root,
        pool_config,
        CancellationToken::new(),
    )
    .await
}

/// Like [`run_expect_over_agent`], but the pool's in-flight session can be
/// actively cancelled out of band by firing `cancel`.
///
/// This is the seam [`AcpGoalDriver`] drives so [`drive_and_revalidate`] can send
/// ACP `session/cancel` when the spec wall-clock timeout elapses, reusing the
/// pool's idle/ceiling cancel path rather than orphaning the agent behind a
/// dropped future. `cancel` is threaded straight to [`AgentPool::new_cancellable`].
async fn run_expect_over_agent_with_cancel(
    agent: DynConnectTo<Client>,
    notification_rx: broadcast::Receiver<SessionNotification>,
    scope: ExpectScope,
    repo_root: &Path,
    pool_config: PoolConfig,
    cancel: CancellationToken,
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
                run_pipeline_in_connection(cx, notifier, pool_config, scope, cancel)
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
/// One prompt turn's outcome from a [`GoalDriver`]: the subagent's structured
/// claim and whether it declared the goal reached.
///
/// `expect` owns both stops of the driver loop (`ideas/expect.md`
/// §"Stop conditions"): the **soft stop** is the agent declaring it reached the
/// goal, which ACP surfaces as `stopReason: end_turn` and this struct carries as
/// [`goal_reached`](DriverTurn::goal_reached). The claim is the agent's own
/// account — recorded for triage, re-validated, never trusted as the verdict.
#[derive(Debug, Clone)]
pub struct DriverTurn {
    /// The tolerant-extracted structured JSON the subagent produced (a *claim*).
    pub claim: serde_json::Value,
    /// Whether the subagent declared the goal reached this turn (the soft stop,
    /// ACP `stopReason: end_turn`). A turn cut short by a hard cap reports
    /// `false`, so the bounded driver loop can re-prompt rather than mistake a
    /// truncated turn for a converged one.
    pub goal_reached: bool,
}

pub trait GoalDriver {
    /// Drive `goal` through one scoped subagent and return its [`DriverTurn`] —
    /// the structured claim plus whether the agent declared the goal reached.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Agent`] when the subagent cannot be driven or its
    /// reply is not recoverable JSON, or [`ExpectError::Pool`] when the pool's
    /// liveness supervisor abandoned the turn.
    fn drive_goal(&self, goal: &str) -> impl Future<Output = Result<DriverTurn, ExpectError>>;

    /// Actively cancel the driver's in-flight scoped session (ACP
    /// `session/cancel`).
    ///
    /// [`drive_and_revalidate`] calls this on the spec wall-clock timeout branch
    /// so the driving agent stops working rather than being orphaned by the
    /// dropped drive future — mirroring the pool's idle/ceiling cancel path. The
    /// default is a no-op for drivers with no live session (the stub drivers);
    /// [`AcpGoalDriver`] overrides it to fire its pool cancel handle.
    fn cancel(&self) {}
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
    /// The pool cancel handle: firing it actively cancels the in-flight scoped
    /// session (ACP `session/cancel`) via the pool's abandonment path. Fired by
    /// [`GoalDriver::cancel`] on the spec-timeout teardown.
    cancel: CancellationToken,
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
            cancel: CancellationToken::new(),
        }
    }
}

impl GoalDriver for AcpGoalDriver {
    fn drive_goal(&self, goal: &str) -> impl Future<Output = Result<DriverTurn, ExpectError>> {
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
        let cancel = self.cancel.clone();
        async move {
            let handle = taken.ok_or_else(|| {
                ExpectError::Agent(
                    "AcpGoalDriver drives a single scoped session and was already used".to_string(),
                )
            })?;
            let mut observations = run_expect_over_agent_with_cancel(
                handle.agent,
                handle.notification_rx,
                scope,
                &repo_root,
                pool_config,
                cancel,
            )
            .await?;
            observations
                .pop()
                .map(|observation| DriverTurn {
                    claim: observation.structured,
                    goal_reached: observation.goal_reached,
                })
                .ok_or_else(|| {
                    ExpectError::Agent(
                        "the driving subagent produced no structured capture".to_string(),
                    )
                })
        }
    }

    fn cancel(&self) {
        // Fire the pool cancel handle wired into the in-flight session. The
        // pool's turn supervisor races this and sends ACP `session/cancel`
        // (the same teardown its idle/ceiling deadlines use).
        self.cancel.cancel();
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
    // claim first, then let the adapter read the authoritative checkpoints. The
    // resolved action is served from the replay cache by default — the agent is
    // hit only on a cache miss or fingerprint drift (`ideas/expect.md`
    // §"Determinism comes from not calling the model"). The drive itself is
    // bounded by the soft stop (the agent declaring the goal reached) and the
    // [`MAX_PROMPT_TURNS`] hard cap — see [`drive_for_goal`].
    let resolved = if needs_agent {
        Some(resolve_action(expectation, driver, &config.repo_root).await?)
    } else {
        None
    };

    let mut observation = observe(expectation, adapter, config)?;

    if let Some(resolved) = resolved {
        // A fingerprint-drift re-resolve is surfaced in the trajectory, never
        // silently applied — "a wrong cached click is worse than a slow click."
        if resolved.source.is_drift() {
            observation
                .trajectory
                .steps
                .push(format!("{DRIFT_STEP_PREFIX}{}", expectation.path));
        }
        observation
            .trajectory
            .steps
            .push(format_claim_step(&resolved.action)?);
    }
    Ok(observation)
}

/// The trajectory prefix recording a fingerprint-drift re-resolve: a cached
/// action could not be safely replayed, so the agent was consulted again and the
/// drift surfaced (never silently applied).
const DRIFT_STEP_PREFIX: &str = "drift: ";

/// Resolve `expectation`'s driven action through the [`ReplayCache`], invoking
/// the agent (`driver`) only on a cache miss or fingerprint drift — the cached
/// path is the default (`ideas/expect.md` §"Determinism comes from not calling
/// the model"). An unchanged target + state replays the stored action with no
/// model call.
///
/// The cache is a determinism optimization layered over the authoritative
/// observation, so its write is best-effort: a failed persist only costs a
/// re-resolve on the next run and never fails the check. A drift re-resolve is
/// surfaced (logged here, recorded in the trajectory by the caller), never
/// silently applied.
///
/// # Errors
///
/// Propagates [`drive_for_goal`]'s error on a miss or drift, and
/// [`ExpectError`] when the existing cache cannot be read.
async fn resolve_action<D: GoalDriver>(
    expectation: &Expectation,
    driver: &D,
    repo_root: &Path,
) -> Result<ResolvedAction, ExpectError> {
    let mut cache = ReplayCache::load(repo_root, &expectation.path)?;
    let (key, state) = replay_inputs(expectation);
    let resolved = cache
        .resolve_or_replay(&key, &state, || drive_for_goal(expectation, driver))
        .await?;

    if !matches!(resolved.source, ReplaySource::Cached) {
        if let Err(err) = cache.save(repo_root, &expectation.path) {
            tracing::warn!(
                identity = %expectation.path,
                error = %err,
                "failed to persist the replay cache; the next run will re-resolve"
            );
        }
    }
    if resolved.source.is_drift() {
        tracing::warn!(
            identity = %expectation.path,
            "a cached action drifted past the replay threshold; re-resolved via the agent (DRIFT)"
        );
    }
    Ok(resolved)
}

/// Derive the [`ReplayKey`] and state snapshot for `expectation`'s driven
/// action: the spec identity is the normalized target, the surface is the
/// method, and the `Given`/`When` steps the agent acts on are the state snapshot
/// whose drift re-triggers a re-resolve.
fn replay_inputs(expectation: &Expectation) -> (ReplayKey, String) {
    let key = ReplayKey::new(
        &expectation.path,
        surface_method(expectation.frontmatter.surface),
    );
    let state = expectation
        .given
        .iter()
        .chain(expectation.when.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    (key, state)
}

/// The replay-cache method string for a [`Surface`] — its lowercase serde form
/// (`"cli"`, `"http"`, …), the single source of truth for the surface name.
fn surface_method(surface: Surface) -> String {
    serde_json::to_value(surface)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Drive `expectation`'s withheld-criteria goal through one scoped session and
/// return the agent's structured claim once it declares the goal reached (the
/// **soft stop**, ACP `stopReason: end_turn`).
///
/// The driver's own bounded agentic loop runs *inside* this single ACP prompt
/// turn — the [`AcpGoalDriver`] mints exactly one scoped session per expectation
/// by contract, so `expect` does not re-prompt a spent connection. `expect` owns
/// the **max-prompt-turns hard cap** ([`MAX_PROMPT_TURNS`]) and imposes it on the
/// driver through [`build_driver_goal`]; the deterministic guarantee enforced
/// here is the terminal one: a turn that ends WITHOUT the soft stop (the agent
/// hit its turn/token budget or stopped early, `stopReason` other than
/// `end_turn`) is a clear error, never mistaken for success.
///
/// The stall/idle floor is the pool's, not re-derived here — a wedged turn
/// surfaces as [`ExpectError::Pool`] from `drive_goal` and propagates out
/// immediately.
///
/// # Errors
///
/// Returns [`ExpectError::Agent`] when the turn ends without the agent declaring
/// the goal reached (the max-turns cap), or propagates the driver's own error
/// (including [`ExpectError::Pool`] for a turn the pool abandoned).
async fn drive_for_goal<D: GoalDriver>(
    expectation: &Expectation,
    driver: &D,
) -> Result<serde_json::Value, ExpectError> {
    let goal = build_driver_goal(expectation);
    let turn = driver.drive_goal(&goal).await?;
    if turn.goal_reached {
        Ok(turn.claim)
    } else {
        Err(ExpectError::Agent(format!(
            "the driving subagent ended its turn without declaring the goal reached \
             (it exhausted its {MAX_PROMPT_TURNS}-turn budget or stopped early); a \
             re-validated completion is required to proceed"
        )))
    }
}

/// The grace window [`drive_and_revalidate`] keeps polling the drive after firing
/// the spec-timeout cancel, so the pool's worker actually transmits
/// `session/cancel` over the still-live connection before teardown.
///
/// On the live path the drive resolves well within this window (the cancelled
/// turn returns [`ExpectError::Pool`] promptly), so the wait ends early; it only
/// elapses in full for a driver that ignores the cancel (the stub drivers). It is
/// short relative to the spec budget yet ample for an in-process notification.
const SPEC_TIMEOUT_CANCEL_DRAIN: std::time::Duration = std::time::Duration::from_secs(1);

/// Drive `expectation` toward its goal under both hard caps and render the
/// **independent** deterministic verdict over the adapter-observed state.
///
/// The full stop-conditioned engine entry of `ideas/expect.md`
/// §"Stop conditions" plus hardening rule 2: it bounds the driver loop with two
/// independent stops and never trusts the agent's self-declared completion.
///
/// - **Soft stop + max-turns** — [`observe_with_driver`] drives over
///   [`drive_for_goal`], which ends on the agent's `end_turn` and rejects a turn
///   that exhausts the [`MAX_PROMPT_TURNS`] budget without converging.
/// - **Spec timeout** — the whole drive-and-observe is bounded by the spec's
///   [`timeout`](crate::Frontmatter::timeout) wall clock; exceeding it yields
///   [`ExpectError::Timeout`] rather than a hang, and **actively cancels the
///   in-flight ACP session** ([`GoalDriver::cancel`] → the pool's `session/cancel`
///   path) so the driving agent stops working rather than being orphaned behind
///   the dropped drive future.
/// - **Stall/idle** — delegated to the pool inside `drive_goal`, surfaced as
///   [`ExpectError::Pool`].
///
/// The agent's self-declared "done" is then **re-validated**, never trusted: the
/// returned [`ExpectationVerdict`] is [`evaluate_spec`] replayed over the
/// adapter's authoritative [`Observation`]. A self-declared done whose observed
/// state fails the criteria yields a failing verdict — the agent's COMPLETE is
/// rejected, because the verdict lives in `expect`, never in the agent.
///
/// # Errors
///
/// Returns [`ExpectError::Timeout`] when the spec budget elapses,
/// [`ExpectError::Pool`] when the pool abandons a wedged turn,
/// [`ExpectError::Agent`] when the max-turns cap is hit or the agent cannot be
/// driven, or any [`ExpectError`] the adapter raises while observing.
pub async fn drive_and_revalidate<A, D>(
    expectation: &Expectation,
    adapter: &A,
    config: &ObserveConfig,
    driver: &D,
) -> Result<ExpectationVerdict, ExpectError>
where
    A: SurfaceAdapter,
    D: GoalDriver,
{
    let timeout = expectation.frontmatter.timeout;
    let bounded = async {
        let observation = observe_with_driver(expectation, adapter, config, driver).await?;
        Ok::<_, ExpectError>(evaluate_spec(expectation, &observation))
    };
    tokio::pin!(bounded);
    match tokio::time::timeout(timeout, &mut bounded).await {
        Ok(result) => result,
        Err(_) => {
            // The spec wall-clock budget elapsed. Actively cancel the in-flight
            // ACP session (mirroring the pool's idle/ceiling `session/cancel`)
            // so the driving agent stops working, rather than orphaning it by
            // dropping the drive future — the gap a bare `tokio::time::timeout`
            // leaves (`AgentPool`'s synchronous `Drop` aborts workers but cannot
            // await a cancel over the connection it is dropping). Then keep
            // polling the drive for a brief grace so the pool's worker actually
            // sends `session/cancel` over the still-live connection before
            // teardown; that drained result is discarded because the
            // deterministic outcome of a spec timeout is `Timeout`, not the
            // pool's [`ExpectError::Pool`] abandonment.
            driver.cancel();
            let _ = tokio::time::timeout(SPEC_TIMEOUT_CANCEL_DRAIN, &mut bounded).await;
            Err(ExpectError::Timeout {
                timeout_ms: timeout.as_millis() as u64,
            })
        }
    }
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
    cancel: CancellationToken,
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
        InitializeRequest::new(ACP_PROTOCOL_VERSION.into()).client_capabilities(
            ClientCapabilities::new().fs(FileSystemCapabilities::new()
                .read_text_file(true)
                .write_text_file(true)),
        ),
    )
    .block_task()
    .await?;

    let pool = AgentPool::new_cancellable(cx, notifier, pool_config, cancel);
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
        // The inner pool error is preserved typed (via `From<PoolError>`) rather
        // than stringified, so a liveness abandonment surfaces as
        // `ExpectError::Pool(PoolError::TurnIdle | TurnCeiling)` — the
        // deterministic stall floor `expect` reuses — distinguishable from a
        // genuine agent failure without parsing message text.
        let collected = rx.await.map_err(|e| {
            ExpectError::Agent(format!("the agent pool dropped the turn for `{goal}`: {e}"))
        })??;

        // The soft stop: the agent declared the goal reached only when its turn
        // ended with `end_turn`. A turn cut short by a hard cap reports `false`.
        let goal_reached = matches!(collected.stop_reason, StopReason::EndTurn);
        let json = extract_json_value(&collected.content, '{', '}');
        let structured: serde_json::Value = serde_json::from_str(json).map_err(|e| {
            ExpectError::Agent(format!(
                "the subagent's reply for `{goal}` was not structured JSON: {e}"
            ))
        })?;
        observations.push(DrivenObservation {
            goal,
            structured,
            goal_reached,
        });
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
    use crate::types::{CliState, SurfaceState};

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

    /// The total the stub adapter reports as the system's observed surface JSON.
    /// An arbitrary fixed fixture value, distinct from any authoritative
    /// checkpoint total, shared by the driver tests that stub the surface state.
    const STUB_OBSERVED_TOTAL: u32 = 40;

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
        assert!(
            observations[0].goal_reached,
            "the stub agent ended its turn with `end_turn` — the soft stop is captured"
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
        assert!(
            goal.contains(&MAX_PROMPT_TURNS.to_string()),
            "the driver goal must impose the {MAX_PROMPT_TURNS}-turn budget on the agent: {goal}"
        );
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
        fn drive_goal(&self, _goal: &str) -> impl Future<Output = Result<DriverTurn, ExpectError>> {
            self.invoked.store(true, Ordering::SeqCst);
            async move {
                Ok(DriverTurn {
                    claim: serde_json::json!({}),
                    goal_reached: true,
                })
            }
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

    // ---- replay-cache integration (deterministic replay) ----------------

    /// A [`GoalDriver`] that counts how many times the agent was invoked,
    /// returning a fixed reached-goal claim — so a test can prove the cached
    /// path skips it.
    struct CountingDriver {
        calls: AtomicUsize,
    }

    impl GoalDriver for CountingDriver {
        fn drive_goal(&self, _goal: &str) -> impl Future<Output = Result<DriverTurn, ExpectError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            async move {
                Ok(DriverTurn {
                    claim: serde_json::json!({ "summary": "resolved" }),
                    goal_reached: true,
                })
            }
        }
    }

    /// Parse [`STOP_SPEC`] under `repo_root` so the replay cache persists to a
    /// real writable `.expect/cache` slot.
    fn stop_expectation_under(repo_root: &Path) -> Expectation {
        Expectation::parse(STOP_SPEC, &repo_root.join("feature.expect.md"), repo_root)
            .expect("parse stop spec")
    }

    /// The cached path is the default: a second run with an unchanged
    /// target+state replays the cached action with NO model call.
    #[tokio::test]
    async fn observe_with_driver_replays_the_cached_action_without_the_agent() {
        let repo = temp_repo();
        let adapter = json_stub_adapter(serde_json::json!({ "total": STUB_OBSERVED_TOTAL }));
        let driver = CountingDriver {
            calls: AtomicUsize::new(0),
        };
        let expectation = stop_expectation_under(repo.path());
        let config = ObserveConfig::new(repo.path());

        // First run: a cache miss resolves via the agent and persists the action.
        observe_with_driver(&expectation, &adapter, &config, &driver)
            .await
            .expect("first observe");
        assert_eq!(
            driver.calls.load(Ordering::SeqCst),
            1,
            "the first run resolves via the agent"
        );

        // Second run, unchanged target+state: the cached action replays with no
        // model call (the cache is reloaded fresh from disk).
        let observation = observe_with_driver(&expectation, &adapter, &config, &driver)
            .await
            .expect("second observe");
        assert_eq!(
            driver.calls.load(Ordering::SeqCst),
            1,
            "the second run replays the cached action without the agent"
        );
        assert!(
            !observation
                .trajectory
                .steps
                .iter()
                .any(|step| step.starts_with(DRIFT_STEP_PREFIX)),
            "an unchanged replay is not surfaced as drift"
        );
    }

    /// A changed state snapshot drifts the cached action past the threshold: the
    /// agent re-resolves AND the drift is surfaced, not silently applied.
    #[tokio::test]
    async fn observe_with_driver_re_resolves_and_surfaces_drift_on_state_change() {
        let repo = temp_repo();
        let adapter = json_stub_adapter(serde_json::json!({ "total": STUB_OBSERVED_TOTAL }));
        let driver = CountingDriver {
            calls: AtomicUsize::new(0),
        };
        let config = ObserveConfig::new(repo.path());

        let first = stop_expectation_under(repo.path());
        observe_with_driver(&first, &adapter, &config, &driver)
            .await
            .expect("first observe");
        assert_eq!(driver.calls.load(Ordering::SeqCst), 1);

        // The `When` steps the agent acts on change: the cached action's state
        // fingerprint drifts, so the agent re-resolves and the drift surfaces.
        let mut drifted = first.clone();
        drifted.when = vec!["a completely different action to perform now".to_string()];
        let observation = observe_with_driver(&drifted, &adapter, &config, &driver)
            .await
            .expect("drift observe");

        assert_eq!(
            driver.calls.load(Ordering::SeqCst),
            2,
            "a drifted state re-resolves via the agent"
        );
        assert!(
            observation
                .trajectory
                .steps
                .iter()
                .any(|step| step.starts_with(DRIFT_STEP_PREFIX)),
            "the drift re-resolve is surfaced in the trajectory: {:?}",
            observation.trajectory.steps
        );
    }

    // ---- stop conditions + independent re-validation ---------------------
    //
    // The two independent stops the driver loop owns (`ideas/expect.md`
    // §"Stop conditions") plus hardening rule 2 — the agent's self-declared
    // done is re-validated, never trusted — each pinned with a stub agent or
    // stub driver, no real model.

    use agent_client_protocol::schema::CancelNotification;
    use swissarmyhammer_validators::PoolError;

    /// A short idle window for the stall test: long enough to arm after the
    /// stub's first chunk, short enough to trip well inside [`PIPELINE_TIMEOUT`].
    const STALL_IDLE_WINDOW: Duration = Duration::from_millis(300);

    /// An absolute ceiling far above [`STALL_IDLE_WINDOW`] so the wedged turn
    /// trips the idle window, not the ceiling.
    const STALL_TURN_CEILING: Duration = Duration::from_secs(30);

    /// How many times [`await_recorded_cancel`] polls for a recorded
    /// `session/cancel` before failing, and the wait between polls. Their product
    /// (2s) comfortably outlasts the one-way notification's in-process delivery.
    const CANCEL_DELIVERY_POLLS: usize = 200;
    const CANCEL_DELIVERY_POLL_INTERVAL: Duration = Duration::from_millis(10);

    /// Assert the in-flight session was actively cancelled: poll `cancelled`
    /// (the [`StallingAgent`]'s record of every `session/cancel` it received)
    /// until it is non-empty, or fail.
    ///
    /// The cancel is a one-way ACP notification, so it can land slightly after
    /// the abandonment/timeout error returns; this gives delivery a bounded
    /// window rather than racing a single check. Shared by the idle-abandonment
    /// and spec-timeout tests, which both prove `session/cancel` is sent.
    async fn await_recorded_cancel(cancelled: &Arc<Mutex<Vec<String>>>) {
        for _ in 0..CANCEL_DELIVERY_POLLS {
            if !cancelled.lock().expect("cancel recorder mutex").is_empty() {
                return;
            }
            tokio::time::sleep(CANCEL_DELIVERY_POLL_INTERVAL).await;
        }
        panic!("a session/cancel must be sent for the abandoned in-flight session");
    }

    /// A spec carrying one `When` step and one deterministic `Then` criterion, so
    /// a [`StubAdapter`] with `resolves = false` forces the agent-fallback drive
    /// and the criterion can be graded against the adapter's observed state.
    const STOP_SPEC: &str = r#"---
description: a stop-conditions spec
surface: cli
---

Drive the system to a known total.

## When
- perform the action

## Then
- [ ] the total is $40
"#;

    /// Parse [`STOP_SPEC`] into an [`Expectation`] addressed under `/repo`.
    fn stop_expectation() -> Expectation {
        Expectation::parse(
            STOP_SPEC,
            Path::new("/repo/feature.expect.md"),
            Path::new("/repo"),
        )
        .expect("parse stop spec")
    }

    /// A [`StubAdapter`] over `body` whose `When` steps are NOT mechanical, so the
    /// agent-fallback drive is exercised and the criterion is graded against the
    /// authoritative `body`.
    fn json_stub_adapter(body: serde_json::Value) -> StubAdapter {
        StubAdapter {
            resolves: false,
            state: SurfaceState::Json { body },
        }
    }

    /// A stub ACP agent that streams one chunk to arm the pool's idle window,
    /// then goes silent far past it — modelling a turn that started decoding then
    /// wedged (e.g. an unanswered nested request). It records the sessions it is
    /// asked to cancel so the test can prove the abandonment sent `session/cancel`.
    struct StallingAgent {
        next_session: AtomicUsize,
        notify_tx: broadcast::Sender<SessionNotification>,
        cancelled: Arc<Mutex<Vec<String>>>,
    }

    impl MockAgent for StallingAgent {
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
                // One chunk arms the idle window; then stay silent far longer than
                // it so the pool abandons the turn as idle (not at the ceiling).
                let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(
                    ContentBlock::Text(TextContent::new("working...".to_string())),
                ));
                let _ = self
                    .notify_tx
                    .send(SessionNotification::new(request.session_id.clone(), update));
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(PromptResponse::new(StopReason::EndTurn))
            })
        }

        fn cancel<'a>(
            &'a self,
            notification: CancelNotification,
        ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
            self.cancelled
                .lock()
                .expect("cancel recorder mutex")
                .push(notification.session_id.to_string());
            Box::pin(async move { Ok(()) })
        }
    }

    /// A stub [`GoalDriver`] that returns a scripted [`DriverTurn`], optionally
    /// after a delay, without standing up a real agent — so the bounded-loop,
    /// timeout, and re-validation stops can be driven deterministically.
    struct ScriptedDriver {
        /// Whether each turn declares the goal reached (the soft stop).
        goal_reached: bool,
        /// Delay before each turn responds — used to outrun the spec timeout.
        delay: Duration,
        /// The structured claim each turn reports — the agent's self-account,
        /// recorded in the trajectory and re-validated, never trusted.
        claim: serde_json::Value,
    }

    impl GoalDriver for ScriptedDriver {
        fn drive_goal(&self, _goal: &str) -> impl Future<Output = Result<DriverTurn, ExpectError>> {
            let goal_reached = self.goal_reached;
            let delay = self.delay;
            let claim = self.claim.clone();
            async move {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                Ok(DriverTurn {
                    claim,
                    goal_reached,
                })
            }
        }
    }

    /// The default scripted claim: the agent's own account of driving the SUT.
    /// Distinct from the adapter's authoritative read, so a verdict graded over it
    /// rather than the checkpoints would be a tamper-resistance failure.
    fn drove_the_sut_claim() -> serde_json::Value {
        serde_json::json!({ "summary": "drove the SUT" })
    }

    /// Stall: a driven turn that arms the idle window then goes silent is
    /// abandoned via `session/cancel` and surfaces as [`ExpectError::Pool`]
    /// wrapping [`PoolError::TurnIdle`] — the deterministic stall floor reused
    /// from the pool, not re-derived — rather than hanging the run.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stop_conditions_idle_turn_is_abandoned_as_turn_idle() {
        let repo = temp_repo();
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let cancelled = Arc::new(Mutex::new(Vec::new()));
        let agent = Arc::new(StallingAgent {
            next_session: AtomicUsize::new(0),
            notify_tx,
            cancelled: Arc::clone(&cancelled),
        });
        let driver = AcpGoalDriver::new(
            DriverHandle {
                agent: DynConnectTo::new(MockAgentAdapter(agent)),
                notification_rx,
            },
            repo.path(),
            PoolConfig::local()
                .with_idle_timeout(STALL_IDLE_WINDOW)
                .with_turn_ceiling(STALL_TURN_CEILING),
        );

        let err = tokio::time::timeout(PIPELINE_TIMEOUT, driver.drive_goal("observe the SUT"))
            .await
            .expect("the abandoned turn must not hang")
            .expect_err("a wedged turn must surface as an error, not a success");

        assert!(
            matches!(err, ExpectError::Pool(PoolError::TurnIdle { .. })),
            "a stalled turn must surface as the typed idle-abandonment outcome, got: {err:?}"
        );

        // The abandonment actively cancelled the in-flight session; the cancel is
        // a one-way notification, so allow it a bounded window to land.
        await_recorded_cancel(&cancelled).await;
    }

    /// Max-turns hard cap: a turn that ends without the agent declaring the goal
    /// reached (it exhausted its [`MAX_PROMPT_TURNS`] budget or stopped early)
    /// terminates the drive with a clear error naming the cap — not a hang, and
    /// not mistaken for success.
    #[tokio::test]
    async fn stop_conditions_max_turns_cap_terminates_with_a_clear_error() {
        let adapter = json_stub_adapter(serde_json::json!({ "total": STUB_OBSERVED_TOTAL }));
        let driver = ScriptedDriver {
            goal_reached: false,
            delay: Duration::ZERO,
            claim: drove_the_sut_claim(),
        };
        let expectation = stop_expectation();
        let config = ObserveConfig::new("/repo");

        let err = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            drive_and_revalidate(&expectation, &adapter, &config, &driver),
        )
        .await
        .expect("the bounded loop must not hang")
        .expect_err("an agent that never declares done must hit the max-turns cap");

        match err {
            ExpectError::Agent(message) => assert!(
                message.contains(&MAX_PROMPT_TURNS.to_string()),
                "the error must name the {MAX_PROMPT_TURNS}-turn cap, got: {message}"
            ),
            other => panic!("max-turns must terminate with a clear agent error, got: {other:?}"),
        }
    }

    /// Spec-timeout hard cap: a drive that outruns the spec's wall-clock budget
    /// is aborted with [`ExpectError::Timeout`] carrying that budget, not left to
    /// hang.
    #[tokio::test]
    async fn stop_conditions_spec_timeout_terminates_with_a_clear_error() {
        let adapter = json_stub_adapter(serde_json::json!({ "total": STUB_OBSERVED_TOTAL }));
        // The driver sleeps far longer than the spec budget, so the wall clock —
        // not the agent — ends the run.
        let driver = ScriptedDriver {
            goal_reached: true,
            delay: Duration::from_secs(30),
            claim: drove_the_sut_claim(),
        };
        let mut expectation = stop_expectation();
        expectation.frontmatter.timeout = Duration::from_millis(100);
        let config = ObserveConfig::new("/repo");

        let err = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            drive_and_revalidate(&expectation, &adapter, &config, &driver),
        )
        .await
        .expect("the timeout cap must fire well inside the test budget")
        .expect_err("a drive that outruns the spec budget must time out");

        match err {
            ExpectError::Timeout { timeout_ms } => assert_eq!(
                timeout_ms,
                expectation.frontmatter.timeout.as_millis() as u64,
                "the timeout error must carry the spec's wall-clock budget"
            ),
            other => panic!("the timeout cap must surface as ExpectError::Timeout, got: {other:?}"),
        }
    }

    /// Independent re-validation: an agent that declares the goal reached (the
    /// soft stop) but whose adapter-observed state fails the criteria yields a
    /// FAILING verdict — the self-declared done is REJECTED, because the verdict
    /// lives in `expect`, never in the agent (hardening rule 2).
    /// Drive `expectation`'s self-declared-done turn over an adapter reporting
    /// `state`, and assert the verdict REJECTS the claim. The agent reaches its
    /// soft stop and reports a claim that *names the passing value*, yet the
    /// verdict is graded over the adapter's authoritative observation: no
    /// interpretation of the one `Then` criterion holds against `state`, so
    /// [`compile`](crate::compile) yields no binding assertion and the criterion
    /// is a non-pass — the verdict fails.
    ///
    /// This pins hardening rule 2 (`ideas/expect.md` §"The Check Loop"): the
    /// verdict lives in `expect`, never in the agent. [`evaluate`] compiles and
    /// grades only against `Observation.checkpoints[*].state`, never the claim
    /// recorded in `trajectory.steps`, so a claim that *would* satisfy the
    /// criterion as text cannot rescue a verdict the observed state fails.
    async fn assert_self_declared_done_rejected_over(state: SurfaceState) {
        let adapter = StubAdapter {
            resolves: false,
            state,
        };
        // The claim names the passing total ("the total is $40"), so a verdict
        // that wrongly read the claim would pass; it must fail because no `$40`
        // interpretation holds against the authoritative checkpoints (total 50 /
        // `Total: $50`).
        let driver = ScriptedDriver {
            goal_reached: true,
            delay: Duration::ZERO,
            claim: serde_json::json!({ "summary": "the total is $40 — done" }),
        };
        let expectation = stop_expectation();
        let config = ObserveConfig::new("/repo");

        let verdict = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            drive_and_revalidate(&expectation, &adapter, &config, &driver),
        )
        .await
        .expect("re-validation must not hang")
        .expect("re-validation produces a verdict even when the agent's claim fails");

        assert_eq!(
            verdict.criteria.len(),
            1,
            "the one Then criterion was graded"
        );
        assert!(
            !verdict.criteria[0].pass,
            "the criterion fails against the authoritative state, not the claim: {:?}",
            verdict.criteria
        );
        assert!(
            !verdict.reliability.satisfied(),
            "the self-declared done must be rejected: the verdict must not be satisfied"
        );
    }

    /// Independent re-validation over a **JSON-field** criterion: a self-declared
    /// done is rejected because no `$40` interpretation holds against the
    /// authoritative `$.total` (50), so the criterion compiles to no binding
    /// assertion and grades non-pass.
    #[tokio::test]
    async fn stop_conditions_self_declared_done_is_rejected_for_a_json_field_criterion() {
        assert_self_declared_done_rejected_over(SurfaceState::Json {
            body: serde_json::json!({ "total": 50 }),
        })
        .await;
    }

    /// Independent re-validation over a **stdout TEXT** criterion (acceptance:
    /// "not just a JSON field"): against the cli checkpoint's `stdout`
    /// (`Total: $50`), no `$40` stream-text interpretation holds, so the criterion
    /// grades non-pass. The agent's "the total is $40" claim — recorded only in
    /// the trajectory, never as a checkpoint — is never consulted, so the
    /// self-declared done is rejected. This exercises `compile`'s cli-stdout
    /// candidate path, distinct from the JSON-field case's structured-value path.
    #[tokio::test]
    async fn stop_conditions_self_declared_done_is_rejected_for_a_stdout_text_criterion() {
        assert_self_declared_done_rejected_over(SurfaceState::Cli(CliState {
            stdout: "Total: $50\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            files: std::collections::BTreeMap::new(),
        }))
        .await;
    }

    /// The window the spec-timeout drive must return within: far below the
    /// wedged agent's 60s sleep and the pool's [`STALL_TURN_CEILING`], so a pass
    /// proves the spec budget — not the agent or the pool floor — aborted the
    /// in-flight drive.
    const SPEC_TIMEOUT_ABORT_WINDOW: Duration = Duration::from_secs(5);

    /// The spec budget for the spec-timeout-over-ACP test: small enough that the
    /// wall clock ends the drive well before the wedged agent's 60s sleep or the
    /// pool's [`STALL_TURN_CEILING`], yet large enough that the scoped session is
    /// reliably established (so there is an in-flight session to cancel) before
    /// the budget elapses.
    const SPEC_TIMEOUT_BUDGET: Duration = Duration::from_millis(500);

    /// Spec-timeout over a **real mock-ACP session**: a drive whose agent wedges
    /// (one chunk, then silent far past the spec budget) is aborted with
    /// [`ExpectError::Timeout`] carrying the spec's wall-clock budget — promptly,
    /// not left to hang on the agent's sleep or the pool's stall floor — AND the
    /// in-flight ACP session is actively cancelled (`session/cancel` sent) so the
    /// agent stops working rather than being orphaned. Unlike
    /// [`stop_conditions_spec_timeout_terminates_with_a_clear_error`], which uses a
    /// stub [`ScriptedDriver`], this drives a real [`AcpGoalDriver`] over the
    /// [`StallingAgent`], so the [`AgentPool`] teardown path is exercised end to
    /// end.
    ///
    /// `^gg00rxf` review Finding 3: a bare `tokio::time::timeout` on the spec
    /// budget only DROPS the drive future, which tears the pool's workers down
    /// but does NOT send `session/cancel` ([`AgentPool`]'s `Drop` is synchronous
    /// and cannot await the cancel over the connection it is dropping). The fix
    /// has [`drive_and_revalidate`] fire the driver's pool cancel handle on the
    /// timeout branch, reusing the pool's existing idle/ceiling cancel path; this
    /// test pins that the cancel is actually sent, in addition to the typed
    /// timeout.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn spec_timeout_over_an_acp_session_cancels_the_in_flight_session() {
        let repo = temp_repo();
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let cancelled = Arc::new(Mutex::new(Vec::new()));
        let agent = Arc::new(StallingAgent {
            next_session: AtomicUsize::new(0),
            notify_tx,
            cancelled: Arc::clone(&cancelled),
        });
        let driver = AcpGoalDriver::new(
            DriverHandle {
                agent: DynConnectTo::new(MockAgentAdapter(agent)),
                notification_rx,
            },
            repo.path(),
            // A generous idle window and ceiling so the SPEC timeout — not the
            // pool's stall floor — is the stop that fires.
            PoolConfig::local()
                .with_idle_timeout(STALL_TURN_CEILING)
                .with_turn_ceiling(STALL_TURN_CEILING),
        );

        // `resolves = false` routes the When step through the wedged subagent; the
        // spec budget is tiny so the wall clock ends the drive well before the
        // agent's 60s sleep or the pool's stall floor.
        let adapter = json_stub_adapter(serde_json::json!({ "total": STUB_OBSERVED_TOTAL }));
        let mut expectation = stop_expectation_under(repo.path());
        expectation.frontmatter.timeout = SPEC_TIMEOUT_BUDGET;
        let config = ObserveConfig::new(repo.path());

        let started = std::time::Instant::now();
        let err = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            drive_and_revalidate(&expectation, &adapter, &config, &driver),
        )
        .await
        .expect("the spec-timeout stop must fire well inside the test budget")
        .expect_err("a drive that outruns the spec budget must time out, not hang");
        let elapsed = started.elapsed();

        match err {
            ExpectError::Timeout { timeout_ms } => assert_eq!(
                timeout_ms,
                expectation.frontmatter.timeout.as_millis() as u64,
                "the timeout error must carry the spec's wall-clock budget"
            ),
            other => {
                panic!("the spec timeout must surface as ExpectError::Timeout, got: {other:?}")
            }
        }
        assert!(
            elapsed < SPEC_TIMEOUT_ABORT_WINDOW,
            "the spec timeout must abort the in-flight drive promptly ({elapsed:?}), not wait \
             on the wedged agent or the pool's stall floor"
        );

        // The spec timeout must ACTIVELY cancel the in-flight session, not merely
        // drop the drive future — mirroring the pool's idle/ceiling cancel path.
        await_recorded_cancel(&cancelled).await;
    }
}
