# Workflow Parameters

Workflow parameters provide a structured way to define and validate inputs for workflows, offering type safety, CLI switch generation, interactive prompting, and advanced validation features.

## Overview

The workflow parameter system enables:

- **Type Safety**: Strong typing with validation for strings, numbers, booleans, choices, and multi-choice parameters
- **CLI Integration**: Automatic generation of CLI switches from parameter definitions
- **Interactive Prompting**: User-friendly prompts when parameters are missing
- **Validation Rules**: Pattern matching, ranges, string lengths, and custom validation
- **Conditional Parameters**: Parameters that are required based on other parameter values
- **Parameter Groups**: Organization of related parameters for better user experience
- **Error Recovery**: Enhanced error messages with suggestions and retry capabilities

## Basic Parameter Types

### String Parameters

String parameters accept text input with optional validation:

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

**Validation Options:**
- `min_length`: Minimum string length
- `max_length`: Maximum string length  
- `pattern`: Regular expression pattern for validation

### Boolean Parameters

Boolean parameters for true/false values:

```yaml
parameters:
  - name: enable_ssl
    description: Enable HTTPS encryption
    type: boolean
    required: false
    default: true
```

**CLI Usage:**
```bash
# Enable SSL
sah flow run deploy --enable-ssl

# Disable SSL
sah flow run deploy --no-enable-ssl
```

### Number Parameters

Numeric parameters with range validation:

```yaml
parameters:
  - name: port
    description: Server port number
    type: number
    required: false
    default: 8080
    validation:
      min: 1024
      max: 65535
```

**Validation Options:**
- `min`: Minimum allowed value
- `max`: Maximum allowed value

### Choice Parameters

Single selection from predefined options:

```yaml
parameters:
  - name: environment
    description: Deployment environment
    type: choice
    required: true
    choices: [dev, staging, prod]
```

**CLI Usage:**
```bash
sah flow run deploy --environment prod
```

### Multi-Choice Parameters

Multiple selections from predefined options:

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

**CLI Usage:**
```bash
sah flow run deploy --features logging,metrics,auth
```

## CLI Integration

### Automatic Switch Generation

Parameters automatically generate CLI switches:

```bash
# Named parameter switches
sah flow run deploy --environment prod --port 8080 --enable-ssl

# Short forms (when available)
sah flow run deploy -e prod -p 8080 --enable-ssl
```

### Interactive Mode

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

### Mixed Mode

Combine CLI switches with interactive prompting:

```bash
# Provide some parameters, prompt for missing ones
sah flow run deploy --environment prod --interactive
```

### Parameter Precedence

Parameters are resolved in this order:
1. CLI parameter switches (`--param-name`)
2. Legacy variable switches (`--var param_name=value`)  
3. Interactive prompting (if enabled and parameter missing)
4. Default values (if specified)
5. Error for required parameters with no value

## Advanced Features

### Conditional Parameters

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
    validation:
      pattern: '^.*\.yaml$'
```

**Condition Expressions:**

Supported operators:
- Equality: `deploy_type == 'custom'`
- Inequality: `port != 80`
- Comparisons: `min_replicas > 1`
- Logical: `ssl_enabled == true && cert_type == 'custom'`
- Membership: `environment in ['staging', 'prod']`

### Parameter Groups

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
    description: Target environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  # Security group  
  - name: enable_ssl
    description: Enable SSL/TLS encryption
    type: boolean
    default: true
```

Groups appear organized in help text and interactive prompts.

### Enhanced Error Handling

The parameter system provides enhanced error messages with:

- **Fuzzy Matching**: Suggestions for typos in choice parameters
- **Pattern Examples**: Show valid examples when pattern validation fails  
- **Recovery Options**: Allow users to retry with corrected input
- **Context Information**: Clear explanations of what went wrong

Example error with suggestions:
```
Error: Parameter 'environment' has invalid value: 'porduction'

Did you mean: 'production'?

Valid choices: dev, staging, production
```

## Validation Rules

### Pattern Validation

Use regular expressions to validate string formats:

```yaml
parameters:
  - name: email
    type: string
    validation:
      pattern: '^[^@]+@[^@]+\.[^@]+$'
      pattern_description: 'Valid email address'
      examples: ['user@example.com', 'admin@company.org']
```

### Range Validation

Validate numeric ranges:

```yaml
parameters:
  - name: timeout
    type: number
    validation:
      min: 1
      max: 3600
    description: 'Timeout in seconds (1-3600)'
```

### String Length Validation

Control string length:

```yaml
parameters:
  - name: description
    type: string
    validation:
      min_length: 10
      max_length: 500
    description: 'Description (10-500 characters)'
```

### Multi-Choice Validation

Control selection counts:

```yaml
parameters:
  - name: components
    type: multi_choice
    choices: [api, web, worker, scheduler]
    validation:
      min_selections: 1
      max_selections: 3
    description: 'Select 1-3 components to deploy'
```

## Help Generation

Parameters automatically generate comprehensive help text:

```bash
sah flow run deploy --help
```

Output:
```
Deploy Application

Usage: sah flow run deploy [OPTIONS]

Parameters:
  Deployment Configuration:
    --environment <ENVIRONMENT>    Target environment [choices: dev, staging, prod]
    --region <REGION>              AWS region [default: us-east-1]
    --replicas <REPLICAS>          Number of replicas [default: 2, range: 1-10]

  Security Settings:
    --enable-ssl                   Enable SSL/TLS encryption [default: true]
    --cert-path <PATH>             Path to SSL certificate [required if enable-ssl]
    --auth-method <METHOD>         Authentication method [choices: oauth2, api-key]

Options:
    --interactive                  Use interactive parameter prompting
    --help                        Show this help message
```

## Best Practices

### Parameter Design

1. **Use Descriptive Names**: Make parameter names self-explanatory
   ```yaml
   # Good
   - name: database_connection_timeout
   # Bad  
   - name: timeout
   ```

2. **Provide Clear Descriptions**: Explain what each parameter does
   ```yaml
   - name: port
     description: 'Server port number for HTTP connections'
   ```

3. **Set Sensible Defaults**: Reduce required user input
   ```yaml
   - name: log_level
     default: info
     choices: [debug, info, warn, error]
   ```

4. **Use Validation**: Prevent invalid input early
   ```yaml
   - name: email
     validation:
       pattern: '^[^@]+@[^@]+\.[^@]+$'
   ```

### Parameter Organization

1. **Group Related Parameters**: Use parameter groups for complex workflows
2. **Order by Importance**: Put required parameters first
3. **Minimize Required Parameters**: Make parameters optional when possible
4. **Use Conditional Parameters**: Only require parameters when relevant

### Error Handling

1. **Provide Examples**: Include valid examples in validation rules
2. **Use Clear Messages**: Write helpful error descriptions
3. **Enable Recovery**: Allow users to correct mistakes

## Template Integration

Parameters are available as Liquid template variables:

```liquid
# Deploy to {{ environment }} environment
Deploying application with {{ replicas }} replicas.

{% if enable_ssl %}
SSL is enabled using certificate: {{ cert_path }}
{% endif %}

{% for feature in features %}
- Enabling feature: {{ feature }}
{% endfor %}
```

## Migration from Legacy Format

### Before (Legacy Variables)

```markdown
---
title: Deploy Application
---

# Deploy Application

## Parameters
- `environment`: Target environment (dev, staging, prod)
- `port`: Server port number
- `enable_ssl`: Enable SSL encryption

## Usage
```bash
sah flow run deploy --var environment=prod --var port=8080 --var enable_ssl=true
```

### After (Structured Parameters)

```yaml
---
title: Deploy Application
parameters:
  - name: environment
    description: Target environment
    type: choice
    choices: [dev, staging, prod]
    required: true
  - name: port
    description: Server port number
    type: number
    default: 8080
    validation:
      min: 1024
      max: 65535
  - name: enable_ssl
    description: Enable SSL encryption
    type: boolean
    default: true
---
```

### Usage (Both Legacy and New Supported)

```bash
# New format (recommended)
sah flow run deploy --environment prod --port 8080 --enable-ssl

# Legacy format (still works)
sah flow run deploy --var environment=prod --var port=8080 --var enable_ssl=true

# Interactive mode (new feature)
sah flow run deploy --interactive
```

## Complete Example

Here's a comprehensive example showing all parameter features:

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
    parameters: [enable_ssl, cert_provider, auth_methods]

parameters:
  # Application group
  - name: service_name
    description: Microservice name
    type: string
    required: true
    validation:
      pattern: '^[a-z][a-z0-9-]*$'
      min_length: 3
      max_length: 50
    
  - name: version
    description: Service version
    type: string
    required: true
    validation:
      pattern: '^\d+\.\d+\.\d+$'
      examples: ['1.0.0', '2.1.5']
    
  - name: replicas
    description: Number of service replicas
    type: number
    default: 2
    validation:
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
    
  - name: auth_methods
    description: Authentication methods
    type: multi_choice
    choices: [basic, oauth2, api_key, jwt]
    validation:
      min_selections: 1
      max_selections: 2
---

# Microservice Deployment

Deploy {{ service_name }} v{{ version }} to {{ environment }}.

## Configuration
- Environment: {{ environment }}
- Replicas: {{ replicas }}
- SSL Enabled: {{ enable_ssl }}
{% if region %}
- Region: {{ region }}
{% endif %}

## Actions

### deploy
Deploy the microservice with the specified configuration.
```