//! Type-safe transition key representation
//!
//! This module provides a strongly-typed key for identifying workflow transitions,
//! avoiding string manipulation errors and providing consistent formatting.

use super::StateId;
use std::fmt;

/// A type-safe key representing a transition between two states
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransitionKey {
    /// The source state of the transition
    pub from: StateId,
    /// The destination state of the transition
    pub to: StateId,
}

impl TransitionKey {
    /// Creates a new transition key
    pub fn new(from: StateId, to: StateId) -> Self {
        Self { from, to }
    }

    /// Creates a transition key from state references
    pub fn from_refs(from: &StateId, to: &StateId) -> Self {
        Self {
            from: from.clone(),
            to: to.clone(),
        }
    }
}

impl fmt::Display for TransitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.from, self.to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_key_creation() {
        let from = StateId::new("start");
        let to = StateId::new("end");
        let key = TransitionKey::new(from.clone(), to.clone());

        assert_eq!(key.from, from);
        assert_eq!(key.to, to);
    }

    #[test]
    fn test_transition_key_display() {
        let key = TransitionKey::new(StateId::new("A"), StateId::new("B"));
        assert_eq!(key.to_string(), "A -> B");
    }

    #[test]
    fn test_transition_key_equality() {
        let key1 = TransitionKey::new(StateId::new("A"), StateId::new("B"));
        let key2 = TransitionKey::new(StateId::new("A"), StateId::new("B"));
        let key3 = TransitionKey::new(StateId::new("B"), StateId::new("A"));

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_transition_key_from_refs() {
        let from = StateId::new("start");
        let to = StateId::new("end");
        let key1 = TransitionKey::new(from.clone(), to.clone());
        let key2 = TransitionKey::from_refs(&from, &to);

        assert_eq!(key1, key2);
    }
}
