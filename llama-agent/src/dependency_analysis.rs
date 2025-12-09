use crate::types::{
    AccessType, ConflictType, ParallelConfig, ParameterReference, ReferenceType, ResourceAccess,
    ResourceType, ToolCall, ToolConflict,
};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// Analyzes tool dependencies and conflicts for parallel execution decisions
pub struct DependencyAnalyzer {
    config: ParallelConfig,
    file_operation_patterns: Vec<Regex>,
    network_operation_patterns: Vec<Regex>,
    data_reference_patterns: Vec<Regex>,
}

impl DependencyAnalyzer {
    pub fn new(config: ParallelConfig) -> Self {
        let file_operation_patterns = vec![
            Regex::new(r"(?i)(file|path|directory|folder)").unwrap(),
            Regex::new(r"(?i)\.(txt|json|xml|yaml|yml|toml|csv|log)").unwrap(),
            Regex::new(r"(?i)/(tmp|temp|home|usr|var|etc)").unwrap(),
            Regex::new(r"(?i)\\(temp|users|program|windows)").unwrap(),
        ];

        let network_operation_patterns = vec![
            Regex::new(r"(?i)(url|uri|endpoint|host|port)").unwrap(),
            Regex::new(r"https?://[^\s]+").unwrap(),
            Regex::new(r"(?i)(api|rest|graphql|websocket)").unwrap(),
        ];

        let data_reference_patterns = vec![
            Regex::new(r"\$\{([^}]+)\}").unwrap(), // ${variable} references
            Regex::new(r"@(\w+)").unwrap(),        // @reference syntax
            Regex::new(r"(?i)result[_\s]*of[_\s]*(\w+)").unwrap(), // "result_of_tool" patterns
        ];

        Self {
            config,
            file_operation_patterns,
            network_operation_patterns,
            data_reference_patterns,
        }
    }

    /// Analyzes whether tool calls can be executed in parallel
    pub fn analyze_parallel_execution(&self, tool_calls: &[ToolCall]) -> ParallelExecutionDecision {
        debug!(
            "Analyzing {} tool calls for parallel execution",
            tool_calls.len()
        );

        if tool_calls.len() <= 1 {
            return ParallelExecutionDecision::Sequential("Single tool call".to_string());
        }

        // Check for explicit configuration conflicts
        if let Some(conflict) = self.check_configured_conflicts(tool_calls) {
            return ParallelExecutionDecision::Sequential(format!(
                "Configuration conflict: {}",
                conflict.description
            ));
        }

        // Analyze parameter dependencies
        if let Some(dependency) = self.analyze_parameter_dependencies(tool_calls) {
            return ParallelExecutionDecision::Sequential(format!(
                "Parameter dependency detected: {}",
                dependency
            ));
        }

        // Analyze resource conflicts
        if let Some(conflict) = self.analyze_resource_conflicts(tool_calls) {
            return ParallelExecutionDecision::Sequential(format!(
                "Resource conflict: {}",
                conflict
            ));
        }

        // Check for duplicate tool names (might be interdependent)
        let mut tool_names = HashSet::new();
        for tool_call in tool_calls {
            if !tool_names.insert(&tool_call.name) {
                return ParallelExecutionDecision::Sequential(
                    "Duplicate tool names detected".to_string(),
                );
            }
        }

        ParallelExecutionDecision::Parallel
    }

    /// Analyzes parameter dependencies between tool calls
    fn analyze_parameter_dependencies(&self, tool_calls: &[ToolCall]) -> Option<String> {
        for (i, tool_call) in tool_calls.iter().enumerate() {
            if let Some(refs) = self.extract_parameter_references(&tool_call.arguments) {
                for parameter_ref in refs {
                    // Check if any previous tool call matches the reference
                    for (j, other_tool) in tool_calls.iter().enumerate() {
                        if i != j && other_tool.name == parameter_ref.referenced_tool {
                            debug!(
                                "Found parameter dependency: {} depends on {}",
                                tool_call.name, other_tool.name
                            );
                            return Some(format!(
                                "{} depends on output from {}",
                                tool_call.name, other_tool.name
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    /// Extracts parameter references from tool arguments
    fn extract_parameter_references(&self, arguments: &Value) -> Option<Vec<ParameterReference>> {
        let mut references = Vec::new();

        self.extract_references_recursive(arguments, "", &mut references);

        if references.is_empty() {
            None
        } else {
            Some(references)
        }
    }

    /// Recursively searches for references in JSON values
    fn extract_references_recursive(
        &self,
        value: &Value,
        path: &str,
        references: &mut Vec<ParameterReference>,
    ) {
        match value {
            Value::String(s) => {
                for pattern in &self.data_reference_patterns {
                    if let Some(captures) = pattern.captures(s) {
                        if let Some(referenced_tool) = captures.get(1) {
                            references.push(ParameterReference {
                                parameter_name: path
                                    .split('.')
                                    .next_back()
                                    .unwrap_or(path)
                                    .to_string(),
                                parameter_path: path.to_string(),
                                referenced_tool: referenced_tool.as_str().to_string(),
                                reference_type: ReferenceType::DirectOutput,
                                target_tool: None,
                            });
                        }
                    }
                }
            }
            Value::Object(obj) => {
                for (key, val) in obj {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };
                    self.extract_references_recursive(val, &new_path, references);
                }
            }
            Value::Array(arr) => {
                for (idx, val) in arr.iter().enumerate() {
                    let new_path = format!("{}[{}]", path, idx);
                    self.extract_references_recursive(val, &new_path, references);
                }
            }
            _ => {}
        }
    }

    /// Analyzes resource conflicts between tool calls
    fn analyze_resource_conflicts(&self, tool_calls: &[ToolCall]) -> Option<String> {
        let mut resource_usage: HashMap<String, Vec<(String, AccessType)>> = HashMap::new();

        for tool_call in tool_calls {
            let resources = self.infer_resource_access(&tool_call.name, &tool_call.arguments);

            for resource in resources {
                let resource_key = match &resource.resource {
                    ResourceType::File(path) => format!("file:{}", path),
                    ResourceType::FileSystem(path) => format!("fs:{}", path),
                    ResourceType::Network(url) => format!("net:{}", url),
                    ResourceType::Database(db) => format!("db:{}", db),
                    ResourceType::Memory => "mem:shared".to_string(),
                    ResourceType::System => "sys:shared".to_string(),
                    ResourceType::Other(name) => format!("other:{}", name),
                };

                resource_usage
                    .entry(resource_key.clone())
                    .or_default()
                    .push((tool_call.name.clone(), resource.access_type.clone()));
            }
        }

        // Check for conflicts
        for (resource, accesses) in resource_usage {
            if accesses.len() > 1 {
                let has_write = accesses.iter().any(|(_, access)| {
                    matches!(
                        access,
                        AccessType::Write | AccessType::ReadWrite | AccessType::Delete
                    )
                });

                if has_write {
                    let tool_names: Vec<String> =
                        accesses.iter().map(|(name, _)| name.clone()).collect();
                    return Some(format!(
                        "Resource conflict on {}: tools {} have conflicting access patterns",
                        resource,
                        tool_names.join(", ")
                    ));
                }
            }
        }

        None
    }

    /// Infers resource access patterns from tool name and arguments
    fn infer_resource_access(&self, tool_name: &str, arguments: &Value) -> Vec<ResourceAccess> {
        let mut resources = Vec::new();

        // Check configured patterns first
        if let Some(patterns) = self.config.resource_access_patterns.get(tool_name) {
            return patterns.clone();
        }

        // Infer from tool name
        self.infer_from_tool_name(tool_name, &mut resources);

        // Infer from arguments
        self.infer_from_arguments(arguments, &mut resources);

        resources
    }

    /// Infers resource access from tool name patterns
    fn infer_from_tool_name(&self, tool_name: &str, resources: &mut Vec<ResourceAccess>) {
        let lower_name = tool_name.to_lowercase();

        // File system operations
        if lower_name.contains("file")
            || lower_name.contains("directory")
            || lower_name.contains("path")
        {
            let access_type = if lower_name.contains("read")
                || lower_name.contains("list")
                || lower_name.contains("get")
            {
                AccessType::Read
            } else if lower_name.contains("write")
                || lower_name.contains("create")
                || lower_name.contains("update")
            {
                AccessType::Write
            } else if lower_name.contains("delete") || lower_name.contains("remove") {
                AccessType::Delete
            } else {
                AccessType::ReadWrite
            };

            let exclusive = matches!(access_type, AccessType::Write | AccessType::Delete);
            resources.push(ResourceAccess {
                resource: ResourceType::FileSystem("*".to_string()),
                access_type,
                exclusive,
            });
        }

        // Network operations
        if lower_name.contains("http")
            || lower_name.contains("api")
            || lower_name.contains("fetch")
            || lower_name.contains("request")
        {
            resources.push(ResourceAccess {
                resource: ResourceType::Network("*".to_string()),
                access_type: AccessType::ReadWrite,
                exclusive: false,
            });
        }
    }

    /// Infers resource access from arguments
    fn infer_from_arguments(&self, arguments: &Value, resources: &mut Vec<ResourceAccess>) {
        let arg_string = arguments.to_string();

        // Check file patterns
        for pattern in &self.file_operation_patterns {
            if pattern.is_match(&arg_string) {
                resources.push(ResourceAccess {
                    resource: ResourceType::FileSystem("inferred".to_string()),
                    access_type: AccessType::ReadWrite,
                    exclusive: false,
                });
                break;
            }
        }

        // Check network patterns
        for pattern in &self.network_operation_patterns {
            if pattern.is_match(&arg_string) {
                resources.push(ResourceAccess {
                    resource: ResourceType::Network("inferred".to_string()),
                    access_type: AccessType::ReadWrite,
                    exclusive: false,
                });
                break;
            }
        }
    }

    /// Checks for explicitly configured conflicts
    fn check_configured_conflicts(&self, tool_calls: &[ToolCall]) -> Option<ToolConflict> {
        let tool_names: HashSet<&str> = tool_calls.iter().map(|call| call.name.as_str()).collect();

        for conflict in &self.config.tool_conflicts {
            if tool_names.contains(conflict.tool1.as_str())
                && tool_names.contains(conflict.tool2.as_str())
            {
                return Some(conflict.clone());
            }
        }

        // Check never_parallel pairs
        for (tool1, tool2) in &self.config.never_parallel {
            if tool_names.contains(tool1.as_str()) && tool_names.contains(tool2.as_str()) {
                return Some(ToolConflict {
                    tool1: tool1.clone(),
                    tool2: tool2.clone(),
                    conflict_type: ConflictType::MutualExclusion,
                    description: "Configured never to run in parallel".to_string(),
                });
            }
        }

        None
    }
}

/// Decision about parallel execution
#[derive(Debug, Clone)]
pub enum ParallelExecutionDecision {
    Parallel,
    Sequential(String), // Reason for sequential execution
}

impl Default for DependencyAnalyzer {
    fn default() -> Self {
        Self::new(ParallelConfig::default())
    }
}
