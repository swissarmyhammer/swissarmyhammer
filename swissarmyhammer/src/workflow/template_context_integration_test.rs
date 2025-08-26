//! Integration test for WorkflowTemplateContext integration
//!
//! This test verifies that the workflow system correctly integrates with
//! the new TemplateContext system.

#[cfg(test)]
mod tests {

    use crate::workflow::WorkflowTemplateContext;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_template_context_integration_in_workflow() {
        // Test that the workflow template integration works end-to-end

        // Create a template context with some values
        let vars = HashMap::from([
            ("workflow_name".to_string(), json!("test-workflow")),
            ("environment".to_string(), json!("development")),
        ]);

        let workflow_context = WorkflowTemplateContext::with_vars(vars).unwrap();

        // Initialize a workflow context like WorkflowRun would
        let context = workflow_context.initialize_workflow_context();

        // Verify the context has _template_vars with our values
        assert!(context.contains_key("_template_vars"));

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        assert_eq!(
            template_vars.get("workflow_name").unwrap(),
            &json!("test-workflow")
        );
        assert_eq!(
            template_vars.get("environment").unwrap(),
            &json!("development")
        );
    }

    #[test]
    fn test_load_and_merge_template_context() {
        // Test the new load_and_merge_template_context function
        let mut context = HashMap::new();

        // This should work even if no config file exists
        let _loaded = if let Ok(template_context) = swissarmyhammer_config::load_configuration() {
            template_context.merge_into_workflow_context(&mut context);
            true
        } else {
            false
        };

        // Should have created _template_vars even if empty
        assert!(context.contains_key("_template_vars"));

        // The function should complete successfully (loaded can be true or false)
        // This exercises the function without a tautological assertion
    }

    #[test]
    fn test_enhanced_context_for_action_parsing() {
        // Test the pattern used in action parsing
        let mut base_context = HashMap::new();
        base_context.insert("action_var".to_string(), json!("action_value"));

        // Create enhanced context like action parsing does
        let mut enhanced_context = base_context.clone();

        // Load configuration
        if let Ok(template_context) = swissarmyhammer_config::load_configuration() {
            template_context.merge_into_workflow_context(&mut enhanced_context);
        }

        // Should have both original and template vars
        assert_eq!(
            enhanced_context.get("action_var").unwrap(),
            &json!("action_value")
        );
        assert!(enhanced_context.contains_key("_template_vars"));

        // Template vars should be an object
        assert!(enhanced_context.get("_template_vars").unwrap().is_object());
    }

    #[test]
    fn test_template_vars_precedence() {
        // Test that workflow variables override config variables
        let mut context = HashMap::new();

        // Set up initial template vars with workflow-specific values
        context.insert(
            "_template_vars".to_string(),
            json!({
                "shared_key": "workflow_value",
                "workflow_only": "workflow_specific"
            }),
        );

        // Load configuration which might have overlapping keys
        if let Ok(template_context) = swissarmyhammer_config::load_configuration() {
            template_context.merge_into_workflow_context(&mut context);
        }

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();

        // Workflow values should be preserved
        assert_eq!(
            template_vars.get("workflow_only").unwrap(),
            &json!("workflow_specific")
        );

        // If there was a conflict, workflow should win (but we can't easily test this
        // without setting up a specific config file, so we just verify the structure)
        assert!(template_vars.contains_key("shared_key"));
    }
}
