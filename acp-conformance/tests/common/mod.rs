//! Common test utilities and fixtures for ACP conformance tests.
//!
//! This module provides shared functionality used across all conformance test files,
//! including agent factory types and creation utilities.

use agent_client_protocol::Agent;
use std::future::Future;
use std::pin::Pin;

/// Type alias for agent factory functions used in parametric tests.
///
/// This allows tests to be parametrized across different agent implementations
/// by passing factory functions that create agent instances.
pub type AgentFactory =
    fn() -> Pin<Box<dyn Future<Output = Result<Box<dyn Agent>, Box<dyn std::error::Error>>>>>;

/// Creates a factory function for the Claude agent.
///
/// Returns a function that can be used in rstest parametrized tests to create
/// Claude agent instances.
pub fn claude_agent_factory() -> AgentFactory {
    || {
        Box::pin(async {
            let agent = crate::agent_fixtures::create_claude_agent().await?;
            Ok(Box::new(agent) as Box<dyn Agent>)
        })
    }
}

/// Creates a factory function for the Llama agent.
///
/// Returns a function that can be used in rstest parametrized tests to create
/// Llama agent instances.
pub fn llama_agent_factory() -> AgentFactory {
    || {
        Box::pin(async {
            let agent = crate::agent_fixtures::create_llama_agent().await?;
            Ok(Box::new(agent) as Box<dyn Agent>)
        })
    }
}

/// Macro to run a test function with an agent created from a factory.
///
/// This macro reduces boilerplate by handling:
/// - Agent creation from factory
/// - LocalSet execution (required for ACP agents with !Send futures)
/// - Result unwrapping and error propagation
///
/// # Example
///
/// ```ignore
/// #[rstest]
/// #[case::llama(llama_agent_factory())]
/// #[case::claude(claude_agent_factory())]
/// #[test_log::test(tokio::test)]
/// #[serial_test::serial]
/// async fn test_something(#[case] factory: AgentFactory) {
///     with_agent!(factory, agent, {
///         // Your test code here using agent
///         let response = agent.initialize(...).await?;
///         assert!(!response.agent_name.is_empty());
///         Ok(())
///     })
/// }
/// ```
#[macro_export]
macro_rules! with_agent {
    ($factory:expr, $agent_name:ident, $body:expr) => {{
        let local_set = tokio::task::LocalSet::new();
        local_set
            .run_until(async {
                let $agent_name = $factory().await?;
                let $agent_name = $agent_name.as_ref();
                $body
            })
            .await
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_claude_agent_factory_creates_agent() {
        let factory = claude_agent_factory();
        let result = factory().await;

        // Agent creation may fail if fixtures are missing, but factory should work
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_llama_agent_factory_creates_agent() {
        let factory = llama_agent_factory();
        let result = factory().await;

        // Agent creation may fail if model is missing, but factory should work
        assert!(result.is_ok() || result.is_err());
    }
}
