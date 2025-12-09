//! ID wrapper types for type-safe identifiers.
//!
//! This module provides strongly typed ID wrappers around ULID to prevent
//! mixing up different types of identifiers in the system.

use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Session identifier for tracking conversation sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Ulid);

impl SessionId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn from_ulid(ulid: Ulid) -> Self {
        Self(ulid)
    }

    pub fn as_ulid(&self) -> Ulid {
        self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Ulid::from_string(s)?))
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool call identifier for tracking tool invocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCallId(Ulid);

impl ToolCallId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn from_ulid(ulid: Ulid) -> Self {
        Self(ulid)
    }

    pub fn as_ulid(&self) -> Ulid {
        self.0
    }
}

impl std::fmt::Display for ToolCallId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ToolCallId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Ulid::from_string(s)?))
    }
}

impl Default for ToolCallId {
    fn default() -> Self {
        Self::new()
    }
}

/// Prompt identifier for tracking prompt templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PromptId(Ulid);

impl PromptId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn from_ulid(ulid: Ulid) -> Self {
        Self(ulid)
    }

    pub fn as_ulid(&self) -> Ulid {
        self.0
    }
}

impl std::fmt::Display for PromptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PromptId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Ulid::from_string(s)?))
    }
}

impl Default for PromptId {
    fn default() -> Self {
        Self::new()
    }
}
