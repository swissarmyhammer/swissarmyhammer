//! Expression operations for the JS engine
//!
//! Operations follow the swissarmyhammer-operations pattern:
//! - `set expression`: Evaluate JS expression and store result as named variable
//! - `get expression`: Retrieve a variable's value

pub mod get;
pub mod set;

pub use get::GetExpression;
pub use set::SetExpression;
