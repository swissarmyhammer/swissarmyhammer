//! Schema generation for the unified question tool using the Operation pattern

use serde_json::{json, Value};
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, Operation, SchemaConfig,
};

/// Build the shared schema config (description + examples) for the question
/// tool, so the wire and full generators stay in lockstep.
fn question_schema_config() -> SchemaConfig {
    SchemaConfig::new(
        "Question operations for interactive user elicitation and Q&A history. Use 'ask question' to prompt the user, 'summarize questions' to retrieve saved Q&A pairs.",
    )
    .with_examples(generate_question_examples())
}

/// Generate the slim WIRE MCP schema for the question tool.
///
/// Model-facing surface: carries only the op enum and per-op required-field
/// signatures, dropping the heavy CLI-facing keys. In-process consumers needing
/// the full per-op detail must call [`generate_question_mcp_schema_full`].
pub fn generate_question_mcp_schema(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_wire(operations, question_schema_config())
}

/// Generate the FULL CLI-facing MCP schema for the question tool.
pub fn generate_question_mcp_schema_full(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_full(operations, question_schema_config())
}

fn generate_question_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "Ask the user a question",
            "value": {"op": "ask question", "question": "What is your preferred deployment target?"}
        }),
        json!({
            "description": "Get all Q&A history",
            "value": {"op": "summarize questions"}
        }),
        json!({
            "description": "Get recent 5 Q&A pairs",
            "value": {"op": "summarize questions", "limit": 5}
        }),
    ]
}
