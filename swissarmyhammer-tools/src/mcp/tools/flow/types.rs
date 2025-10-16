//! Request and response types for flow MCP operations

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Request to execute a workflow or list available workflows
///
/// # Examples
///
/// Execute a workflow:
/// ```ignore
/// FlowToolRequest {
///     flow_name: "implement".to_string(),
///     parameters: Default::default(),
///     format: None,
///     verbose: false,
///     interactive: false,
///     dry_run: false,
///     quiet: false,
/// }
/// ```
///
/// List workflows:
/// ```ignore
/// FlowToolRequest {
///     flow_name: "list".to_string(),
///     parameters: Default::default(),
///     format: Some("json".to_string()),
///     verbose: true,
///     interactive: false,
///     dry_run: false,
///     quiet: false,
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FlowToolRequest {
    /// Name of the workflow to execute, or "list" to show all workflows
    pub flow_name: String,

    /// Workflow-specific parameters as key-value pairs (ignored when flow_name='list')
    #[serde(default)]
    pub parameters: serde_json::Map<String, JsonValue>,

    /// Output format when flow_name='list' (json, yaml, or table)
    #[serde(default)]
    pub format: Option<String>,

    /// Include detailed parameter information when flow_name='list'
    #[serde(default)]
    pub verbose: bool,

    /// Enable interactive mode for prompts (workflow execution only)
    #[serde(default)]
    pub interactive: bool,

    /// Show execution plan without running (workflow execution only)
    #[serde(default)]
    pub dry_run: bool,

    /// Suppress progress output (workflow execution only)
    #[serde(default)]
    pub quiet: bool,
}

impl FlowToolRequest {
    /// Create a new FlowToolRequest for workflow execution
    pub fn new(flow_name: impl Into<String>) -> Self {
        Self {
            flow_name: flow_name.into(),
            parameters: Default::default(),
            format: None,
            verbose: false,
            interactive: false,
            dry_run: false,
            quiet: false,
        }
    }

    /// Create a new FlowToolRequest for listing workflows
    pub fn list() -> Self {
        Self {
            flow_name: "list".to_string(),
            parameters: Default::default(),
            format: Some("json".to_string()),
            verbose: false,
            interactive: false,
            dry_run: false,
            quiet: false,
        }
    }

    /// Set workflow parameters
    pub fn with_parameters(mut self, parameters: serde_json::Map<String, JsonValue>) -> Self {
        self.parameters = parameters;
        self
    }

    /// Set output format
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Set verbose flag
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set interactive flag
    pub fn with_interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Set dry run flag
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Set quiet flag
    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Check if this is a list request
    pub fn is_list(&self) -> bool {
        self.flow_name == "list"
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), String> {
        if self.flow_name.trim().is_empty() {
            return Err("flow_name cannot be empty".to_string());
        }

        if let Some(format) = &self.format {
            match format.as_str() {
                "json" | "yaml" | "table" => {}
                _ => {
                    return Err(format!(
                        "Invalid format: {}. Must be json, yaml, or table",
                        format
                    ))
                }
            }
        }

        Ok(())
    }
}

/// Response containing list of available workflows
///
/// # Examples
///
/// ```ignore
/// WorkflowListResponse {
///     workflows: vec![
///         WorkflowMetadata {
///             name: "implement".to_string(),
///             description: "Execute the implement workflow".to_string(),
///             source: "builtin".to_string(),
///             parameters: vec![],
///         }
///     ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowListResponse {
    /// List of available workflows
    pub workflows: Vec<WorkflowMetadata>,
}

impl WorkflowListResponse {
    /// Create a new WorkflowListResponse
    pub fn new(workflows: Vec<WorkflowMetadata>) -> Self {
        Self { workflows }
    }
}

/// Metadata about a workflow
///
/// # Examples
///
/// ```ignore
/// WorkflowMetadata {
///     name: "plan".to_string(),
///     description: "Execute planning workflow".to_string(),
///     source: "builtin".to_string(),
///     parameters: vec![
///         WorkflowParameter {
///             name: "plan_filename".to_string(),
///             param_type: "string".to_string(),
///             description: "Path to the specification file".to_string(),
///             required: true,
///         }
///     ],
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowMetadata {
    /// Name of the workflow
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// Source of the workflow (builtin, project, user)
    pub source: String,

    /// List of workflow parameters
    pub parameters: Vec<WorkflowParameter>,
}

impl WorkflowMetadata {
    /// Create a new WorkflowMetadata
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            source: source.into(),
            parameters: vec![],
        }
    }

    /// Add a parameter to the workflow
    pub fn with_parameter(mut self, parameter: WorkflowParameter) -> Self {
        self.parameters.push(parameter);
        self
    }

    /// Add multiple parameters to the workflow
    pub fn with_parameters(mut self, parameters: Vec<WorkflowParameter>) -> Self {
        self.parameters = parameters;
        self
    }
}

/// Parameter definition for a workflow
///
/// # Examples
///
/// ```ignore
/// WorkflowParameter {
///     name: "plan_filename".to_string(),
///     param_type: "string".to_string(),
///     description: "Path to the specification file to process".to_string(),
///     required: true,
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowParameter {
    /// Name of the parameter
    pub name: String,

    /// Type of the parameter (string, integer, boolean, etc.)
    #[serde(rename = "type")]
    pub param_type: String,

    /// Human-readable description
    pub description: String,

    /// Whether the parameter is required
    pub required: bool,
}

impl WorkflowParameter {
    /// Create a new WorkflowParameter
    pub fn new(
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        Self {
            name: name.into(),
            param_type: param_type.into(),
            description: description.into(),
            required,
        }
    }
}

/// Generate JSON schema for the flow tool with dynamic workflow names
///
/// This function generates a complete JSON schema for the flow tool, including
/// a dynamic enum of workflow names. The "list" special case is always included
/// as the first option.
///
/// # Arguments
///
/// * `workflow_names` - List of available workflow names to include in the enum
///
/// # Returns
///
/// A JSON schema object suitable for MCP tool registration
///
/// # Examples
///
/// ```ignore
/// let schema = generate_flow_tool_schema(vec![
///     "implement".to_string(),
///     "plan".to_string(),
/// ]);
/// ```
pub fn generate_flow_tool_schema(workflow_names: Vec<String>) -> JsonValue {
    let mut flow_names = vec!["list".to_string()];
    flow_names.extend(workflow_names);

    serde_json::json!({
        "type": "object",
        "properties": {
            "flow_name": {
                "type": "string",
                "description": "Name of the workflow to execute, or 'list' to show all workflows",
                "enum": flow_names
            },
            "parameters": {
                "type": "object",
                "description": "Workflow-specific parameters as key-value pairs (ignored when flow_name='list')",
                "additionalProperties": true,
                "default": {}
            },
            "format": {
                "type": "string",
                "description": "Output format when flow_name='list'",
                "enum": ["json", "yaml", "table"],
                "default": "json"
            },
            "verbose": {
                "type": "boolean",
                "description": "Include detailed parameter information when flow_name='list'",
                "default": false
            },
            "interactive": {
                "type": "boolean",
                "description": "Enable interactive mode for prompts (workflow execution only)",
                "default": false
            },
            "dry_run": {
                "type": "boolean",
                "description": "Show execution plan without running (workflow execution only)",
                "default": false
            },
            "quiet": {
                "type": "boolean",
                "description": "Suppress progress output (workflow execution only)",
                "default": false
            }
        },
        "required": ["flow_name"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ============================================================================
    // FlowToolRequest Tests
    // ============================================================================

    #[test]
    fn test_flow_tool_request_new() {
        let request = FlowToolRequest::new("implement");
        assert_eq!(request.flow_name, "implement");
        assert!(request.parameters.is_empty());
        assert_eq!(request.format, None);
        assert!(!request.verbose);
        assert!(!request.interactive);
        assert!(!request.dry_run);
        assert!(!request.quiet);
    }

    #[test]
    fn test_flow_tool_request_list() {
        let request = FlowToolRequest::list();
        assert_eq!(request.flow_name, "list");
        assert!(request.is_list());
        assert_eq!(request.format, Some("json".to_string()));
    }

    #[test]
    fn test_flow_tool_request_builder_pattern() {
        let mut params = serde_json::Map::new();
        params.insert("plan_filename".to_string(), json!("spec.md"));

        let request = FlowToolRequest::new("plan")
            .with_parameters(params.clone())
            .with_interactive(true)
            .with_dry_run(true)
            .with_quiet(true);

        assert_eq!(request.flow_name, "plan");
        assert_eq!(request.parameters, params);
        assert!(request.interactive);
        assert!(request.dry_run);
        assert!(request.quiet);
    }

    #[test]
    fn test_flow_tool_request_is_list() {
        let list_request = FlowToolRequest::list();
        assert!(list_request.is_list());

        let exec_request = FlowToolRequest::new("implement");
        assert!(!exec_request.is_list());
    }

    #[test]
    fn test_flow_tool_request_validation_success() {
        let request = FlowToolRequest::new("implement");
        assert!(request.validate().is_ok());

        let list_request = FlowToolRequest::list().with_format("yaml");
        assert!(list_request.validate().is_ok());
    }

    #[test]
    fn test_flow_tool_request_validation_empty_name() {
        let request = FlowToolRequest::new("");
        assert!(request.validate().is_err());
        assert!(request.validate().unwrap_err().contains("empty"));
    }

    #[test]
    fn test_flow_tool_request_validation_invalid_format() {
        let request = FlowToolRequest::list().with_format("invalid");
        assert!(request.validate().is_err());
        assert!(request.validate().unwrap_err().contains("Invalid format"));
    }

    #[test]
    fn test_flow_tool_request_serialization_minimal() {
        let request = FlowToolRequest::new("implement");
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: FlowToolRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.flow_name, deserialized.flow_name);
        assert_eq!(request.parameters, deserialized.parameters);
    }

    #[test]
    fn test_flow_tool_request_serialization_full() {
        let mut params = serde_json::Map::new();
        params.insert("key".to_string(), json!("value"));

        let request = FlowToolRequest::new("plan")
            .with_parameters(params.clone())
            .with_format("yaml")
            .with_verbose(true)
            .with_interactive(true)
            .with_dry_run(true)
            .with_quiet(true);

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: FlowToolRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.flow_name, deserialized.flow_name);
        assert_eq!(request.parameters, deserialized.parameters);
        assert_eq!(request.format, deserialized.format);
        assert_eq!(request.verbose, deserialized.verbose);
        assert_eq!(request.interactive, deserialized.interactive);
        assert_eq!(request.dry_run, deserialized.dry_run);
        assert_eq!(request.quiet, deserialized.quiet);
    }

    #[test]
    fn test_flow_tool_request_deserialization_with_defaults() {
        let json = r#"{"flow_name": "implement"}"#;
        let deserialized: FlowToolRequest = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.flow_name, "implement");
        assert!(deserialized.parameters.is_empty());
        assert_eq!(deserialized.format, None);
        assert!(!deserialized.verbose);
        assert!(!deserialized.interactive);
        assert!(!deserialized.dry_run);
        assert!(!deserialized.quiet);
    }

    // ============================================================================
    // WorkflowListResponse Tests
    // ============================================================================

    #[test]
    fn test_workflow_list_response_new() {
        let workflows = vec![WorkflowMetadata::new("test", "Test workflow", "builtin")];
        let response = WorkflowListResponse::new(workflows.clone());
        assert_eq!(response.workflows.len(), 1);
        assert_eq!(response.workflows[0].name, "test");
    }

    #[test]
    fn test_workflow_list_response_serialization() {
        let workflows = vec![WorkflowMetadata::new(
            "implement",
            "Execute implement",
            "builtin",
        )];
        let response = WorkflowListResponse::new(workflows);

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: WorkflowListResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.workflows.len(), 1);
        assert_eq!(deserialized.workflows[0].name, "implement");
    }

    // ============================================================================
    // WorkflowMetadata Tests
    // ============================================================================

    #[test]
    fn test_workflow_metadata_new() {
        let metadata = WorkflowMetadata::new("plan", "Planning workflow", "builtin");
        assert_eq!(metadata.name, "plan");
        assert_eq!(metadata.description, "Planning workflow");
        assert_eq!(metadata.source, "builtin");
        assert!(metadata.parameters.is_empty());
    }

    #[test]
    fn test_workflow_metadata_with_parameter() {
        let param = WorkflowParameter::new("filename", "string", "File path", true);
        let metadata = WorkflowMetadata::new("test", "Test", "builtin").with_parameter(param);

        assert_eq!(metadata.parameters.len(), 1);
        assert_eq!(metadata.parameters[0].name, "filename");
    }

    #[test]
    fn test_workflow_metadata_with_parameters() {
        let params = vec![
            WorkflowParameter::new("param1", "string", "First param", true),
            WorkflowParameter::new("param2", "integer", "Second param", false),
        ];
        let metadata = WorkflowMetadata::new("test", "Test", "builtin").with_parameters(params);

        assert_eq!(metadata.parameters.len(), 2);
        assert_eq!(metadata.parameters[0].name, "param1");
        assert_eq!(metadata.parameters[1].name, "param2");
    }

    #[test]
    fn test_workflow_metadata_serialization() {
        let metadata = WorkflowMetadata::new("implement", "Implementation workflow", "builtin")
            .with_parameter(WorkflowParameter::new(
                "param",
                "string",
                "A parameter",
                true,
            ));

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: WorkflowMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "implement");
        assert_eq!(deserialized.description, "Implementation workflow");
        assert_eq!(deserialized.source, "builtin");
        assert_eq!(deserialized.parameters.len(), 1);
    }

    // ============================================================================
    // WorkflowParameter Tests
    // ============================================================================

    #[test]
    fn test_workflow_parameter_new() {
        let param = WorkflowParameter::new("plan_filename", "string", "File path", true);
        assert_eq!(param.name, "plan_filename");
        assert_eq!(param.param_type, "string");
        assert_eq!(param.description, "File path");
        assert!(param.required);
    }

    #[test]
    fn test_workflow_parameter_serialization() {
        let param = WorkflowParameter::new("test_param", "integer", "Test parameter", false);

        let json = serde_json::to_string(&param).unwrap();
        let deserialized: WorkflowParameter = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "test_param");
        assert_eq!(deserialized.param_type, "integer");
        assert_eq!(deserialized.description, "Test parameter");
        assert!(!deserialized.required);
    }

    #[test]
    fn test_workflow_parameter_type_field_rename() {
        let param = WorkflowParameter::new("param", "string", "Description", true);
        let json = serde_json::to_value(&param).unwrap();

        assert!(json.get("type").is_some());
        assert_eq!(json["type"], "string");
    }

    // ============================================================================
    // Schema Generation Tests
    // ============================================================================

    #[test]
    fn test_generate_flow_tool_schema_empty() {
        let schema = generate_flow_tool_schema(vec![]);

        assert!(schema.is_object());
        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("flow_name"));

        let flow_name_enum = schema["properties"]["flow_name"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(flow_name_enum.len(), 1);
        assert_eq!(flow_name_enum[0], "list");
    }

    #[test]
    fn test_generate_flow_tool_schema_with_workflows() {
        let workflows = vec!["implement".to_string(), "plan".to_string()];
        let schema = generate_flow_tool_schema(workflows);

        let flow_name_enum = schema["properties"]["flow_name"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(flow_name_enum.len(), 3);
        assert_eq!(flow_name_enum[0], "list"); // list is always first
        assert_eq!(flow_name_enum[1], "implement");
        assert_eq!(flow_name_enum[2], "plan");
    }

    #[test]
    fn test_generate_flow_tool_schema_structure() {
        let schema = generate_flow_tool_schema(vec!["test".to_string()]);

        assert_eq!(schema["type"], "object");

        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("flow_name"));
        assert!(properties.contains_key("parameters"));
        assert!(properties.contains_key("format"));
        assert!(properties.contains_key("verbose"));
        assert!(properties.contains_key("interactive"));
        assert!(properties.contains_key("dry_run"));
        assert!(properties.contains_key("quiet"));

        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "flow_name");
    }

    #[test]
    fn test_generate_flow_tool_schema_format_enum() {
        let schema = generate_flow_tool_schema(vec![]);

        let format_enum = schema["properties"]["format"]["enum"].as_array().unwrap();
        assert_eq!(format_enum.len(), 3);
        assert!(format_enum.contains(&json!("json")));
        assert!(format_enum.contains(&json!("yaml")));
        assert!(format_enum.contains(&json!("table")));
    }

    #[test]
    fn test_generate_flow_tool_schema_default_values() {
        let schema = generate_flow_tool_schema(vec![]);

        assert_eq!(schema["properties"]["parameters"]["default"], json!({}));
        assert_eq!(schema["properties"]["format"]["default"], "json");
        assert_eq!(schema["properties"]["verbose"]["default"], false);
        assert_eq!(schema["properties"]["interactive"]["default"], false);
        assert_eq!(schema["properties"]["dry_run"]["default"], false);
        assert_eq!(schema["properties"]["quiet"]["default"], false);
    }

    #[test]
    fn test_generate_flow_tool_schema_list_always_first() {
        let workflows = vec!["zebra".to_string(), "alpha".to_string(), "beta".to_string()];
        let schema = generate_flow_tool_schema(workflows);

        let flow_name_enum = schema["properties"]["flow_name"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(flow_name_enum[0], "list");
    }
}
