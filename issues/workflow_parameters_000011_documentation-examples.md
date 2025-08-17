# Documentation and Examples

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Create comprehensive documentation and examples for the workflow parameter system, including user guides, developer documentation, and examples that demonstrate all parameter features and best practices.

## Current State

- Parameter system implementation complete
- No comprehensive documentation for users or developers  
- No examples demonstrating advanced parameter features
- Users need guidance on parameter definition and usage patterns

## Implementation Tasks

### 1. User Documentation

Create user-facing documentation for workflow parameters:

#### Parameter Definition Guide
```markdown
# Workflow Parameter Guide

## Overview

Workflow parameters provide a structured way to define and validate inputs for workflows, offering:

- Type safety with validation
- CLI switch generation
- Interactive prompting  
- Conditional parameters
- Parameter groups

## Parameter Types

### String Parameters
```yaml
parameters:
  - name: username
    description: User account name
    type: string
    required: true
    validation:
      min_length: 3
      max_length: 20
      pattern: '^[a-zA-Z][a-zA-Z0-9_]*$'
```

### Number Parameters
```yaml
parameters:
  - name: port
    description: Server port number
    type: number
    required: false
    default: 8080
    validation:
      min: 1
      max: 65535
```

### Boolean Parameters  
```yaml
parameters:
  - name: enable_ssl
    description: Enable HTTPS encryption
    type: boolean
    required: false
    default: true
```

### Choice Parameters
```yaml
parameters:
  - name: environment
    description: Deployment environment
    type: choice
    required: true
    choices: [dev, staging, prod]
```

### Multi-Choice Parameters
```yaml
parameters:
  - name: features
    description: Features to enable
    type: multi_choice
    choices: [logging, metrics, caching, auth]
    validation:
      min_selections: 1
      max_selections: 3
```
```

#### CLI Usage Guide
```markdown
# Using Workflow Parameters

## CLI Switches

Parameters automatically generate CLI switches:

```bash
# Named parameter switches
sah flow run deploy --environment prod --port 8080 --enable-ssl

# Short forms (when available)
sah flow run deploy -e prod -p 8080 --enable-ssl
```

## Interactive Mode

Use interactive mode for guided parameter input:

```bash
sah flow run deploy --interactive
```

Example interaction:
```
┌─ Deployment Configuration
│  Environment and infrastructure settings
└─
? Select environment: 
  > dev
    staging  
    prod

? Enter port [8080]: 9000
? Enable SSL encryption? (Y/n): y
```

## Mixed Mode

Combine CLI switches with interactive prompting:

```bash
# Provide some parameters, prompt for missing ones
sah flow run deploy --environment prod --interactive
```

## Parameter Precedence

Parameters are resolved in this order:
1. CLI parameter switches (`--param-name`)
2. Legacy variable switches (`--var param_name=value`)  
3. Interactive prompting (if enabled and parameter missing)
4. Default values (if specified)
5. Error for required parameters with no value
```

### 2. Advanced Features Documentation

Document advanced parameter features:

#### Conditional Parameters
```markdown
# Conditional Parameters

Parameters can be conditionally required based on other parameter values:

```yaml
parameters:
  - name: deploy_type
    description: Deployment type
    type: choice
    choices: [standard, custom]
    required: true
    
  - name: custom_config
    description: Custom deployment configuration file
    type: string
    required: true
    condition: "deploy_type == 'custom'"
    pattern: '^.*\.yaml$'
```

## Condition Expressions

Supported operators:
- Equality: `deploy_type == 'custom'`
- Inequality: `port != 80`
- Comparisons: `min_replicas > 1`
- Logical: `ssl_enabled == true && cert_type == 'custom'`
- Membership: `environment in ['staging', 'prod']`
```

#### Parameter Groups
```markdown
# Parameter Groups

Organize related parameters for better UX:

```yaml
parameter_groups:
  - name: deployment
    description: Deployment configuration
    parameters: [environment, region, replicas]
    
  - name: security  
    description: Security settings
    parameters: [enable_ssl, cert_path, auth_method]

parameters:
  # Deployment group
  - name: environment
    # ... parameter definition
    
  # Security group  
  - name: enable_ssl
    # ... parameter definition
```

Groups appear organized in help text and interactive prompts.
```

### 3. Developer Documentation

Create documentation for developers extending the parameter system:

#### Parameter System Architecture
```markdown
# Parameter System Architecture

## Components

### Core Types
- `Parameter`: Individual parameter definition
- `ParameterType`: Type enumeration (String, Boolean, Number, etc.)
- `ValidationRules`: Validation constraints
- `ParameterGroup`: Parameter organization

### Validation System
- `ParameterValidator`: Core validation engine
- `ConditionEvaluator`: Conditional parameter logic
- `ValidationContext`: Context for enhanced error messages

### CLI Integration
- `ParameterResolver`: Resolve parameters from multiple sources
- `InteractivePrompts`: User prompting interface
- `CliHelpGenerator`: Auto-generated help text

## Adding New Parameter Types

To add a new parameter type:

1. Extend `ParameterType` enum
2. Add validation logic to `ParameterValidator`
3. Add CLI parsing in `ParameterResolver`
4. Add interactive prompting in `InteractivePrompts`
5. Update help text generation

Example:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterType {
    String,
    Boolean,
    Number,
    Choice,
    MultiChoice,
    DateTime, // New type
}
```
```

### 4. Example Workflows

Create comprehensive example workflows demonstrating all features:

#### Basic Example (basic-app-deploy.md)
```yaml
---
title: Basic Application Deployment
description: Deploy a simple web application
parameters:
  - name: app_name
    description: Application name
    type: string
    required: true
    pattern: '^[a-z][a-z0-9-]*$'
    
  - name: environment
    description: Target environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  - name: port
    description: Application port
    type: number
    default: 8080
    min: 1024
    max: 65535
---

# Basic Application Deployment

Deploy {{ app_name }} to {{ environment }} on port {{ port }}.

## Steps
1. Validate deployment environment
2. Build application image  
3. Deploy to {{ environment }}
4. Health check on port {{ port }}
```

#### Advanced Example (microservice-deploy.md)
```yaml
---
title: Microservice Deployment
description: Deploy microservice with advanced configuration
parameter_groups:
  - name: application
    description: Application configuration
    parameters: [service_name, version, replicas]
    
  - name: infrastructure
    description: Infrastructure settings
    parameters: [environment, region, instance_type]
    
  - name: security
    description: Security configuration
    parameters: [enable_ssl, cert_provider, auth_method]
    
  - name: monitoring
    description: Monitoring and observability
    parameters: [log_level, metrics_enabled, tracing_enabled]

parameters:
  # Application group
  - name: service_name
    description: Microservice name
    type: string
    required: true
    pattern: '^[a-z][a-z0-9-]*$'
    
  - name: version
    description: Service version
    type: string
    required: true
    pattern: '^\d+\.\d+\.\d+$'
    
  - name: replicas
    description: Number of service replicas
    type: number
    default: 2
    min: 1
    max: 10
    
  # Infrastructure group
  - name: environment
    description: Deployment environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  - name: region
    description: AWS region
    type: choice
    choices: [us-east-1, us-west-2, eu-west-1]
    required: true
    condition: "environment in ['staging', 'prod']"
    
  - name: instance_type
    description: EC2 instance type
    type: choice
    choices: [t3.small, t3.medium, t3.large, c5.large]
    default: t3.small
    
  # Security group
  - name: enable_ssl
    description: Enable SSL/TLS
    type: boolean
    default: true
    
  - name: cert_provider
    description: SSL certificate provider
    type: choice
    choices: [letsencrypt, aws-acm, custom]
    default: letsencrypt
    condition: "enable_ssl == true"
    
  - name: custom_cert_path
    description: Path to custom certificate
    type: string
    required: true
    condition: "cert_provider == 'custom'"
    pattern: '^.*\.(pem|crt)$'
    
  - name: auth_method
    description: Authentication method
    type: multi_choice
    choices: [basic, oauth2, api_key, jwt]
    min_selections: 1
    max_selections: 2
    
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
    
  - name: tracing_enabled
    description: Enable distributed tracing
    type: boolean
    default: false
    condition: "environment == 'prod'"
---

# Microservice Deployment

Deploy {{ service_name }} v{{ version }} to {{ environment }} with {{ replicas }} replicas.

## Configuration
- Environment: {{ environment }}
- Region: {{ region }}
- Instance Type: {{ instance_type }}
- SSL: {{ enable_ssl }}
- Auth: {{ auth_method }}
- Logging: {{ log_level }}
```

### 5. Migration Guide

Create guide for migrating existing workflows:

```markdown
# Migration Guide: Legacy to Parameter System

## Overview

This guide helps migrate existing workflows from ad-hoc parameter handling to the structured parameter system.

## Before and After

### Before (Legacy)
```markdown
# Deployment Workflow

## Parameters
- `app_name`: Application name
- `environment`: Target environment (dev, staging, prod)

## Usage
sah flow run deploy --var app_name=myapp --var environment=prod
```

### After (Structured Parameters)
```yaml
---
title: Deployment Workflow
parameters:
  - name: app_name
    description: Application name
    type: string
    required: true
  - name: environment
    description: Target environment
    type: choice
    choices: [dev, staging, prod]
    required: true
---
```

### Benefits
- CLI switches: `--app-name myapp --environment prod`
- Interactive prompting
- Parameter validation
- Better help text
- Type safety

## Migration Steps

1. **Extract Parameters**: Identify all variables used in your workflow
2. **Define Schema**: Create structured parameter definitions in frontmatter
3. **Add Validation**: Define types, choices, patterns as appropriate
4. **Test Migration**: Verify both old and new syntax work
5. **Update Documentation**: Update workflow documentation
6. **Deprecate Legacy**: Gradually migrate users to new syntax
```

## Technical Details

### Documentation Structure
```
doc/
├── user-guide/
│   ├── workflow-parameters.md
│   ├── parameter-types.md
│   ├── conditional-parameters.md
│   └── parameter-groups.md
├── examples/
│   ├── basic-workflows/
│   ├── advanced-workflows/
│   └── migration-examples/
├── developer-guide/
│   ├── parameter-system-architecture.md
│   ├── extending-parameters.md
│   └── testing-parameters.md
└── reference/
    ├── parameter-schema.md
    ├── validation-rules.md
    └── error-messages.md
```

### Integration with Existing Docs

- Update existing workflow documentation
- Add parameter examples to tutorial content
- Include parameter system in getting-started guide
- Reference from CLI help system

### Testing Requirements

- Documentation accuracy tests
- Example workflow execution tests  
- Link validation in documentation
- Code example syntax validation

## Success Criteria

- [ ] Comprehensive user documentation for all parameter features
- [ ] Developer guide for extending the parameter system
- [ ] Complete set of example workflows demonstrating features
- [ ] Migration guide for existing workflows
- [ ] Integration with existing documentation structure
- [ ] All examples are tested and working
- [ ] Documentation is clear and actionable

## Dependencies

- Requires completion of all previous workflow parameter implementation steps
- Foundation for user adoption and system usage

## Example Documentation Usage

```bash
# View parameter help
sah flow run deploy --help

# Interactive tutorial
sah flow run tutorial-parameters --interactive

# Example workflows
sah flow run example-basic-deploy --help
sah flow run example-microservice --interactive
```

## Next Steps

After completion, workflow parameters are:
- Fully documented with examples
- Ready for user adoption
- Maintainable by developers
- Extensible for future features