//! Agent-agnostic chain output types.
//!
//! These types represent what chains produce internally, independent of
//! any specific agent platform. Agent strategies transform these into
//! their platform-specific formats.

use crate::types::HookType;

/// Internal output from a chain link.
///
/// This is agent-agnostic - it contains the information needed for
/// any agent strategy to produce its specific output format.
#[derive(Debug, Clone, Default)]
pub struct LinkOutput {
    /// Whether to continue (None means no preference).
    pub continue_execution: Option<bool>,

    /// Reason for stopping.
    pub stop_reason: Option<String>,

    /// Whether to suppress output.
    pub suppress_output: Option<bool>,

    /// System message to add.
    pub system_message: Option<String>,

    /// Validator that blocked (if any).
    pub validator_block: Option<ValidatorBlockInfo>,
}

/// Information about a validator that blocked.
#[derive(Debug, Clone)]
pub struct ValidatorBlockInfo {
    /// Name of the validator.
    pub validator_name: String,
    /// Message from the validator.
    pub message: String,
    /// Hook type this block occurred on.
    pub hook_type: HookType,
}

impl LinkOutput {
    /// Create an empty link output.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create a link output that allows continuation.
    pub fn allow() -> Self {
        Self {
            continue_execution: Some(true),
            ..Default::default()
        }
    }

    /// Create a link output that blocks execution.
    pub fn block(reason: impl Into<String>) -> Self {
        Self {
            continue_execution: Some(false),
            stop_reason: Some(reason.into()),
            ..Default::default()
        }
    }

    /// Create a link output from a validator block.
    pub fn from_validator_block(
        validator_name: impl Into<String>,
        message: impl Into<String>,
        hook_type: HookType,
    ) -> Self {
        let message = message.into();
        Self {
            continue_execution: Some(false),
            stop_reason: Some(message.clone()),
            validator_block: Some(ValidatorBlockInfo {
                validator_name: validator_name.into(),
                message,
                hook_type,
            }),
            ..Default::default()
        }
    }

    /// Add a system message.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.system_message = Some(message.into());
        self
    }
}

/// Aggregated output from chain execution.
///
/// This is the agent-agnostic result that strategies transform
/// into their platform-specific format.
#[derive(Debug, Clone, Default)]
pub struct ChainOutput {
    /// Whether to continue execution.
    pub continue_execution: bool,

    /// Reason for stopping.
    pub stop_reason: Option<String>,

    /// Whether to suppress output.
    pub suppress_output: bool,

    /// System message to add.
    pub system_message: Option<String>,

    /// Validator that blocked (if any).
    pub validator_block: Option<ValidatorBlockInfo>,
}

impl ChainOutput {
    /// Create a success output.
    pub fn success() -> Self {
        Self {
            continue_execution: true,
            ..Default::default()
        }
    }

    /// Check if execution was blocked.
    pub fn is_blocked(&self) -> bool {
        !self.continue_execution || self.validator_block.is_some()
    }

    /// Get the blocking validator info if present.
    pub fn blocking_validator(&self) -> Option<&ValidatorBlockInfo> {
        self.validator_block.as_ref()
    }
}

/// Aggregator that combines link outputs into a final chain output.
#[derive(Debug, Default)]
pub struct ChainOutputAggregator {
    outputs: Vec<LinkOutput>,
}

impl ChainOutputAggregator {
    /// Create a new empty aggregator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a link output.
    pub fn add(&mut self, output: LinkOutput) {
        self.outputs.push(output);
    }

    /// Helper: find first non-None value.
    fn first_non_none<T: Clone, F>(&self, extractor: F) -> Option<T>
    where
        F: Fn(&LinkOutput) -> &Option<T>,
    {
        self.outputs.iter().find_map(|o| extractor(o).clone())
    }

    /// Aggregate all outputs into a final chain output.
    pub fn aggregate(&self) -> ChainOutput {
        let mut result = ChainOutput::success();

        // AND all continue values (false if any is false)
        if self
            .outputs
            .iter()
            .any(|o| o.continue_execution == Some(false))
        {
            result.continue_execution = false;
        }

        // OR suppress_output
        if self.outputs.iter().any(|o| o.suppress_output == Some(true)) {
            result.suppress_output = true;
        }

        // First non-None wins
        result.stop_reason = self.first_non_none(|o| &o.stop_reason);
        result.validator_block = self.first_non_none(|o| &o.validator_block);

        // Concatenate system messages
        let messages: Vec<&str> = self
            .outputs
            .iter()
            .filter_map(|o| o.system_message.as_deref())
            .collect();
        if !messages.is_empty() {
            result.system_message = Some(messages.join("\n"));
        }

        result
    }

    /// Clear all collected outputs.
    pub fn clear(&mut self) {
        self.outputs.clear();
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    /// Get count.
    pub fn len(&self) -> usize {
        self.outputs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_output_empty() {
        let output = LinkOutput::empty();
        assert!(output.continue_execution.is_none());
        assert!(output.stop_reason.is_none());
    }

    #[test]
    fn test_link_output_allow() {
        let output = LinkOutput::allow();
        assert_eq!(output.continue_execution, Some(true));
    }

    #[test]
    fn test_link_output_block() {
        let output = LinkOutput::block("Access denied");
        assert_eq!(output.continue_execution, Some(false));
        assert_eq!(output.stop_reason, Some("Access denied".to_string()));
    }

    #[test]
    fn test_link_output_from_validator_block() {
        let output =
            LinkOutput::from_validator_block("no-secrets", "Found API key", HookType::PostToolUse);
        assert_eq!(output.continue_execution, Some(false));
        let block = output.validator_block.unwrap();
        assert_eq!(block.validator_name, "no-secrets");
        assert_eq!(block.message, "Found API key");
        assert_eq!(block.hook_type, HookType::PostToolUse);
    }

    #[test]
    fn test_chain_output_success() {
        let output = ChainOutput::success();
        assert!(output.continue_execution);
        assert!(!output.is_blocked());
    }

    #[test]
    fn test_aggregator_empty() {
        let aggregator = ChainOutputAggregator::new();
        let result = aggregator.aggregate();
        assert!(result.continue_execution);
        assert!(result.stop_reason.is_none());
    }

    #[test]
    fn test_aggregator_continue_and_logic() {
        let mut aggregator = ChainOutputAggregator::new();
        aggregator.add(LinkOutput {
            continue_execution: Some(true),
            ..Default::default()
        });
        aggregator.add(LinkOutput {
            continue_execution: Some(false),
            stop_reason: Some("Stopped".to_string()),
            ..Default::default()
        });

        let result = aggregator.aggregate();
        assert!(!result.continue_execution);
        assert_eq!(result.stop_reason, Some("Stopped".to_string()));
    }

    #[test]
    fn test_aggregator_validator_block_preserved() {
        let mut aggregator = ChainOutputAggregator::new();
        aggregator.add(LinkOutput::from_validator_block(
            "no-secrets",
            "Found secret",
            HookType::PostToolUse,
        ));

        let result = aggregator.aggregate();
        assert!(!result.continue_execution);
        assert!(result.validator_block.is_some());
        let block = result.validator_block.unwrap();
        assert_eq!(block.validator_name, "no-secrets");
    }
}
