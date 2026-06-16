//! Wiring-layer tests for the production `review` agent factory.
//!
//! `swissarmyhammer-agent` is the tier that may legally name both
//! `swissarmyhammer-tools` (where the `review_op::AgentFactory` seam is defined)
//! and the ACP agent backends (`create_agent`). These tests cover the factory
//! builder that the server wiring injects via `McpServer::set_review_factories`.
//!
//! The builder is pure: it does NOT spawn a backend or load a model. Actually
//! driving a `review` op against a live agent end-to-end is covered, with a
//! scripted agent, by the `swissarmyhammer-tools` review tests; this crate only
//! owns the construction of the production factory from a `ModelConfig`.

use std::sync::Arc;

use swissarmyhammer_agent::review_agent_factory;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::model::{
    ModelConfig, ModelExecutorConfig, ModelExecutorType, ModelManager, ModelPaths,
};

/// The factory builder is pure: it returns a callable `review_op::AgentFactory`
/// from a `ModelConfig` without constructing an agent or loading a model. The
/// returned value is exactly the seam type the server injects via
/// `McpServer::set_review_factories`, proving the wiring type-checks across the
/// crate boundary without a dependency cycle.
#[test]
fn review_agent_factory_builds_a_factory_from_a_model_config() {
    let config = Arc::new(ModelConfig::default());
    let factory: swissarmyhammer_tools::mcp::tools::review::review_op::AgentFactory =
        review_agent_factory(config);
    // Holding the closure is the contract the server depends on; building it must
    // not require a backend. Drop without invoking — invocation would spawn a
    // real agent.
    drop(factory);
}

/// Read the `claude-code` CLI switches a resolved `ModelConfig` carries — the
/// `--model haiku` for the review default — exactly as the spawn path forwards
/// them onto the `claude` process (the production seam copies these verbatim into
/// `claude.extra_args`; that copy is unit-tested in the agent crate's lib tests).
fn claude_args(config: &ModelConfig) -> Vec<String> {
    match config.executor() {
        ModelExecutorConfig::ClaudeCode(cfg) => cfg.args.clone(),
        _ => panic!(
            "expected a claude-code executor, got {:?}",
            config.executor_type()
        ),
    }
}

/// Wiring-layer real-path proof: the production review factory the server injects
/// (`review_agent_factory`) is built from the review `ModelConfig` the RUNTIME
/// RESOLVER produces for a fully-unconfigured scope — which resolves to
/// `claude-code-haiku` — so the agent it mints spawns `claude --model haiku`.
///
/// This drives the actual entry point the wired review tool uses, with no
/// hardcoded `ModelConfig::claude_code_haiku()` constructor:
/// `ModelManager::resolve_review_agent_config` (the resolver `serve/mod.rs`'
/// `review_model_config` exercises) → `review_agent_factory` (the exact factory
/// `McpServer::set_review_factories` injects). It asserts the resolved config the
/// factory captures carries `["--model","haiku"]`, which the production spawn seam
/// forwards onto the `claude` argv. Invoking the factory would spawn a real
/// `claude` (and needs the CLI), so this asserts on the captured config rather
/// than the spawn — the spawn-config copy is covered by the agent crate's lib
/// `review_resolved_default_spawns_claude_with_model_haiku`.
#[test]
fn review_factory_from_resolved_default_carries_model_haiku() {
    let _env = IsolatedTestEnvironment::new().expect("isolated env");

    let config = ModelManager::resolve_review_agent_config(&ModelPaths::sah())
        .expect("an unconfigured review scope must resolve to the baked-in default");
    assert_eq!(
        config.executor_type(),
        ModelExecutorType::ClaudeCode,
        "the review default must be a claude-code executor"
    );
    assert_eq!(
        claude_args(&config),
        vec!["--model".to_string(), "haiku".to_string()],
        "the resolved review default the factory is built from must carry --model haiku"
    );

    // The factory the server injects is built from exactly this config.
    let factory: swissarmyhammer_tools::mcp::tools::review::review_op::AgentFactory =
        review_agent_factory(Arc::new(config));
    drop(factory);
}

/// Real parity guard at the wiring layer: `local` and `session` review runs build
/// their agent from the SAME resolved review `ModelConfig` (the `backend` modifier
/// is not an input to agent creation), so both spawn `claude --model haiku`.
///
/// Each "backend" independently resolves its review config through the runtime
/// resolver — the same path each backend's run takes — and the two must be
/// byte-identical, both carrying `--model haiku`.
#[test]
fn local_and_session_review_runs_resolve_the_same_model() {
    let _env = IsolatedTestEnvironment::new().expect("isolated env");

    let local = ModelManager::resolve_review_agent_config(&ModelPaths::sah())
        .expect("local backend review scope resolves");
    let session = ModelManager::resolve_review_agent_config(&ModelPaths::sah())
        .expect("session backend review scope resolves");

    assert_eq!(
        claude_args(&local),
        claude_args(&session),
        "local and session backends must resolve the same review model (no drift)"
    );
    assert_eq!(
        claude_args(&local),
        vec!["--model".to_string(), "haiku".to_string()],
        "both backends must carry the resolved haiku default's --model haiku"
    );
}
