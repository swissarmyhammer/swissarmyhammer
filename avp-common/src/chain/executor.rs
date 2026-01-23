//! Chain orchestrator that executes starters and links.

use std::marker::PhantomData;

use crate::error::ChainError;
use crate::types::HookOutput;

use super::aggregator::ChainAggregator;
use super::context::ChainContext;
use super::link::{ChainLink, ChainResult, HookInputType};
use super::starters::{ChainStarter, StarterResult, SuccessStarter};

/// A chain of processing links with a starter and aggregator.
///
/// The chain is generic over the input type `I`, ensuring type safety
/// throughout the chain processing.
///
/// The chain executes in this order:
/// 1. The starter runs to determine if processing should continue
/// 2. Each link processes the input in order
/// 3. The aggregator combines all link outputs into a final output
pub struct Chain<I: HookInputType> {
    /// The starter that determines initial behavior.
    starter: Box<dyn ChainStarter<I>>,

    /// Processing links in order of execution.
    links: Vec<Box<dyn ChainLink<I>>>,

    /// Aggregator for combining link outputs.
    aggregator: ChainAggregator,

    _phantom: PhantomData<I>,
}

impl<I: HookInputType> Chain<I> {
    /// Create a new chain with the given starter.
    pub fn new<S: ChainStarter<I> + 'static>(starter: S) -> Self {
        Self {
            starter: Box::new(starter),
            links: Vec::new(),
            aggregator: ChainAggregator::new(),
            _phantom: PhantomData,
        }
    }

    /// Create a new chain with a success starter.
    pub fn success() -> Self {
        Self::new(SuccessStarter::new())
    }

    /// Add a link to the chain.
    pub fn add_link<L: ChainLink<I> + 'static>(mut self, link: L) -> Self {
        self.links.push(Box::new(link));
        self
    }

    /// Execute the chain with the given input.
    ///
    /// Returns the final output and exit code.
    pub fn execute(&mut self, input: &I) -> Result<(HookOutput, i32), ChainError> {
        let mut ctx = ChainContext::new();

        // Execute starter
        match self.starter.start(input, &mut ctx)? {
            StarterResult::Continue => {}
            StarterResult::Stop(output) => {
                return Ok((output, ctx.exit_code()));
            }
        }

        // Execute each link
        for link in &self.links {
            match link.process(input, &mut ctx) {
                ChainResult::Continue(output) => {
                    if let Some(o) = output {
                        self.aggregator.add(o);
                    }
                }
                ChainResult::Stop(output) => {
                    self.aggregator.add(output);
                    break;
                }
                ChainResult::Error(e) => {
                    return Err(e);
                }
            }
        }

        // Aggregate results
        let output = self.aggregator.aggregate();
        let exit_code = if output.continue_execution {
            ctx.exit_code()
        } else {
            2 // Blocking error
        };

        Ok((output, exit_code))
    }

    /// Get the number of links in the chain.
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Check if the chain has no links.
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Get the names of all links in the chain.
    pub fn link_names(&self) -> Vec<&'static str> {
        self.links.iter().map(|l| l.name()).collect()
    }
}

impl<I: HookInputType> Default for Chain<I> {
    fn default() -> Self {
        Self::success()
    }
}

/// Builder for constructing chains fluently.
pub struct ChainBuilder<I: HookInputType> {
    starter: Option<Box<dyn ChainStarter<I>>>,
    links: Vec<Box<dyn ChainLink<I>>>,
    _phantom: PhantomData<I>,
}

impl<I: HookInputType> ChainBuilder<I> {
    /// Create a new chain builder.
    pub fn new() -> Self {
        Self {
            starter: None,
            links: Vec::new(),
            _phantom: PhantomData,
        }
    }

    /// Set the chain starter.
    pub fn with_starter<S: ChainStarter<I> + 'static>(mut self, starter: S) -> Self {
        self.starter = Some(Box::new(starter));
        self
    }

    /// Add a link to the chain.
    pub fn add_link<L: ChainLink<I> + 'static>(mut self, link: L) -> Self {
        self.links.push(Box::new(link));
        self
    }

    /// Build the chain.
    pub fn build(self) -> Chain<I> {
        let starter = self
            .starter
            .unwrap_or_else(|| Box::new(SuccessStarter::new()));
        Chain {
            starter,
            links: self.links,
            aggregator: ChainAggregator::new(),
            _phantom: PhantomData,
        }
    }
}

impl<I: HookInputType> Default for ChainBuilder<I> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::link::PassThroughLink;
    use crate::chain::starters::BlockingErrorStarter;
    use crate::types::{LinkOutput, PreToolUseInput};

    fn make_input() -> PreToolUseInput {
        serde_json::from_value(serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .unwrap()
    }

    #[test]
    fn test_empty_chain() {
        let mut chain: Chain<PreToolUseInput> = Chain::success();
        let input = make_input();

        let (output, exit_code) = chain.execute(&input).unwrap();
        assert!(output.continue_execution);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_chain_with_pass_through() {
        let mut chain: Chain<PreToolUseInput> = Chain::success().add_link(PassThroughLink::new());
        let input = make_input();

        let (output, exit_code) = chain.execute(&input).unwrap();
        assert!(output.continue_execution);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_blocking_error_starter() {
        let mut chain: Chain<PreToolUseInput> = Chain::new(BlockingErrorStarter::new("Blocked"));
        let input = make_input();

        let (output, exit_code) = chain.execute(&input).unwrap();
        assert!(!output.continue_execution);
        assert_eq!(output.stop_reason, Some("Blocked".to_string()));
        assert_eq!(exit_code, 2);
    }

    #[test]
    fn test_chain_builder() {
        let chain: Chain<PreToolUseInput> = ChainBuilder::new()
            .with_starter(SuccessStarter::new())
            .add_link(PassThroughLink::new())
            .build();

        assert_eq!(chain.len(), 1);
        assert_eq!(chain.link_names(), vec!["PassThrough"]);
    }

    struct BlockingLink;

    impl ChainLink<PreToolUseInput> for BlockingLink {
        fn process(&self, _input: &PreToolUseInput, _ctx: &mut ChainContext) -> ChainResult {
            ChainResult::stop(LinkOutput::block("Link blocked"))
        }

        fn name(&self) -> &'static str {
            "BlockingLink"
        }

        fn can_short_circuit(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_chain_short_circuit() {
        let mut chain: Chain<PreToolUseInput> = Chain::success()
            .add_link(BlockingLink)
            .add_link(PassThroughLink::new());
        let input = make_input();

        let (output, exit_code) = chain.execute(&input).unwrap();
        assert!(!output.continue_execution);
        assert_eq!(exit_code, 2);
    }
}
