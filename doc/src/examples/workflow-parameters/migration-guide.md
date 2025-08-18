# Migration Guide: Legacy to Parameter System

This guide helps migrate existing workflows from ad-hoc parameter handling to the structured parameter system, providing step-by-step instructions and examples.

## Overview

The structured parameter system replaces legacy variable handling with:

- **Type Safety**: Parameters have defined types with validation
- **CLI Integration**: Automatic generation of CLI switches
- **Interactive Prompting**: User-friendly parameter input
- **Enhanced Help**: Auto-generated documentation
- **Backward Compatibility**: Legacy `--var` syntax still works

## Migration Benefits

### Before (Legacy System)
```bash
# Manual parameter documentation in markdown
# No type validation
# Basic CLI support with --var
sah flow run deploy --var app_name=myapp --var environment=prod --var port=8080
```

### After (Parameter System)
```bash
# Structured parameter definitions in frontmatter
# Type validation and error checking
# Rich CLI support with parameter-specific switches
sah flow run deploy --app-name myapp --environment prod --port 8080

# Interactive prompting
sah flow run deploy --interactive

# Comprehensive help
sah flow run deploy --help
```

## Step-by-Step Migration Process

### Step 1: Identify Current Parameters

First, analyze your existing workflow to identify all parameters:

**Legacy Workflow Example:**
```markdown
---
title: Web Application Deployment
---

# Web Application Deployment

Deploy a web application to various environments.

## Parameters
- `app_name`: Name of the application to deploy
- `environment`: Target environment (dev, staging, prod)  
- `port`: Port number for the application (default: 3000)
- `enable_ssl`: Enable SSL encryption (default: false)
- `ssl_cert_path`: Path to SSL certificate (required if enable_ssl=true)

## Usage
```bash
sah flow run web-deploy --var app_name=myapp --var environment=prod --var enable_ssl=true --var ssl_cert_path=/etc/ssl/cert.pem
```

### Step 2: Define Parameter Schema

Convert the parameter documentation into structured YAML frontmatter:

```yaml
---
title: Web Application Deployment
description: Deploy a web application to various environments
parameters:
  - name: app_name
    description: Name of the application to deploy
    type: string
    required: true
    validation:
      pattern: '^[a-z][a-z0-9-]*$'
      min_length: 3
      max_length: 30
      
  - name: environment
    description: Target environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  - name: port
    description: Port number for the application
    type: number
    default: 3000
    validation:
      min: 1024
      max: 65535
      
  - name: enable_ssl
    description: Enable SSL encryption
    type: boolean
    default: false
    
  - name: ssl_cert_path
    description: Path to SSL certificate
    type: string
    required: true
    condition: "enable_ssl == true"
    validation:
      pattern: '^.*\.(pem|crt)$'
---
```

### Step 3: Update Documentation

Replace the manual parameter documentation with auto-generated information:

**Before:**
```markdown
## Parameters
- `app_name`: Name of the application to deploy
- `environment`: Target environment (dev, staging, prod)  
- `port`: Port number for the application (default: 3000)

## Usage
```bash
sah flow run web-deploy --var app_name=myapp --var environment=prod
```

**After:**
```markdown
# Web Application Deployment

Deploy {{ app_name }} to {{ environment }} environment on port {{ port }}.

{% if enable_ssl %}
SSL is enabled using certificate: {{ ssl_cert_path }}
{% endif %}

Use `sah flow run web-deploy --help` to see all available parameters.

## Example Usage
```bash
# Using parameter switches
sah flow run web-deploy --app-name myapp --environment prod --port 8080

# Interactive mode
sah flow run web-deploy --interactive
```

### Step 4: Test Both Formats

Verify both legacy and new parameter formats work:

```bash
# New format (preferred)
sah flow run web-deploy --app-name myapp --environment prod --enable-ssl --ssl-cert-path /etc/ssl/cert.pem

# Legacy format (still works)  
sah flow run web-deploy --var app_name=myapp --var environment=prod --var enable_ssl=true --var ssl_cert_path=/etc/ssl/cert.pem

# Interactive mode (new feature)
sah flow run web-deploy --interactive
```

## Common Migration Patterns

### Pattern 1: Simple String Parameters

**Legacy:**
```markdown
## Parameters
- `service_name`: Name of the service
- `config_file`: Path to configuration file
```

**Migrated:**
```yaml
parameters:
  - name: service_name
    description: Name of the service
    type: string
    required: true
    validation:
      pattern: '^[a-z][a-z0-9-]*$'
      
  - name: config_file
    description: Path to configuration file
    type: string
    required: true
    validation:
      pattern: '^.*\.(yaml|json|toml)$'
      examples: ['config.yaml', 'settings.json']
```

### Pattern 2: Environment Choices

**Legacy:**
```markdown
## Parameters
- `environment`: Target environment (dev, test, staging, prod)
```

**Migrated:**
```yaml
parameters:
  - name: environment
    description: Target environment
    type: choice
    choices: [dev, test, staging, prod]
    required: true
```

### Pattern 3: Boolean Flags

**Legacy:**
```markdown
## Parameters  
- `enable_debug`: Enable debug mode (true/false, default: false)
- `skip_tests`: Skip running tests (true/false, default: false)
```

**Migrated:**
```yaml
parameters:
  - name: enable_debug
    description: Enable debug mode
    type: boolean
    default: false
    
  - name: skip_tests
    description: Skip running tests
    type: boolean
    default: false
```

### Pattern 4: Numeric Parameters with Ranges

**Legacy:**
```markdown
## Parameters
- `timeout`: Request timeout in seconds (default: 30)
- `retry_count`: Number of retries (default: 3, max: 10)
```

**Migrated:**
```yaml
parameters:
  - name: timeout
    description: Request timeout in seconds
    type: number
    default: 30
    validation:
      min: 1
      max: 300
      
  - name: retry_count
    description: Number of retries
    type: number
    default: 3
    validation:
      min: 0
      max: 10
```

### Pattern 5: Conditional Parameters

**Legacy:**
```markdown
## Parameters
- `deploy_type`: Deployment type (standard, custom)
- `custom_script`: Custom deployment script (required if deploy_type=custom)
```

**Migrated:**
```yaml
parameters:
  - name: deploy_type
    description: Deployment type
    type: choice
    choices: [standard, custom]
    required: true
    
  - name: custom_script
    description: Custom deployment script
    type: string
    required: true
    condition: "deploy_type == 'custom'"
    validation:
      pattern: '^.*\.(sh|py|js)$'
```

## Real-World Migration Examples

### Example 1: Database Migration Workflow

**Before Migration:**
```markdown
---
title: Database Migration
---

# Database Migration

Run database migrations with various options.

## Parameters
- `db_host`: Database hostname
- `db_port`: Database port (default: 5432)
- `db_name`: Database name  
- `migration_type`: Type of migration (up, down, reset)
- `dry_run`: Perform dry run without changes (default: false)
- `backup_first`: Create backup before migration (default: true for prod)

## Usage
```bash
sah flow run db-migrate --var db_host=localhost --var db_name=myapp --var migration_type=up
```

**After Migration:**
```yaml
---
title: Database Migration
description: Run database migrations with various options
parameters:
  - name: db_host
    description: Database hostname
    type: string
    required: true
    validation:
      pattern: '^[a-zA-Z0-9.-]+$'
      
  - name: db_port
    description: Database port
    type: number
    default: 5432
    validation:
      min: 1
      max: 65535
      
  - name: db_name
    description: Database name
    type: string
    required: true
    validation:
      pattern: '^[a-zA-Z][a-zA-Z0-9_]*$'
      
  - name: migration_type
    description: Type of migration to perform
    type: choice
    choices: [up, down, reset]
    required: true
    
  - name: dry_run
    description: Perform dry run without making changes
    type: boolean
    default: false
    
  - name: backup_first
    description: Create backup before migration
    type: boolean
    default: true
---

# Database Migration

Run {{ migration_type }} migration on {{ db_name }} at {{ db_host }}:{{ db_port }}.

{% if dry_run %}
**DRY RUN MODE** - No changes will be made.
{% endif %}

{% if backup_first %}
A backup will be created before applying migrations.
{% endif %}
```

**Usage Comparison:**
```bash
# Legacy format
sah flow run db-migrate --var db_host=prod.db.com --var db_name=myapp --var migration_type=up --var backup_first=true

# New format
sah flow run db-migrate --db-host prod.db.com --db-name myapp --migration-type up --backup-first

# Interactive mode (new capability)
sah flow run db-migrate --interactive
```

### Example 2: CI/CD Pipeline Workflow

**Before Migration:**
```markdown
---
title: CI/CD Pipeline
---

# CI/CD Pipeline

Continuous integration and deployment pipeline.

## Parameters
- `branch`: Git branch to build (default: main)
- `build_type`: Build type (debug, release)
- `run_tests`: Run test suite (default: true)
- `deploy_env`: Deployment environment (dev, staging, prod)
- `notify_slack`: Send Slack notification (default: false)
- `slack_webhook`: Slack webhook URL (required if notify_slack=true)

## Usage  
```bash
sah flow run ci-cd --var branch=feature/auth --var build_type=release --var deploy_env=staging
```

**After Migration:**
```yaml
---
title: CI/CD Pipeline
description: Continuous integration and deployment pipeline
parameter_groups:
  - name: build
    description: Build configuration
    parameters: [branch, build_type, run_tests]
    
  - name: deployment
    description: Deployment settings
    parameters: [deploy_env]
    
  - name: notifications
    description: Notification settings
    parameters: [notify_slack, slack_webhook]

parameters:
  # Build group
  - name: branch
    description: Git branch to build
    type: string
    default: main
    validation:
      pattern: '^[a-zA-Z0-9/_-]+$'
      
  - name: build_type
    description: Build configuration type
    type: choice
    choices: [debug, release]
    required: true
    
  - name: run_tests
    description: Run the test suite
    type: boolean
    default: true
    
  # Deployment group
  - name: deploy_env
    description: Target deployment environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  # Notifications group  
  - name: notify_slack
    description: Send Slack notification
    type: boolean
    default: false
    
  - name: slack_webhook
    description: Slack webhook URL for notifications
    type: string
    required: true
    condition: "notify_slack == true"
    validation:
      pattern: '^https://hooks\.slack\.com/.*'
      examples: ['https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX']
---

# CI/CD Pipeline

Build {{ branch }} branch ({{ build_type }}) and deploy to {{ deploy_env }}.

## Build Configuration
- Branch: {{ branch }}
- Type: {{ build_type }}
- Tests: {% if run_tests %}Enabled{% else %}Disabled{% endif %}

## Deployment
- Environment: {{ deploy_env }}

{% if notify_slack %}
## Notifications
Slack notifications will be sent to the configured webhook.
{% endif %}
```

## Migration Checklist

### Pre-Migration Assessment
- [ ] Identify all parameters used in the workflow
- [ ] Document parameter types and constraints  
- [ ] Note any interdependencies between parameters
- [ ] Review current CLI usage patterns
- [ ] Check if parameters are documented in multiple places

### Migration Implementation
- [ ] Add parameter definitions to frontmatter
- [ ] Set appropriate types for each parameter
- [ ] Add validation rules where needed
- [ ] Define conditional parameters if applicable
- [ ] Group related parameters
- [ ] Set sensible defaults
- [ ] Update workflow content to use parameter references

### Testing and Validation
- [ ] Test new CLI parameter switches
- [ ] Verify legacy `--var` syntax still works  
- [ ] Test interactive parameter prompting
- [ ] Validate parameter validation rules
- [ ] Check conditional parameter logic
- [ ] Test help text generation
- [ ] Verify all parameter combinations work

### Documentation Updates
- [ ] Remove manual parameter documentation
- [ ] Update usage examples
- [ ] Add migration notes if needed
- [ ] Update any external documentation
- [ ] Create team training materials

### Cleanup (Optional)
- [ ] Plan deprecation of legacy `--var` syntax
- [ ] Update CI/CD scripts to use new syntax
- [ ] Train team members on new parameter system
- [ ] Monitor usage and gather feedback

## Troubleshooting Migration Issues

### Issue: Parameter Not Found
```
Error: Required parameter 'app_name' is missing
```

**Solution:** Check parameter name matches exactly (case-sensitive):
```yaml
# Correct
- name: app_name

# Incorrect  
- name: App_Name
- name: appName
```

### Issue: Type Validation Fails
```
Error: Parameter 'port' expects number, got string
```

**Solution:** Ensure CLI usage matches parameter type:
```bash
# Correct
--port 8080

# Incorrect
--port "8080"  # String in some contexts
```

### Issue: Conditional Parameter Not Working
```
Error: Parameter 'ssl_cert_path' is required but not provided
```

**Solution:** Check condition syntax and logic:
```yaml
# Correct condition syntax
condition: "enable_ssl == true"

# Common mistakes
condition: enable_ssl == true     # Missing quotes
condition: "enable_ssl = true"    # Single equals
condition: "enable_ssl === true"  # Triple equals not supported
```

### Issue: Pattern Validation Errors
```
Error: Parameter 'email' does not match required pattern
```

**Solution:** Test patterns with example values:
```yaml
validation:
  pattern: '^[^@]+@[^@]+\.[^@]+$'  # Basic email pattern
  examples: ['user@example.com']   # Include examples for testing
```

## Best Practices for Migration

### 1. Incremental Migration
- Migrate one workflow at a time
- Keep both old and new documentation during transition
- Test thoroughly before removing legacy support

### 2. Maintain Backward Compatibility
- Keep legacy `--var` syntax working during migration period
- Provide clear migration timeline to users
- Include migration notes in workflow documentation

### 3. Improve During Migration
- Add validation that wasn't present before
- Organize parameters into logical groups
- Add helpful defaults and examples
- Improve parameter descriptions

### 4. User Communication
- Announce migration plans early
- Provide clear examples of new syntax
- Create migration tools if needed
- Gather feedback and iterate

The migration process transforms legacy workflows into robust, user-friendly parameter systems while maintaining compatibility and improving the overall user experience.