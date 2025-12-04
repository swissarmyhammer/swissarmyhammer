#[cfg(test)]
mod tests {
    use crate::executor::core::WorkflowExecutor;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_result_content_field_accessible_in_cel() {
        let mut executor = WorkflowExecutor::new();
        let mut context = HashMap::new();

        // Simulate what actions.rs does - store AgentResponse as JSON
        let agent_response = json!({
            "content": "YES",
            "metadata": null,
            "response_type": "Success"
        });

        context.insert("result".to_string(), agent_response);

        // This should work - accessing result.content
        let expression = "result.content.contains(\"YES\")";
        let result = executor.evaluate_condition(
            &crate::TransitionCondition {
                condition_type: crate::ConditionType::Custom,
                expression: Some(expression.to_string()),
            },
            &context,
        );

        assert!(result.is_ok(), "Failed to evaluate: {:?}", result.err());
        assert!(result.unwrap());
    }

    #[test]
    fn test_result_as_string_does_not_have_content_field() {
        let mut executor = WorkflowExecutor::new();
        let mut context = HashMap::new();

        // If result is just a string, it won't have .content
        context.insert("result".to_string(), json!("YES"));

        // This should fail - string doesn't have content field
        let expression = "result.content.contains(\"YES\")";
        let result = executor.evaluate_condition(
            &crate::TransitionCondition {
                condition_type: crate::ConditionType::Custom,
                expression: Some(expression.to_string()),
            },
            &context,
        );

        assert!(result.is_err(), "Should have failed");
    }
}
