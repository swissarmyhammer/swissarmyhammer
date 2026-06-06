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
use swissarmyhammer_config::model::ModelConfig;

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
