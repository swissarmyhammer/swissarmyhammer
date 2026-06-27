//! The agent seam for the `expect` tool: resolve a driving agent, gate the
//! pipeline, and drive the engine off the async-trait executor.
//!
//! This is the `expect` mirror of `review`'s `review_op` seam. It owns the three
//! pieces the engine deliberately leaves to its caller:
//!
//! - The [`AgentHandle`] / [`AgentFactory`] seam — the engine
//!   ([`run_expect_over_agent`](swissarmyhammer_expect::run_expect_over_agent))
//!   takes a ready `DynConnectTo<Client>` + notifier and constructs no agent, so
//!   the production server injects a factory that builds the configured backend
//!   while tests inject a scripted agent.
//! - The process-global [`EXPECT_PIPELINE_GATE`] — one permit serializes whole
//!   `expect` pipelines so concurrent runs do not multiply the resident agent +
//!   model footprint (the same rationale as review's pipeline gate).
//! - The spawn-blocking-on-a-current-thread-runtime pattern — the pipeline drives
//!   an ACP connection across `await`s, so it runs on a dedicated current-thread
//!   runtime on a blocking thread, keeping non-`Send` futures off the shared
//!   async-trait executor (the same pattern `review_op::run_review_request` uses).
//!
//! The driving agent is resolved from the session via
//! [`expectations_agent`], which asks [`ToolContext`] for the
//! [`AgentUseCase::Expectations`] agent (falling back to root when unconfigured).

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use agent_client_protocol::schema::SessionNotification;
use agent_client_protocol::{Client, DynConnectTo};
use tokio::sync::{broadcast, Semaphore};

use swissarmyhammer_config::{AgentUseCase, ModelConfig};
use swissarmyhammer_expect::{
    create, drive_and_revalidate, parse_draft, render_authoring_goal, run_expect_over_agent,
    AcpGoalDriver, AuthoringRequest, CreateOutcome, CreateSource, DoctorFacts, DraftSpec,
    DrivenObservation, DriverHandle, ExpectError, ExpectScope, Expectation, ExpectationVerdict,
    GoalDriver, ObserveConfig, SpecAuthor, SurfaceAdapter,
};
use swissarmyhammer_validators::PoolConfig;

use crate::mcp::tool_registry::ToolContext;

/// The two halves of a ready-to-drive ACP agent handle: its
/// [`DynConnectTo<Client>`] component and the broadcast receiver of its streamed
/// `session/update` notifications.
///
/// This is exactly the shape of `swissarmyhammer_agent::AcpAgentHandle`, supplied
/// to the tool so this crate's `expect` engine never constructs an agent itself.
#[derive(Debug)]
pub struct AgentHandle {
    /// The agent component the driver runs as the ACP server side.
    pub agent: DynConnectTo<Client>,
    /// The receiver of the agent's streamed notifications.
    pub notification_rx: broadcast::Receiver<SessionNotification>,
}

/// A factory that mints a fresh [`AgentHandle`] for one `expect` run.
///
/// The tool resolves its agent through this seam rather than constructing one
/// inline: the production server injects a factory that builds the configured
/// backend from the resolved [`ModelConfig`], while tests inject a scripted ACP
/// agent. The factory is async and fallible — a backend that fails to start
/// surfaces as a tool error.
pub type AgentFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<AgentHandle, String>> + Send>> + Send + Sync,
>;

/// Process-global cap on concurrent `expect` pipelines.
///
/// A single `expect` run already fans out internally across its
/// [`AgentPool`](swissarmyhammer_validators::AgentPool); running many pipelines
/// at once instead multiplies the per-run footprint — each holds its own agent
/// (and, later, embedding corpus). One permit serializes pipelines so only one
/// such resource set is resident at a time; throughput is preserved by the in-run
/// fan-out, which this does not touch. This mirrors review's `REVIEW_PIPELINE_GATE`.
static EXPECT_PIPELINE_GATE: Semaphore = Semaphore::const_new(1);

/// Resolve the driving agent for the [`AgentUseCase::Expectations`] use case,
/// falling back to the root agent when it is unconfigured.
///
/// `expect` needs a *driving* agent distinct from the *grading* model; this is
/// the single resolution point the tool consults so the production server's
/// `[agent]` mapping (or its absence) governs which backend drives the system
/// under test. The fallback to root is the design's chosen behavior
/// (`ideas/rule_agent.md`, "Design Decisions").
pub fn expectations_agent(context: &ToolContext) -> &ModelConfig {
    context.get_agent_for_use_case(AgentUseCase::Expectations)
}

/// Run an `expect` scope end to end over a freshly-minted agent, behind the
/// pipeline gate, and return each driven subagent's structured capture.
///
/// The gate permit is acquired here, *outside* the `spawn_blocking`, so a second
/// concurrent request waits before it builds any agent. The pipeline then runs on
/// a dedicated current-thread runtime on a blocking thread (the same pattern
/// `review_op::run_review_request` uses) because it drives an ACP connection
/// across `await`s and must stay off the shared async-trait executor.
///
/// # Errors
///
/// Returns a message on agent-construction failure or a pipeline error.
pub async fn run_expect_request(
    scope: ExpectScope,
    repo_path: PathBuf,
    pool_config: PoolConfig,
    agent_factory: AgentFactory,
) -> Result<Vec<DrivenObservation>, String> {
    // Serialize `expect` pipelines process-wide: hold a permit for the whole run
    // so only one agent set is resident at a time (see `EXPECT_PIPELINE_GATE`).
    let _permit = EXPECT_PIPELINE_GATE
        .acquire()
        .await
        .map_err(|e| format!("expect pipeline gate closed: {e}"))?;

    // Carry the current span across the thread boundary so the engine's
    // observability lines stay correlated with the originating tool-call span.
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to build expect runtime: {e}"))?;
        rt.block_on(run_expect_request_inner(
            scope,
            repo_path,
            pool_config,
            agent_factory,
        ))
    })
    .await
    .map_err(|e| format!("expect task join error: {e}"))?
}

/// The pipeline body, run inside the dedicated current-thread runtime: mint the
/// agent, then hand both halves of its handle to the engine.
async fn run_expect_request_inner(
    scope: ExpectScope,
    repo_path: PathBuf,
    pool_config: PoolConfig,
    agent_factory: AgentFactory,
) -> Result<Vec<DrivenObservation>, String> {
    let handle = agent_factory().await?;
    run_expect_over_agent(
        handle.agent,
        handle.notification_rx,
        scope,
        &repo_path,
        pool_config,
    )
    .await
    .map_err(|e| format!("expect pipeline failed: {e}"))
}

/// Drive one expectation toward its goal under the spec's stop conditions and
/// return the **independent** re-validated [`ExpectationVerdict`], behind the
/// pipeline gate, over a freshly-minted agent and the chosen surface `adapter`.
///
/// The stop-conditioned mirror of [`run_expect_request`]: where that returns each
/// driven subagent's raw [`DrivenObservation`], this threads the [`Expectation`]
/// and its [`SurfaceAdapter`] through
/// [`drive_and_revalidate`](swissarmyhammer_expect::drive_and_revalidate) so the
/// verdict is produced over the adapter's authoritative observation — never the
/// agent's self-declared claim. The agent is consulted only for a `When` step the
/// adapter cannot resolve mechanically (the agent-fallback gate,
/// [`SurfaceAdapter::resolves_mechanically`]); on a deterministic surface the
/// adapter drives every step and the minted agent is left unused.
///
/// The gate permit, the spawn-blocking current-thread runtime, and the
/// [`AgentFactory`] seam are exactly those of [`run_expect_request`]: the permit
/// is held for the whole run so only one agent set is resident, and the
/// `!Send` ACP drive runs off the shared async-trait executor.
///
/// # Errors
///
/// Returns a message on agent-construction failure or any [`ExpectError`] the
/// drive raises — a spec-timeout stop, a pool abandonment, the max-turns cap, or
/// an adapter failure while observing.
pub async fn run_drive_request<A>(
    expectation: Expectation,
    adapter: A,
    repo_path: PathBuf,
    pool_config: PoolConfig,
    agent_factory: AgentFactory,
) -> Result<ExpectationVerdict, String>
where
    A: SurfaceAdapter + Send + 'static,
{
    // Serialize `expect` pipelines process-wide: hold a permit for the whole run
    // so only one agent set is resident at a time (see `EXPECT_PIPELINE_GATE`).
    let _permit = EXPECT_PIPELINE_GATE
        .acquire()
        .await
        .map_err(|e| format!("expect pipeline gate closed: {e}"))?;

    // Carry the current span across the thread boundary so the engine's
    // observability lines stay correlated with the originating tool-call span.
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to build expect runtime: {e}"))?;
        rt.block_on(run_drive_request_inner(
            expectation,
            adapter,
            repo_path,
            pool_config,
            agent_factory,
        ))
    })
    .await
    .map_err(|e| format!("expect drive task join error: {e}"))?
}

/// The drive body, run inside the dedicated current-thread runtime: mint the
/// agent into a single-session [`AcpGoalDriver`], then drive and re-validate the
/// expectation over the adapter.
async fn run_drive_request_inner<A>(
    expectation: Expectation,
    adapter: A,
    repo_path: PathBuf,
    pool_config: PoolConfig,
    agent_factory: AgentFactory,
) -> Result<ExpectationVerdict, String>
where
    A: SurfaceAdapter,
{
    let handle = agent_factory().await?;
    let driver = AcpGoalDriver::new(
        DriverHandle {
            agent: handle.agent,
            notification_rx: handle.notification_rx,
        },
        repo_path.clone(),
        pool_config,
    );
    let config = ObserveConfig::new(repo_path);
    drive_and_revalidate(&expectation, &adapter, &config, &driver)
        .await
        .map_err(|e| format!("expect drive failed: {e}"))
}

/// A [`SpecAuthor`] backed by the ACP agent seam — the production authoring agent.
///
/// Each draft is authored over a **fresh scoped session**: an
/// [`AcpGoalDriver`](swissarmyhammer_expect::AcpGoalDriver) is single-use per
/// session, and the green-loop calls [`author`](SpecAuthor::author) once per repair
/// turn, so a new agent is minted (via the [`AgentFactory`]) for every call. The
/// request is rendered to a goal with
/// [`render_authoring_goal`](swissarmyhammer_expect::render_authoring_goal), the
/// agent is driven, and its structured reply is parsed back into a [`DraftSpec`]
/// with [`parse_draft`](swissarmyhammer_expect::parse_draft).
struct AgentSpecAuthor {
    /// Mints a fresh agent handle per authoring turn.
    factory: AgentFactory,
    /// The repo root the authoring subagent's reads are confined under.
    repo_root: PathBuf,
    /// The pool sizing for the single scoped authoring session.
    pool_config: PoolConfig,
}

impl SpecAuthor for AgentSpecAuthor {
    fn author(
        &self,
        request: &AuthoringRequest,
    ) -> impl Future<Output = Result<DraftSpec, ExpectError>> {
        let goal = render_authoring_goal(request);
        let factory = Arc::clone(&self.factory);
        let repo_root = self.repo_root.clone();
        let pool_config = self.pool_config;
        async move {
            let handle = factory().await.map_err(ExpectError::Agent)?;
            let driver = AcpGoalDriver::new(
                DriverHandle {
                    agent: handle.agent,
                    notification_rx: handle.notification_rx,
                },
                repo_root,
                pool_config,
            );
            let turn = driver.drive_goal(&goal).await?;
            parse_draft(&turn.claim)
        }
    }
}

/// Author one expectation from `source` end to end behind the pipeline gate, on
/// the spawn-blocking current-thread runtime, and return the [`CreateOutcome`].
///
/// The same gate + spawn-blocking pattern as [`run_expect_request`]: authoring
/// drives an ACP connection across `await`s (`!Send`), so it runs off the shared
/// async-trait executor, and the gate serializes whole pipelines. The green-loop
/// itself ([`create`](swissarmyhammer_expect::create)) mints a fresh agent per
/// draft via [`AgentSpecAuthor`].
///
/// # Errors
///
/// Returns a message on agent-construction failure, a draft that cannot be made
/// doctor-green within the repair budget, or a write failure.
pub async fn run_create_request(
    source: CreateSource,
    repo_root: PathBuf,
    facts: DoctorFacts,
    pool_config: PoolConfig,
    agent_factory: AgentFactory,
) -> Result<CreateOutcome, String> {
    let _permit = EXPECT_PIPELINE_GATE
        .acquire()
        .await
        .map_err(|e| format!("expect pipeline gate closed: {e}"))?;

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to build expect runtime: {e}"))?;
        rt.block_on(async move {
            let author = AgentSpecAuthor {
                factory: agent_factory,
                repo_root: repo_root.clone(),
                pool_config,
            };
            create(&source, &repo_root, &facts, &author)
                .await
                .map_err(|e| format!("expect create failed: {e}"))
        })
    })
    .await
    .map_err(|e| format!("expect create task join error: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use std::path::Path;

    use swissarmyhammer_expect::{Setup, SurfaceState};
    use swissarmyhammer_validators::review::test_support::{
        ScriptedAdapter, ScriptedAgent, ScriptedAgentConfig, ScriptedReply,
    };

    use crate::mcp::tool_handlers::ToolHandlers;

    /// Notification-channel capacity for the scripted factory — comfortably above
    /// any test's notification volume so a slow subscriber never lags chunks away.
    const SCRIPTED_BROADCAST_CAPACITY: usize = 64;

    /// The goal the scripted agent is keyed on, and the structured reply it
    /// streams back when driven with it.
    const TEST_GOAL: &str = "observe src/checkout/coupon";
    const TEST_REPLY: &str = r#"{"path": "src/checkout/coupon", "verdict": "pass"}"#;

    /// Build a [`ToolContext`] whose root agent and use-case map are both
    /// configured, mirroring the `tool_registry` resolution tests.
    fn context_with_agents(
        root: ModelConfig,
        use_case_agents: HashMap<AgentUseCase, ModelConfig>,
    ) -> ToolContext {
        let git_ops = Arc::new(tokio::sync::Mutex::new(None));
        let tool_handlers = Arc::new(ToolHandlers::new());
        ToolContext::new(tool_handlers, git_ops, Arc::new(root))
            .with_use_case_agents(Arc::new(use_case_agents))
    }

    /// `expectations_agent` resolves the configured Expectations agent.
    #[test]
    fn expectations_agent_resolves_the_configured_agent() {
        let expectations = ModelConfig {
            quiet: true,
            ..ModelConfig::default()
        };
        let root = ModelConfig {
            quiet: false,
            ..ModelConfig::default()
        };
        let mut map = HashMap::new();
        map.insert(AgentUseCase::Expectations, expectations.clone());
        let context = context_with_agents(root, map);

        assert_eq!(
            expectations_agent(&context).quiet,
            expectations.quiet,
            "the configured Expectations agent must be resolved"
        );
    }

    /// `expectations_agent` falls back to the root agent when Expectations is
    /// unconfigured.
    #[test]
    fn expectations_agent_falls_back_to_root_when_unconfigured() {
        let root = ModelConfig {
            quiet: true,
            ..ModelConfig::default()
        };
        let context = context_with_agents(root.clone(), HashMap::new());

        assert_eq!(
            expectations_agent(&context).quiet,
            root.quiet,
            "an unconfigured Expectations use case must fall back to root"
        );
    }

    /// Adapt a scripted agent into an [`AgentFactory`], opening a fresh broadcast
    /// per connection so the minted handle is shaped like a real `AcpAgentHandle`
    /// (streams onto the backend broadcast AND bridges onto the live connection).
    fn scripted_factory(agent: Arc<ScriptedAgent>) -> AgentFactory {
        Arc::new(move || {
            let agent = Arc::clone(&agent);
            Box::pin(async move {
                let (notify_tx, notification_rx) = broadcast::channel(SCRIPTED_BROADCAST_CAPACITY);
                let agent = ScriptedAgent::rebind_broadcast(&agent, notify_tx, true);
                Ok(AgentHandle {
                    agent: DynConnectTo::new(ScriptedAdapter(agent)),
                    notification_rx,
                })
            })
        })
    }

    /// `run_expect_request` mints the agent through the factory, runs the pipeline
    /// behind the gate on the spawn-blocking runtime, and returns the subagent's
    /// captured structured reply — the full tool-layer glue end to end.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_expect_request_drives_the_scope_through_the_seam() {
        let repo = tempfile::TempDir::new().expect("temp repo");
        let agent = ScriptedAgent::with_config(
            vec![(
                TEST_GOAL.to_string(),
                ScriptedReply::Text(TEST_REPLY.to_string()),
            )],
            ScriptedAgentConfig::default(),
        );
        let factory = scripted_factory(agent);

        let scope = ExpectScope {
            goals: vec![TEST_GOAL.to_string()],
        };
        let observations = run_expect_request(
            scope,
            repo.path().to_path_buf(),
            PoolConfig::remote(1),
            factory,
        )
        .await
        .expect("the expect request must produce observations");

        assert_eq!(observations.len(), 1, "exactly one goal was driven");
        assert_eq!(observations[0].goal, TEST_GOAL);
        assert_eq!(
            observations[0].structured["verdict"], "pass",
            "the subagent's structured reply is captured: {:?}",
            observations[0].structured
        );
    }

    /// A repo-relative cli spec carrying one `When` step and one deterministic
    /// `Then` criterion, so a [`StubAdapter`] with a non-mechanical step forces
    /// the agent-fallback drive and the criterion is graded against the adapter's
    /// observed state.
    const DRIVE_SPEC: &str = "---\ndescription: a drive-and-revalidate spec\nsurface: cli\n---\n\nDrive the system to a known total.\n\n## When\n- perform the action\n\n## Then\n- [ ] the total is $40\n";

    /// The stable preamble substring present in every driver goal
    /// ([`build_driver_goal`](swissarmyhammer_expect::build_driver_goal)), used to
    /// key the scripted agent's reply regardless of the rendered goal body.
    const DRIVER_GOAL_NEEDLE: &str = "driving a system under test";

    /// A stub surface adapter whose checkpoints are a fixed authoritative read.
    /// `resolves_mechanically` returns `false`, so every `When` step is routed
    /// through the scoped subagent — the agent-fallback path
    /// [`drive_and_revalidate`](swissarmyhammer_expect::drive_and_revalidate)
    /// exercises — while the adapter still reads the ground-truth `state`.
    struct StubAdapter {
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
            false
        }
    }

    /// `run_drive_request` mints the agent through the factory, drives the
    /// expectation under the spec's stop conditions, and re-validates over the
    /// adapter's authoritative observation — producing the verdict, not the raw
    /// `DrivenObservation`. The agent's claim never decides the verdict: it is
    /// graded against the adapter's observed `total = 40`. This is the production
    /// op-layer seam wiring `drive_and_revalidate` into the live tool, exercised
    /// over a real `AcpGoalDriver` and a real `SurfaceAdapter` (not a stub driver).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_drive_request_revalidates_over_the_adapter_through_the_seam() {
        let repo = tempfile::TempDir::new().expect("temp repo");
        let expectation = Expectation::parse(
            DRIVE_SPEC,
            &repo.path().join("feature.expect.md"),
            repo.path(),
        )
        .expect("parse drive spec");

        let agent = ScriptedAgent::with_config(
            vec![(
                DRIVER_GOAL_NEEDLE.to_string(),
                ScriptedReply::Text(r#"{"summary": "drove the total to 40"}"#.to_string()),
            )],
            ScriptedAgentConfig::default(),
        );
        let factory = scripted_factory(agent);

        let adapter = StubAdapter {
            state: SurfaceState::Json {
                body: serde_json::json!({ "total": 40 }),
            },
        };

        let verdict = run_drive_request(
            expectation,
            adapter,
            repo.path().to_path_buf(),
            PoolConfig::remote(1),
            factory,
        )
        .await
        .expect("the drive request must produce a re-validated verdict");

        assert_eq!(
            verdict.criteria.len(),
            1,
            "the one Then criterion was graded"
        );
        assert!(
            verdict.criteria[0].pass,
            "the criterion is graded over the adapter's observed total=40: {:?}",
            verdict.criteria
        );
        assert!(
            verdict.reliability.satisfied(),
            "the verdict is satisfied over the adapter-observed state"
        );
    }
}
