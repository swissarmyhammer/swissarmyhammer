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
///     flow_name: "plan".to_string(),
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
    /// Operation to perform: "run" (execute workflow), "exit" (terminate current workflow), or "list" (show workflows)
    #[serde(default)]
    pub op: Option<String>,

    /// Name of the workflow to execute, or "list" to show all workflows
    #[serde(default)]
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
            op: None,
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
            op: Some("list".to_string()),
            flow_name: String::new(),
            parameters: Default::default(),
            format: Some("json".to_string()),
            verbose: false,
            interactive: false,
            dry_run: false,
            quiet: false,
        }
    }

    /// Create a new FlowToolRequest for exiting the current workflow
    pub fn exit() -> Self {
        Self {
            op: Some("exit".to_string()),
            flow_name: String::new(),
            parameters: Default::default(),
            format: None,
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
        self.op.as_deref() == Some("list") || self.flow_name == "list"
    }

    /// Check if this is an exit request
    pub fn is_exit(&self) -> bool {
        self.op.as_deref() == Some("exit")
    }

    /// Resolve the effective operation based on op field and flow_name fallback
    pub fn effective_op(&self) -> &str {
        if let Some(op) = &self.op {
            op.as_str()
        } else if self.flow_name == "list" {
            "list"
        } else {
            "run"
        }
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), String> {
        // Validate op if provided
        if let Some(op) = &self.op {
            match op.as_str() {
                "run" | "exit" | "list" => {}
                _ => {
                    return Err(format!(
                        "Invalid op: '{}'. Must be 'run', 'exit', or 'list'",
                        op
                    ))
                }
            }
        }

        // flow_name is required for run, but not for exit or list
        match self.effective_op() {
            "run" => {
                if self.flow_name.trim().is_empty() {
                    return Err("flow_name is required when op is 'run'".to_string());
                }
            }
            "exit" | "list" => {}
            _ => {} // already validated above
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
///             name: "plan".to_string(),
///             description: "Execute the plan workflow".to_string(),
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
///     "plan".to_string(),
///     "review".to_string(),
/// ]);
/// ```
pub fn generate_flow_tool_schema(workflow_names: Vec<String>) -> JsonValue {
    let mut flow_names = vec!["list".to_string()];
    flow_names.extend(workflow_names);

    serde_json::json!({
        "type": "object",
        "properties": {
            "op": {
                "type": "string",
                "description": "Operation to perform: 'run' (execute workflow), 'exit' (terminate current workflow), or 'list' (show workflows). If omitted, inferred from flow_name.",
                "enum": ["run", "exit", "list"]
            },
            "flow_name": {
                "type": "string",
                "description": "Name of the workflow to execute (required for 'run' op)",
                "enum": flow_names
            },
            "parameters": {
                "type": "object",
                "description": "Workflow-specific parameters as key-value pairs (only used with 'run' op)",
                "additionalProperties": {
                    "type": ["string", "boolean", "number", "object", "array", "null"]
                },
                "default": {}
            },
            "format": {
                "type": "string",
                "description": "Output format when op='list'",
                "enum": ["json", "yaml", "table"],
                "default": "json"
            },
            "verbose": {
                "type": "boolean",
                "description": "Include detailed parameter information when op='list'",
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
        "required": ["op"]
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
        let request = FlowToolRequest::new("plan");
        assert_eq!(request.op, None);
        assert_eq!(request.flow_name, "plan");
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
        assert_eq!(request.op, Some("list".to_string()));
        assert!(request.is_list());
        assert_eq!(request.format, Some("json".to_string()));
    }

    #[test]
    fn test_flow_tool_request_exit() {
        let request = FlowToolRequest::exit();
        assert_eq!(request.op, Some("exit".to_string()));
        assert!(request.is_exit());
        assert!(!request.is_list());
        assert_eq!(request.effective_op(), "exit");
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

        let exec_request = FlowToolRequest::new("plan");
        assert!(!exec_request.is_list());

        // Backward compat: flow_name="list" still works
        let legacy_list = FlowToolRequest::new("list");
        assert!(legacy_list.is_list());
    }

    #[test]
    fn test_flow_tool_request_effective_op() {
        // Explicit op takes priority
        let request = FlowToolRequest::exit();
        assert_eq!(request.effective_op(), "exit");

        let request = FlowToolRequest::list();
        assert_eq!(request.effective_op(), "list");

        // No op: infer from flow_name
        let request = FlowToolRequest::new("plan");
        assert_eq!(request.effective_op(), "run");

        // flow_name="list" infers list op
        let request = FlowToolRequest::new("list");
        assert_eq!(request.effective_op(), "list");
    }

    #[test]
    fn test_flow_tool_request_validation_success() {
        let request = FlowToolRequest::new("plan");
        assert!(request.validate().is_ok());

        let list_request = FlowToolRequest::list().with_format("yaml");
        assert!(list_request.validate().is_ok());
    }

    #[test]
    fn test_flow_tool_request_validation_empty_name_for_run() {
        let request = FlowToolRequest::new("");
        assert!(request.validate().is_err());
        assert!(request
            .validate()
            .unwrap_err()
            .contains("flow_name is required"));
    }

    #[test]
    fn test_flow_tool_request_validation_empty_name_ok_for_exit() {
        let request = FlowToolRequest::exit();
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_flow_tool_request_validation_invalid_op() {
        let mut request = FlowToolRequest::new("plan");
        request.op = Some("bogus".to_string());
        assert!(request.validate().is_err());
        assert!(request.validate().unwrap_err().contains("Invalid op"));
    }

    #[test]
    fn test_flow_tool_request_validation_invalid_format() {
        let request = FlowToolRequest::list().with_format("invalid");
        assert!(request.validate().is_err());
        assert!(request.validate().unwrap_err().contains("Invalid format"));
    }

    #[test]
    fn test_flow_tool_request_serialization_minimal() {
        let request = FlowToolRequest::new("plan");
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
        let json = r#"{"flow_name": "plan"}"#;
        let deserialized: FlowToolRequest = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.op, None);
        assert_eq!(deserialized.flow_name, "plan");
        assert!(deserialized.parameters.is_empty());
        assert_eq!(deserialized.format, None);
        assert!(!deserialized.verbose);
        assert!(!deserialized.interactive);
        assert!(!deserialized.dry_run);
        assert!(!deserialized.quiet);
    }

    #[test]
    fn test_flow_tool_request_deserialization_with_op() {
        let json = r#"{"op": "exit"}"#;
        let deserialized: FlowToolRequest = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.op, Some("exit".to_string()));
        assert!(deserialized.is_exit());
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
        let workflows = vec![WorkflowMetadata::new("plan", "Execute plan", "builtin")];
        let response = WorkflowListResponse::new(workflows);

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: WorkflowListResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.workflows.len(), 1);
        assert_eq!(deserialized.workflows[0].name, "plan");
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
        let metadata =
            WorkflowMetadata::new("plan", "Planning workflow", "builtin").with_parameter(
                WorkflowParameter::new("param", "string", "A parameter", true),
            );

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: WorkflowMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "plan");
        assert_eq!(deserialized.description, "Planning workflow");
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
        let workflows = vec!["plan".to_string(), "review".to_string()];
        let schema = generate_flow_tool_schema(workflows);

        let flow_name_enum = schema["properties"]["flow_name"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(flow_name_enum.len(), 3);
        assert_eq!(flow_name_enum[0], "list"); // list is always first
        assert_eq!(flow_name_enum[1], "plan");
        assert_eq!(flow_name_enum[2], "review");
    }

    #[test]
    fn test_generate_flow_tool_schema_structure() {
        let schema = generate_flow_tool_schema(vec!["test".to_string()]);

        assert_eq!(schema["type"], "object");

        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("op"));
        assert!(properties.contains_key("flow_name"));
        assert!(properties.contains_key("parameters"));
        assert!(properties.contains_key("format"));
        assert!(properties.contains_key("verbose"));
        assert!(properties.contains_key("interactive"));
        assert!(properties.contains_key("dry_run"));
        assert!(properties.contains_key("quiet"));

        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "op");

        // Verify op enum
        let op_enum = schema["properties"]["op"]["enum"].as_array().unwrap();
        assert_eq!(op_enum.len(), 3);
        assert!(op_enum.contains(&json!("run")));
        assert!(op_enum.contains(&json!("exit")));
        assert!(op_enum.contains(&json!("list")));
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
    fn test_generate_flow_tool_schema_op_enum() {
        let schema = generate_flow_tool_schema(vec![]);

        let op_enum = schema["properties"]["op"]["enum"].as_array().unwrap();
        assert!(op_enum.contains(&json!("run")));
        assert!(op_enum.contains(&json!("exit")));
        assert!(op_enum.contains(&json!("list")));
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
