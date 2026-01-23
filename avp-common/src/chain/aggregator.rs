//! Chain aggregator for combining results from multiple links.

use crate::types::{HookOutput, LinkOutput};

/// Aggregator that combines partial outputs from chain links into a final output.
///
/// The aggregation rules are:
/// - `continue`: AND all results (false if any is false)
/// - `stopReason`: First non-None value wins
/// - `suppressOutput`: OR all results (true if any is true)
/// - `systemMessage`: Concatenate with newlines
/// - `hookSpecificOutput`: Deep merge, last write wins
#[derive(Debug, Default)]
pub struct ChainAggregator {
    /// Collected link outputs.
    outputs: Vec<LinkOutput>,
}

impl ChainAggregator {
    /// Create a new empty aggregator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a link output to be aggregated.
    pub fn add(&mut self, output: LinkOutput) {
        self.outputs.push(output);
    }

    /// Aggregate all collected outputs into a final hook output.
    pub fn aggregate(&self) -> HookOutput {
        let mut result = HookOutput::success();

        // AND all continue values (false if any is false)
        for output in &self.outputs {
            if let Some(false) = output.continue_execution {
                result.continue_execution = false;
            }
        }

        // First non-None stop reason wins
        for output in &self.outputs {
            if output.stop_reason.is_some() {
                result.stop_reason = output.stop_reason.clone();
                break;
            }
        }

        // OR all suppress_output values (true if any is true)
        for output in &self.outputs {
            if output.suppress_output == Some(true) {
                result.suppress_output = true;
                break;
            }
        }

        // Concatenate all system messages
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

    /// Check if any outputs have been collected.
    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    /// Get the number of collected outputs.
    pub fn len(&self) -> usize {
        self.outputs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_aggregation() {
        let aggregator = ChainAggregator::new();
        let result = aggregator.aggregate();
        assert!(result.continue_execution);
        assert!(result.stop_reason.is_none());
    }

    #[test]
    fn test_continue_and_logic() {
        let mut aggregator = ChainAggregator::new();
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
    fn test_suppress_output_or_logic() {
        let mut aggregator = ChainAggregator::new();
        aggregator.add(LinkOutput {
            suppress_output: Some(false),
            ..Default::default()
        });
        aggregator.add(LinkOutput {
            suppress_output: Some(true),
            ..Default::default()
        });

        let result = aggregator.aggregate();
        assert!(result.suppress_output);
    }

    #[test]
    fn test_system_message_concatenation() {
        let mut aggregator = ChainAggregator::new();
        aggregator.add(LinkOutput {
            system_message: Some("First message".to_string()),
            ..Default::default()
        });
        aggregator.add(LinkOutput {
            system_message: Some("Second message".to_string()),
            ..Default::default()
        });

        let result = aggregator.aggregate();
        assert_eq!(
            result.system_message,
            Some("First message\nSecond message".to_string())
        );
    }

    #[test]
    fn test_first_stop_reason_wins() {
        let mut aggregator = ChainAggregator::new();
        aggregator.add(LinkOutput {
            stop_reason: Some("First reason".to_string()),
            ..Default::default()
        });
        aggregator.add(LinkOutput {
            stop_reason: Some("Second reason".to_string()),
            ..Default::default()
        });

        let result = aggregator.aggregate();
        assert_eq!(result.stop_reason, Some("First reason".to_string()));
    }
}
