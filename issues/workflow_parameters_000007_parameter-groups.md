# Parameter Groups and Organization

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Implement parameter groups to organize related parameters together, improving user experience through logical grouping and better help text organization as described in the specification.

## Current State

- Parameters are presented as flat lists
- No logical organization or grouping
- Help text can be overwhelming for workflows with many parameters
- No way to categorize related parameters

## Implementation Tasks

### 1. Parameter Group Schema

Extend workflow frontmatter to support parameter groups:

```yaml
---
title: Deployment Workflow
description: Deploy application to various environments
parameter_groups:
  - name: deployment
    description: Deployment configuration
    parameters: [deploy_env, region, instance_count]
    
  - name: security
    description: Security settings
    parameters: [enable_ssl, cert_path, auth_method]
    
  - name: monitoring
    description: Monitoring and logging
    parameters: [log_level, metrics_enabled, alert_email]

parameters:
  # Deployment group
  - name: deploy_env
    description: Target deployment environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  - name: region
    description: AWS region for deployment
    type: choice
    choices: [us-east-1, us-west-2, eu-west-1]
    required: true
    
  - name: instance_count
    description: Number of instances to deploy
    type: number
    min: 1
    max: 10
    default: 2
    
  # Security group
  - name: enable_ssl
    description: Enable SSL/TLS encryption
    type: boolean
    default: true
    
  - name: cert_path
    description: Path to SSL certificate
    type: string
    condition: "enable_ssl == true"
    pattern: '^.*\.(pem|crt)$'
    
  - name: auth_method
    description: Authentication method
    type: choice
    choices: [basic, oauth2, api_key]
    default: basic
    
  # Monitoring group  
  - name: log_level
    description: Application log level
    type: choice
    choices: [debug, info, warn, error]
    default: info
    
  - name: metrics_enabled
    description: Enable metrics collection
    type: boolean
    default: true
    
  - name: alert_email
    description: Email for alerts
    type: string
    pattern: '^[^@\s]+@[^@\s]+\.[^@\s]+$'
    condition: "metrics_enabled == true"
---
```

### 2. Parameter Group Data Structures

Implement data structures for parameter groups:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterGroup {
    pub name: String,
    pub description: String,
    pub parameters: Vec<String>,        // Parameter names in this group
    pub collapsed: Option<bool>,        // Whether group starts collapsed in UI
    pub condition: Option<String>,      // Group-level condition
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub title: Option<String>,
    pub description: String,
    pub parameters: Vec<Parameter>,
    pub parameter_groups: Option<Vec<ParameterGroup>>,
}

impl WorkflowMetadata {
    pub fn get_parameters_by_group(&self) -> HashMap<String, Vec<&Parameter>> {
        let mut grouped = HashMap::new();
        
        if let Some(groups) = &self.parameter_groups {
            for group in groups {
                let group_params: Vec<&Parameter> = self.parameters
                    .iter()
                    .filter(|p| group.parameters.contains(&p.name))
                    .collect();
                grouped.insert(group.name.clone(), group_params);
            }
        }
        
        // Add ungrouped parameters to default group
        let ungrouped: Vec<&Parameter> = self.parameters
            .iter()
            .filter(|p| !self.is_parameter_in_any_group(&p.name))
            .collect();
            
        if !ungrouped.is_empty() {
            grouped.insert("general".to_string(), ungrouped);
        }
        
        grouped
    }
    
    pub fn is_parameter_in_any_group(&self, param_name: &str) -> bool {
        self.parameter_groups
            .as_ref()
            .map(|groups| {
                groups.iter().any(|g| g.parameters.contains(&param_name.to_string()))
            })
            .unwrap_or(false)
    }
}
```

### 3. Enhanced CLI Help Generation

Update help text generation to display parameters by group:

```rust
impl ParameterHelpGenerator {
    pub fn generate_grouped_help(&self, workflow: &Workflow) -> String {
        let mut help = String::new();
        let grouped_params = workflow.metadata.get_parameters_by_group();
        
        help.push_str(&format!("Execute workflow: {}\n\n", workflow.name));
        help.push_str(&format!("{}\n\n", workflow.description));
        
        // Display parameter groups
        for (group_name, group_params) in &grouped_params {
            if group_params.is_empty() { continue; }
            
            // Find group metadata
            let group_info = workflow.metadata.parameter_groups
                .as_ref()
                .and_then(|groups| groups.iter().find(|g| &g.name == group_name));
                
            let group_title = group_info
                .map(|g| format!("{} - {}", self.capitalize(group_name), g.description))
                .unwrap_or_else(|| self.capitalize(group_name));
                
            help.push_str(&format!("{}:\n", group_title));
            
            // List parameters in group
            for param in group_params {
                help.push_str(&self.format_parameter_help(param));
            }
            
            help.push('\n');
        }
        
        help
    }
    
    fn format_parameter_help(&self, param: &Parameter) -> String {
        let switch_name = param.to_cli_switch();
        let required_indicator = if param.required { " (required)" } else { "" };
        let default_text = param.default
            .as_ref()
            .map(|d| format!(" [default: {}]", d))
            .unwrap_or_default();
            
        format!(
            "  {:<20} {}{}{}\n",
            switch_name,
            param.description,
            required_indicator,
            default_text
        )
    }
}
```

### 4. Interactive Prompting with Groups

Update interactive prompting to present parameters by group:

```rust
impl InteractivePrompts {
    pub async fn prompt_parameters_by_groups(
        &self,
        workflow: &Workflow,
        existing_values: HashMap<String, serde_json::Value>
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut resolved = existing_values;
        let grouped_params = workflow.metadata.get_parameters_by_group();
        
        for (group_name, group_params) in &grouped_params {
            if group_params.is_empty() { continue; }
            
            // Check if any parameters in this group need prompting
            let needs_prompting = group_params.iter()
                .any(|p| !resolved.contains_key(&p.name) && self.should_prompt_parameter(p, &resolved));
                
            if !needs_prompting { continue; }
            
            // Display group header
            self.display_group_header(group_name, &workflow.metadata.parameter_groups).await?;
            
            // Prompt for parameters in this group
            for param in group_params {
                if !resolved.contains_key(&param.name) {
                    if let Some(value) = self.prompt_parameter_conditionally(param, &resolved).await? {
                        resolved.insert(param.name.clone(), value);
                    }
                }
            }
            
            println!(); // Blank line after group
        }
        
        Ok(resolved)
    }
    
    async fn display_group_header(
        &self,
        group_name: &str,
        groups: &Option<Vec<ParameterGroup>>
    ) -> Result<()> {
        if let Some(groups) = groups {
            if let Some(group) = groups.iter().find(|g| g.name == group_name) {
                println!("┌─ {} Configuration", self.capitalize(&group.name));
                println!("│  {}", group.description);
                println!("└─");
                return Ok(());
            }
        }
        
        // Fallback for ungrouped parameters
        println!("┌─ {} Parameters", self.capitalize(group_name));
        println!("└─");
        Ok(())
    }
}
```

### 5. CLI Switch Organization

Organize CLI switches by parameter groups in help output:

```bash
$ sah flow run deploy --help

Execute workflow: deploy

Deploy application to various environments

Deployment Configuration:
  --deploy-env <ENV>         Target deployment environment (required) [possible values: dev, staging, prod]
  --region <REGION>          AWS region for deployment (required) [possible values: us-east-1, us-west-2, eu-west-1] 
  --instance-count <COUNT>   Number of instances to deploy [default: 2] [range: 1-10]

Security Settings:
  --enable-ssl               Enable SSL/TLS encryption [default: true]
  --cert-path <PATH>         Path to SSL certificate (required when SSL enabled)
  --auth-method <METHOD>     Authentication method [default: basic] [possible values: basic, oauth2, api_key]

Monitoring and Logging:
  --log-level <LEVEL>        Application log level [default: info] [possible values: debug, info, warn, error]
  --metrics-enabled          Enable metrics collection [default: true]
  --alert-email <EMAIL>      Email for alerts (required when metrics enabled)

General Options:
  --interactive              Run in interactive mode
  --help                     Print help
```

## Technical Details

### Group Validation

Validate parameter group definitions:

```rust
impl ParameterGroupValidator {
    pub fn validate_groups(&self, workflow: &Workflow) -> Result<(), ValidationError> {
        if let Some(groups) = &workflow.metadata.parameter_groups {
            // Check for duplicate parameter assignments
            let mut assigned_params = HashSet::new();
            
            for group in groups {
                for param_name in &group.parameters {
                    if assigned_params.contains(param_name) {
                        return Err(ValidationError::DuplicateParameterAssignment {
                            parameter: param_name.clone(),
                        });
                    }
                    
                    // Verify parameter exists
                    if !workflow.metadata.parameters.iter().any(|p| &p.name == param_name) {
                        return Err(ValidationError::UnknownParameterInGroup {
                            parameter: param_name.clone(),
                            group: group.name.clone(),
                        });
                    }
                    
                    assigned_params.insert(param_name.clone());
                }
            }
        }
        
        Ok(())
    }
}
```

### File Locations
- `swissarmyhammer/src/common/parameter_groups.rs` - Group data structures
- `swissarmyhammer/src/common/parameter_help.rs` - Help text generation  
- `swissarmyhammer/src/common/interactive_prompts.rs` - Updated prompting logic
- `swissarmyhammer/src/workflow/validation.rs` - Group validation

### Testing Requirements

- Unit tests for parameter group parsing
- Help text generation tests with groups
- Interactive prompting with groups tests
- Group validation tests (duplicates, missing parameters)
- CLI help output format tests
- Empty group handling tests

## Success Criteria

- [ ] Parameters can be organized into logical groups in frontmatter
- [ ] CLI help text displays parameters organized by groups
- [ ] Interactive prompting presents parameters by group with clear headers
- [ ] Group validation prevents duplicate parameter assignments
- [ ] Ungrouped parameters are handled gracefully
- [ ] Group descriptions provide context for related parameters
- [ ] Clean, readable help output with proper formatting

## Dependencies

- Requires completion of workflow_parameters_000001_frontmatter-parameter-schema
- Requires completion of workflow_parameters_000002_shared-parameter-system
- Requires completion of workflow_parameters_000003_cli-parameter-switches
- Requires completion of workflow_parameters_000004_interactive-parameter-prompting

## Example User Experience

```bash
$ sah flow run deploy --interactive

┌─ Deployment Configuration  
│  Target environment and infrastructure settings
└─
? Select deploy_env: prod
? Select region: us-east-1  
? Enter instance_count [2]: 3

┌─ Security Settings
│  SSL and authentication configuration  
└─
? Enable SSL/TLS encryption? (Y/n): y
? Enter cert_path: /path/to/cert.pem
? Select auth_method [basic]: oauth2

🚀 Starting workflow: deploy
```

## Next Steps

After completion, enables:
- Builtin workflow migration to new parameter format
- Comprehensive testing across all parameter features
- Documentation updates with examples