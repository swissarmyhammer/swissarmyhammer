//! Core validation traits and interfaces

use super::errors::ValidationError;
use crate::types::Session;

/// Core validation trait that all validators implement
///
/// The session parameter provides universal context for all validation operations.
/// Every validation occurs within the scope of a session, providing access to:
/// - Message history for context-aware validation
/// - Tool availability for validation decisions  
/// - Session state and metadata for temporal validations
/// - MCP configuration that may affect validation rules
pub trait Validator<Target> {
    type Error;

    /// Validate a target within the context of a session
    ///
    /// # Arguments
    /// * `session` - The session context providing validation metadata
    /// * `target` - The object to validate
    ///
    /// # Returns
    /// Ok(()) if validation passes, Error if validation fails
    fn validate(&self, session: &Session, target: &Target) -> Result<(), Self::Error>;
}

/// Validation trait specifically for generation requests
///
/// This is a convenience trait that pre-specifies the error type for generation
/// request validation to ensure consistency across all generation validators.
pub trait ValidatesGenerationRequest<Target>: Validator<Target, Error = ValidationError> {}

/// Blanket implementation for any validator that validates with ValidationError
impl<T, Target> ValidatesGenerationRequest<Target> for T where
    T: Validator<Target, Error = ValidationError>
{
}

/// Validation trait for tool calls
///
/// Similar convenience trait for tool call validation
pub trait ValidatesToolCall<Target>: Validator<Target, Error = ValidationError> {}

/// Blanket implementation for tool call validators
impl<T, Target> ValidatesToolCall<Target> for T where T: Validator<Target, Error = ValidationError> {}

/// Trait for composite validators that combine multiple validators
pub trait CompositeValidator<Target> {
    type Error;

    /// Add a validator to this composite
    fn add_validator<V>(&mut self, validator: V)
    where
        V: Validator<Target, Error = Self::Error> + Send + Sync + 'static;

    /// Validate using all contained validators
    fn validate_all(&self, session: &Session, target: &Target) -> Result<(), Self::Error>;
}
