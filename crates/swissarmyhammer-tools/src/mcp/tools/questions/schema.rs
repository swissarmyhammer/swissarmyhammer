//! Schema generation for the unified question tool using the Operation pattern

use serde_json::{json, Value};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

/// Generate the MCP schema for the question tool from operation metadata
pub fn generate_question_mcp_schema(operations: &[&dyn Operation]) -> Value {
    let config = SchemaConfig::new(
        "Question operations for interactive user elicitation and Q&A history. Use 'ask question' to prompt the user, 'summarize questions' to retrieve saved Q&A pairs.",
    )
    .with_examples(generate_question_examples());

    generate_mcp_schema(operations, config)
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
