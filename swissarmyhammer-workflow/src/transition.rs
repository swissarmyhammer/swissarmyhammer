//! Transition-related types for workflows

use crate::StateId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of transition conditions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionType {
    /// Always transition (unconditional)
    Always,
    /// Never transition
    Never,
    /// Transition on successful execution
    OnSuccess,
    /// Transition on failed execution
    OnFailure,
    /// Custom condition with expression
    Custom,
}

impl ConditionType {
    /// Convert to string for backward compatibility
    pub fn as_str(&self) -> &'static str {
        match self {
            ConditionType::Always => "always",
            ConditionType::Never => "never",
            ConditionType::OnSuccess => "on_success",
            ConditionType::OnFailure => "on_failure",
            ConditionType::Custom => "custom",
        }
    }
}

/// Condition for a state transition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionCondition {
    /// Type of condition
    pub condition_type: ConditionType,
    /// Optional expression for custom conditions
    pub expression: Option<String>,
}

/// Represents a transition between states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    /// Source state ID
    pub from_state: StateId,
    /// Target state ID
    pub to_state: StateId,
    /// Condition that must be met for transition
    pub condition: TransitionCondition,
    /// Optional action to perform during transition
    pub action: Option<String>,
    /// Metadata for debugging and monitoring
    pub metadata: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_creation() {
        let transition = Transition {
            from_state: StateId::new("start"),
            to_state: StateId::new("end"),
            condition: TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        };

        assert_eq!(transition.from_state.as_str(), "start");
        assert_eq!(transition.to_state.as_str(), "end");
        assert_eq!(transition.condition.condition_type, ConditionType::Always);
    }
}
