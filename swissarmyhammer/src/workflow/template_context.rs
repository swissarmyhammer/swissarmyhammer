//! Workflow template context integration
//!
//! This module provides integration between the new TemplateContext system and
//! the existing workflow HashMap-based context system.

use serde_json::Value;
use std::collections::HashMap;
use swissarmyhammer_config::{ConfigurationResult, TemplateContext};
use crate::workflow::action_parser::ActionParser;

/// Workflow-specific template context that bridges between TemplateContext and HashMap
///
/// This type manages the integration between the new TemplateContext configuration
/// system and workflow variables. It ensures proper precedence rules where workflow
/// variables override template configuration values.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowTemplateContext {
    /// The underlying template context with configuration values
    template_context: TemplateContext,
    /// Workflow variables that override template configuration values
    workflow_vars: HashMap<String, Value>,
}

impl WorkflowTemplateContext {
    /// Create a new WorkflowTemplateContext from configuration
    pub fn load() -> ConfigurationResult<Self> {
        let template_context = TemplateContext::load()?;
        Ok(Self { 
            template_context,
            workflow_vars: HashMap::new(),
        })
    }

    /// Create a new WorkflowTemplateContext with CLI environment variables loaded
    pub fn load_for_cli() -> ConfigurationResult<Self> {
        let template_context = TemplateContext::load_for_cli()?;
        Ok(Self { 
            template_context,
            workflow_vars: HashMap::new(),
        })
    }

    /// Create a WorkflowTemplateContext with additional template variables
    pub fn with_vars(vars: HashMap<String, Value>) -> ConfigurationResult<Self> {
        let template_context = TemplateContext::with_template_vars(vars)?;
        Ok(Self { 
            template_context,
            workflow_vars: HashMap::new(),
        })
    }

    /// Initialize a workflow context HashMap with template variables
    ///
    /// This method sets up a fresh workflow context with configuration values
    /// populated in the _template_vars object. Workflow state variables can
    /// then be added on top with higher precedence.
    pub fn initialize_workflow_context(&self) -> HashMap<String, Value> {
        let mut context = HashMap::new();
        self.template_context
            .merge_into_workflow_context(&mut context);
        context
    }

    /// Update workflow context with latest template configuration
    ///
    /// This method preserves existing workflow variables while ensuring
    /// configuration values are available. Workflow variables in _template_vars
    /// maintain higher precedence than configuration values.
    pub fn update_workflow_context(&self, context: &mut HashMap<String, Value>) {
        self.template_context.merge_into_workflow_context(context);
    }

    /// Get a value from the template context
    pub fn get(&self, key: &str) -> Option<&Value> {
        // Check workflow variables first (higher precedence)
        if let Some(value) = self.workflow_vars.get(key) {
            return Some(value);
        }
        
        // Then check template context
        self.template_context.get(key)
    }

    /// Get the underlying template context for advanced operations
    pub fn template_context(&self) -> &TemplateContext {
        &self.template_context
    }

    /// Set a workflow variable
    pub fn set_workflow_var(&mut self, key: String, value: Value) {
        self.workflow_vars.insert(key, value);
    }

    /// Set multiple workflow variables
    pub fn set_workflow_vars(&mut self, vars: HashMap<String, Value>) {
        for (key, value) in vars {
            self.workflow_vars.insert(key, value);
        }
    }

    /// Get a workflow variable
    pub fn get_workflow_var(&self, key: &str) -> Option<&Value> {
        self.workflow_vars.get(key)
    }

    /// Remove a workflow variable
    pub fn remove_workflow_var(&mut self, key: &str) -> Option<Value> {
        self.workflow_vars.remove(key)
    }

    /// Clear all workflow variables
    pub fn clear_workflow_vars(&mut self) {
        self.workflow_vars.clear();
    }

    /// Convert template context to liquid context for template rendering
    /// This includes both template configuration variables and workflow variables,
    /// with workflow variables taking precedence over template configuration.
    pub fn to_liquid_context(&self) -> liquid::Object {
        let mut liquid_vars = self.template_context.to_liquid_context();

        // Add workflow variables, giving them higher precedence
        for (key, value) in &self.workflow_vars {
            // Skip internal keys that shouldn't be exposed to templates
            if key.starts_with('_') {
                continue;
            }
            liquid_vars.insert(
                key.clone().into(),
                liquid::model::to_value(value).unwrap_or(liquid::model::Value::Nil),
            );
        }

        liquid_vars
    }



    /// Render a template string with both liquid and fallback variable substitution
    ///
    /// This method provides comprehensive template rendering by:
    /// 1. Using liquid templating for {{variable}} syntax
    /// 2. Using fallback substitution for ${variable} syntax
    /// 3. Using stored workflow variables with higher precedence over configuration
    /// 4. Internal variables (starting with _) are filtered out and left unrendered
    pub fn render_template(&self, input: &str) -> String {
        // Get liquid context with workflow variables merged (excludes internal vars)
        let liquid_vars = self.to_liquid_context();

        // For liquid template parsing, we need to handle internal variables specially
        // since liquid fails if it encounters undefined variables
        let mut template_for_liquid = input.to_string();
        
        // Find all internal variables in the template and temporarily replace them
        let internal_var_regex = regex::Regex::new(r"\{\{(_\w+)\}\}").unwrap();
        let internal_vars: Vec<String> = internal_var_regex
            .captures_iter(input)
            .map(|cap| cap[1].to_string())
            .collect();
        
        // Replace internal variables with unique placeholders
        let mut placeholder_map = HashMap::new();
        for (i, var) in internal_vars.iter().enumerate() {
            let placeholder = format!("__INTERNAL_VAR_{}__", i);
            let pattern = format!("{{{{{}}}}}", var);
            template_for_liquid = template_for_liquid.replace(&pattern, &placeholder);
            placeholder_map.insert(placeholder, format!("{{{{{}}}}}", var));
        }

        // Try liquid template rendering on the modified template
        let liquid_rendered = match liquid::ParserBuilder::with_stdlib()
            .build()
            .and_then(|parser| parser.parse(&template_for_liquid))
        {
            Ok(template) => match template.render(&liquid_vars) {
                Ok(rendered) => rendered,
                Err(_) => template_for_liquid.clone(),
            },
            Err(_) => template_for_liquid.clone(),
        };

        // Restore the internal variable placeholders back to their original form
        let mut restored_template = liquid_rendered;
        for (placeholder, original) in placeholder_map {
            restored_template = restored_template.replace(&placeholder, &original);
        }

        // For fallback variable substitution, create a combined context
        // that includes both template variables and workflow variables
        let mut combined_context = HashMap::new();
        
        // Add template variables first
        for (key, value) in self.template_context.variables() {
            combined_context.insert(key.clone(), value.clone());
        }
        
        // Add workflow variables (higher precedence)
        for (key, value) in &self.workflow_vars {
            combined_context.insert(key.clone(), value.clone());
        }

        // Apply fallback variable substitution for any remaining ${variable} syntax
        let parser = ActionParser::new().expect("Failed to create ActionParser");
        parser
            .substitute_variables_safe(&restored_template, &combined_context)
            .unwrap_or(restored_template)
    }

    /// Merge workflow HashMap context back into template context
    ///
    /// This method allows workflow state changes to be reflected in the
    /// template context. Only _template_vars are synchronized back.
    pub fn sync_from_workflow_context(&mut self, context: &HashMap<String, Value>) {
        if let Some(Value::Object(template_vars)) = context.get("_template_vars") {
            // Only sync non-configuration keys back to avoid overriding config
            for (key, _value) in template_vars {
                // Skip keys that are likely from configuration
                // In practice, workflow-specific keys would have specific prefixes or naming
                if self.template_context.get(key).is_none() {
                    // This is a workflow-generated variable, preserve it
                    // For now, we'll be conservative and not sync back to avoid complications
                    tracing::trace!(
                        "Workflow variable '{}' not synced back to template context",
                        key
                    );
                }
            }
        }
    }

    /// Convert WorkflowTemplateContext to TemplateContext for prompt rendering
    ///
    /// This method creates a new TemplateContext that includes both configuration
    /// variables and workflow variables, with workflow variables taking precedence.
    /// This is particularly useful for prompt actions that need to render templates
    /// using the full context.
    pub fn to_template_context(&self) -> ConfigurationResult<TemplateContext> {
        // Create a combined variables map
        let mut combined_vars = serde_json::Map::new();
        
        // Add template configuration variables first
        for (key, value) in self.template_context.variables() {
            combined_vars.insert(key.clone(), value.clone());
        }
        
        // Add workflow variables with higher precedence
        for (key, value) in &self.workflow_vars {
            combined_vars.insert(key.clone(), value.clone());
        }

        // Create new TemplateContext with combined variables
        TemplateContext::with_template_vars(combined_vars.into_iter().collect())
    }

    /// Convert to HashMap for backward compatibility with existing code
    ///
    /// This creates a HashMap containing workflow variables that can be used
    /// with existing action execution code. The returned HashMap includes
    /// both template configuration and workflow variables with proper precedence.
    pub fn to_workflow_hashmap(&self) -> HashMap<String, Value> {
        let mut context = self.initialize_workflow_context();
        
        // Flatten template variables from _template_vars to top level for substitution
        let template_vars_to_flatten = context.get("_template_vars")
            .and_then(|v| v.as_object())
            .cloned();
            
        if let Some(template_vars) = template_vars_to_flatten {
            for (key, value) in template_vars {
                // Don't overwrite if key already exists at top level
                if !context.contains_key(&key) {
                    context.insert(key, value);
                }
            }
        }
        
        // Include workflow variables (which include action results)
        // These take precedence over template variables
        for (key, value) in &self.workflow_vars {
            context.insert(key.clone(), value.clone());
        }
        context
    }

    /// Insert a workflow variable (HashMap-like interface)
    pub fn insert(&mut self, key: String, value: Value) {
        self.set_workflow_var(key, value);
    }

    /// Get a copy of workflow variables for action execution
    pub fn workflow_vars(&self) -> HashMap<String, Value> {
        self.workflow_vars.clone()
    }

    /// Remove a workflow variable (HashMap-like interface)
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.remove_workflow_var(key)
    }

    /// Iterate over workflow variables (for compensation states, etc.)
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.workflow_vars.iter()
    }

    /// Check if a key exists (HashMap-like interface)
    pub fn contains_key(&self, key: &str) -> bool {
        // Check workflow variables first (higher precedence)
        if self.workflow_vars.contains_key(key) {
            return true;
        }
        
        // Then check template context
        self.template_context.get(key).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_workflow_template_context() {
        // Simple test that doesn't rely on environment or file system
        let vars = HashMap::from([("test_var".to_string(), json!("test_value"))]);

        let workflow_context = WorkflowTemplateContext::with_vars(vars).unwrap();

        // Should be able to get the value we set
        assert_eq!(
            workflow_context.get("test_var").unwrap(),
            &json!("test_value")
        );

        // Should be able to initialize a workflow context
        let context = workflow_context.initialize_workflow_context();
        assert!(context.contains_key("_template_vars"));

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        assert_eq!(template_vars.get("test_var").unwrap(), &json!("test_value"));
    }

    #[test]
    fn test_update_workflow_context_preserves_workflow_vars() {
        // Create a template context with some values
        let vars = HashMap::from([
            ("config_var".to_string(), json!("config_value")),
            ("shared_var".to_string(), json!("config_shared")),
        ]);
        let workflow_context = WorkflowTemplateContext::with_vars(vars).unwrap();

        // Create context with existing workflow variables
        let mut context = HashMap::new();
        context.insert(
            "_template_vars".to_string(),
            json!({
                "workflow_var": "workflow_value",
                "shared_var": "workflow_shared" // Should override config
            }),
        );

        // Update with configuration - should preserve workflow vars
        workflow_context.update_workflow_context(&mut context);

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();

        // Workflow variable should be preserved
        assert_eq!(
            template_vars.get("workflow_var").unwrap(),
            &json!("workflow_value")
        );

        // Config var should be added
        assert_eq!(
            template_vars.get("config_var").unwrap(),
            &json!("config_value")
        );

        // Workflow override should win over config
        assert_eq!(
            template_vars.get("shared_var").unwrap(),
            &json!("workflow_shared")
        );
    }

    #[test]
    fn test_liquid_context_conversion() {
        let vars = HashMap::from([
            ("project_name".to_string(), json!("TestProject")),
            ("version".to_string(), json!("1.0.0")),
        ]);

        let workflow_context = WorkflowTemplateContext::with_vars(vars).unwrap();
        let liquid_context = workflow_context.to_liquid_context();

        // Should be able to use in liquid template
        let template_source = "{{project_name}} v{{version}}";
        let liquid_parser = liquid::ParserBuilder::with_stdlib().build().unwrap();
        let template = liquid_parser.parse(template_source).unwrap();
        let output = template.render(&liquid_context).unwrap();

        assert_eq!(output, "TestProject v1.0.0");
    }

    #[test]
    fn test_liquid_context_with_workflow_vars() {
        // Template context with base values
        let template_vars = HashMap::from([
            ("project_name".to_string(), json!("BaseProject")),
            ("version".to_string(), json!("1.0.0")),
        ]);
        let mut workflow_context = WorkflowTemplateContext::with_vars(template_vars).unwrap();

        // Workflow variables that should override template values
        let workflow_vars = HashMap::from([
            ("project_name".to_string(), json!("OverrideProject")),
            ("build_number".to_string(), json!(42)),
        ]);

        workflow_context.set_workflow_vars(workflow_vars);
        let liquid_context = workflow_context.to_liquid_context();

        // Test that workflow vars override template vars
        let template_source = "{{project_name}} v{{version}} build{{build_number}}";
        let liquid_parser = liquid::ParserBuilder::with_stdlib().build().unwrap();
        let template = liquid_parser.parse(template_source).unwrap();
        let output = template.render(&liquid_context).unwrap();

        assert_eq!(output, "OverrideProject v1.0.0 build42");
    }

    #[test]
    fn test_render_template_with_liquid_syntax() {
        let template_vars = HashMap::from([
            ("user_name".to_string(), json!("Alice")),
            ("greeting".to_string(), json!("Hello")),
        ]);
        let mut workflow_context = WorkflowTemplateContext::with_vars(template_vars).unwrap();

        let workflow_vars = HashMap::from([
            ("action".to_string(), json!("deployed")),
            ("service".to_string(), json!("API")),
        ]);

        workflow_context.set_workflow_vars(workflow_vars);
        let template = "{{greeting}} {{user_name}}! The {{service}} was {{action}}.";
        let result = workflow_context.render_template(template);

        assert_eq!(result, "Hello Alice! The API was deployed.");
    }

    #[test]
    fn test_render_template_with_fallback_syntax() {
        let template_vars = HashMap::from([
            ("base_url".to_string(), json!("https://example.com")),
        ]);
        let mut workflow_context = WorkflowTemplateContext::with_vars(template_vars).unwrap();

        let workflow_vars = HashMap::from([
            ("endpoint".to_string(), json!("/api/v1/users")),
            ("method".to_string(), json!("GET")),
        ]);

        workflow_context.set_workflow_vars(workflow_vars);
        let template = "${method} ${base_url}${endpoint}";
        let result = workflow_context.render_template(template);

        assert_eq!(result, "GET https://example.com/api/v1/users");
    }

    #[test]
    fn test_render_template_mixed_syntax() {
        let template_vars = HashMap::from([
            ("app_name".to_string(), json!("MyApp")),
            ("version".to_string(), json!("2.0.0")),
        ]);
        let mut workflow_context = WorkflowTemplateContext::with_vars(template_vars).unwrap();

        let workflow_vars = HashMap::from([
            ("environment".to_string(), json!("production")),
            ("timestamp".to_string(), json!("2024-01-15T10:30:00Z")),
        ]);

        workflow_context.set_workflow_vars(workflow_vars);
        // Mix of liquid {{}} and fallback ${} syntax
        let template = "Deploying {{app_name}} v{{version}} to ${environment} at ${timestamp}";
        let result = workflow_context.render_template(template);

        assert_eq!(result, "Deploying MyApp v2.0.0 to production at 2024-01-15T10:30:00Z");
    }

    #[test]
    fn test_render_template_workflow_vars_precedence() {
        // Template has a base value
        let template_vars = HashMap::from([
            ("database_host".to_string(), json!("localhost")),
            ("database_port".to_string(), json!(5432)),
        ]);
        let mut workflow_context = WorkflowTemplateContext::with_vars(template_vars).unwrap();

        // Workflow overrides the host but not port
        let workflow_vars = HashMap::from([
            ("database_host".to_string(), json!("prod.example.com")),
        ]);

        workflow_context.set_workflow_vars(workflow_vars);
        let template = "postgresql://{{database_host}}:{{database_port}}/mydb";
        let result = workflow_context.render_template(template);

        assert_eq!(result, "postgresql://prod.example.com:5432/mydb");
    }

    #[test]
    fn test_render_template_ignores_internal_keys() {
        let template_vars = HashMap::from([
            ("public_var".to_string(), json!("public_value")),
        ]);
        let mut workflow_context = WorkflowTemplateContext::with_vars(template_vars).unwrap();

        let workflow_vars = HashMap::from([
            ("_internal_var".to_string(), json!("should_be_ignored")),
            ("normal_var".to_string(), json!("normal_value")),
        ]);

        workflow_context.set_workflow_vars(workflow_vars);
        let template = "Public: {{public_var}}, Normal: {{normal_var}}, Internal: {{_internal_var}}";
        let result = workflow_context.render_template(template);

        // Internal variable should not be rendered, leaving the template syntax as-is
        assert_eq!(result, "Public: public_value, Normal: normal_value, Internal: {{_internal_var}}");
    }
}
