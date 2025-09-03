# Basic Parameter Examples

This guide demonstrates basic workflow parameter usage with simple, practical examples.

## Example 1: Simple Web Application Deployment

A basic deployment workflow with essential parameters:

```yaml
---
title: Simple Web App Deploy
description: Deploy a web application to a target environment
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
    description: Target deployment environment
    type: choice
    choices: [development, staging, production]
    required: true
    
  - name: port
    description: Port number for the application
    type: number
    default: 3000
    validation:
      min: 1024
      max: 65535
      
  - name: enable_monitoring
    description: Enable application monitoring
    type: boolean
    default: true
---

# Simple Web App Deploy

Deploy {{ app_name }} to {{ environment }} environment on port {{ port }}.

## States

### validate
**Description**: Validate deployment parameters and environment

**Actions:**
- Validate app name format
- Check environment availability
- Verify port is not in use

**Transitions:**
- Always → deploy

### deploy
**Description**: Deploy the application

**Actions:**
- Build application image
- Deploy to {{ environment }}
- Configure port {{ port }}
{% if enable_monitoring %}
- Enable monitoring and health checks
{% endif %}

**Transitions:**
- On success → verify
- On failure → cleanup

### verify
**Description**: Verify deployment success

**Actions:**
- Health check on port {{ port }}
- Verify application responds
{% if enable_monitoring %}
- Check monitoring endpoints
{% endif %}

**Transitions:**
- Always → complete

### complete
**Description**: Deployment completed successfully

### cleanup
**Description**: Clean up failed deployment

**Actions:**
- Remove failed deployment artifacts
- Free up allocated resources
```

**Usage Examples:**
```bash
# Interactive mode
sah flow run simple-deploy --interactive

# CLI parameters
sah flow run simple-deploy --app-name my-webapp --environment staging --port 8080

# With defaults
sah flow run simple-deploy --app-name my-webapp --environment development
```

## Example 2: Database Backup Workflow

A workflow for backing up databases with different strategies:

```yaml
---
title: Database Backup
description: Backup database with configurable options
parameters:
  - name: database_name
    description: Name of database to backup
    type: string
    required: true
    validation:
      pattern: '^[a-zA-Z][a-zA-Z0-9_]*$'
      
  - name: backup_type
    description: Type of backup to perform
    type: choice
    choices: [full, incremental, differential]
    default: full
    
  - name: compression_level
    description: Compression level (0-9, 0=none, 9=max)
    type: number
    default: 6
    validation:
      min: 0
      max: 9
      
  - name: encrypt_backup
    description: Encrypt the backup file
    type: boolean
    default: false
    
  - name: retention_days
    description: Number of days to retain backup
    type: number
    default: 30
    validation:
      min: 1
      max: 365
---

# Database Backup

Backup {{ database_name }} using {{ backup_type }} backup strategy.

## States

### prepare
**Description**: Prepare backup environment

**Actions:**
- Check database connectivity
- Verify backup storage space
- Create backup directory with timestamp

**Transitions:**
- Always → backup

### backup
**Description**: Perform database backup

**Actions:**
- Execute {{ backup_type }} backup of {{ database_name }}
- Apply compression level {{ compression_level }}
{% if encrypt_backup %}
- Encrypt backup with configured key
{% endif %}

**Transitions:**
- On success → verify
- On failure → cleanup

### verify
**Description**: Verify backup integrity

**Actions:**
- Check backup file size
- Verify backup can be read
{% if encrypt_backup %}
- Test decryption process
{% endif %}

**Transitions:**
- If verification passes → cleanup_old
- If verification fails → retry_backup

### cleanup_old
**Description**: Clean up old backups

**Actions:**
- Find backups older than {{ retention_days }} days
- Remove expired backups
- Update backup catalog

**Transitions:**
- Always → complete

### complete
**Description**: Backup completed successfully

### retry_backup
**Description**: Retry backup with different settings

**Actions:**
- Log backup failure details
- Adjust backup parameters if needed
- Retry backup operation

**Transitions:**
- On success → verify
- On failure (max retries) → failed

### cleanup
**Description**: Clean up failed backup

### failed
**Description**: Backup failed after retries
```

**Usage Examples:**
```bash
# Full encrypted backup
sah flow run db-backup --database-name myapp_db --encrypt-backup

# Quick incremental backup
sah flow run db-backup --database-name myapp_db --backup-type incremental --compression-level 3

# Long-term archive
sah flow run db-backup --database-name myapp_db --retention-days 365 --compression-level 9
```

## Example 3: Code Quality Check

A workflow for running code quality checks with configurable options:

```yaml
---
title: Code Quality Check
description: Run comprehensive code quality checks
parameters:
  - name: project_type
    description: Type of project to analyze
    type: choice
    choices: [javascript, typescript, python, rust, java]
    required: true
    
  - name: check_types
    description: Types of checks to run
    type: multi_choice
    choices: [linting, formatting, testing, security, performance]
    validation:
      min_selections: 1
      max_selections: 5
      
  - name: fail_on_warnings
    description: Fail the workflow if warnings are found
    type: boolean
    default: false
    
  - name: output_format
    description: Format for check results
    type: choice
    choices: [console, json, xml, html]
    default: console
    
  - name: max_issues
    description: Maximum allowed issues before failing
    type: number
    default: 0
    validation:
      min: 0
      max: 1000
---

# Code Quality Check

Run quality checks for {{ project_type }} project.

## Checks Configuration
{% for check in check_types %}
- {{ check | capitalize }}
{% endfor %}

Output format: {{ output_format }}
{% if fail_on_warnings %}
Failing on warnings: Yes
{% else %}
Failing on warnings: No
{% endif %}
Maximum allowed issues: {{ max_issues }}

## States

### setup
**Description**: Set up quality check environment

**Actions:**
- Detect project structure
- Install/update quality tools for {{ project_type }}
- Configure check parameters

**Transitions:**
- Always → run_checks

### run_checks
**Description**: Execute quality checks

**Actions:**
{% if check_types contains 'linting' %}
- Run linting checks
{% endif %}
{% if check_types contains 'formatting' %}
- Check code formatting
{% endif %}
{% if check_types contains 'testing' %}
- Run test suite with coverage
{% endif %}
{% if check_types contains 'security' %}
- Perform security vulnerability scan
{% endif %}
{% if check_types contains 'performance' %}
- Run performance analysis
{% endif %}

**Transitions:**
- Always → analyze_results

### analyze_results
**Description**: Analyze check results

**Actions:**
- Collect results from all checks
- Count issues by severity
- Generate report in {{ output_format }} format

**Transitions:**
- If issue count <= {{ max_issues }} → success
{% if fail_on_warnings %}
- If warnings found → failed
{% endif %}
- Otherwise → failed

### success
**Description**: All quality checks passed

**Actions:**
- Generate success report
- Archive check results

### failed
**Description**: Quality checks failed

**Actions:**
- Generate detailed failure report
- List all issues found
- Suggest fixes where possible
```

**Usage Examples:**
```bash
# Basic linting for JavaScript project
sah flow run quality-check --project-type javascript --check-types linting

# Comprehensive checks for production
sah flow run quality-check --project-type typescript --check-types linting,formatting,testing,security --fail-on-warnings --output-format html

# Quick development checks
sah flow run quality-check --project-type python --check-types linting,testing --max-issues 5
```

## Parameter Type Examples

### String Parameters with Validation

```yaml
# Email address
- name: email
  type: string
  validation:
    pattern: '^[^@]+@[^@]+\.[^@]+$'
    examples: ['user@example.com', 'admin@company.org']

# Semantic version
- name: version
  type: string
  validation:
    pattern: '^\d+\.\d+\.\d+$'
    examples: ['1.0.0', '2.1.5', '0.1.0']

# File path
- name: config_path
  type: string
  validation:
    pattern: '^.*\.(yaml|yml|json)$'
    examples: ['config.yaml', 'settings.json']
```

### Number Parameters with Ranges

```yaml
# Percentage
- name: cpu_usage
  type: number
  validation:
    min: 0
    max: 100
  description: 'CPU usage percentage (0-100)'

# Port number
- name: port
  type: number
  validation:
    min: 1024
    max: 65535
  description: 'Network port (1024-65535)'

# Memory in MB
- name: memory_mb
  type: number
  validation:
    min: 128
    max: 32768
  default: 512
```

### Boolean Parameters

```yaml
# Feature flags
- name: enable_logging
  type: boolean
  default: true
  description: 'Enable application logging'

- name: debug_mode
  type: boolean
  default: false
  description: 'Enable debug output'

- name: auto_restart
  type: boolean
  default: true
  description: 'Automatically restart on failure'
```

### Choice Parameters

```yaml
# Log levels
- name: log_level
  type: choice
  choices: [trace, debug, info, warn, error]
  default: info

# Cloud providers
- name: cloud_provider
  type: choice
  choices: [aws, azure, gcp, digitalocean]
  required: true

# Operating systems
- name: target_os
  type: choice
  choices: [linux, windows, macos]
  default: linux
```

### Multi-Choice Parameters

```yaml
# Build targets
- name: build_targets
  type: multi_choice
  choices: [x86_64, arm64, armv7]
  validation:
    min_selections: 1
    max_selections: 3

# Features to enable
- name: features
  type: multi_choice
  choices: [auth, logging, metrics, caching, compression]
  description: 'Features to enable in the build'

# Test suites to run
- name: test_suites
  type: multi_choice
  choices: [unit, integration, e2e, performance, security]
  validation:
    min_selections: 1
```

## CLI Usage Patterns

### Basic Usage
```bash
# Required parameters only
sah flow run my-workflow --required-param value

# With optional parameters
sah flow run my-workflow --required-param value --optional-param value

# Using defaults
sah flow run my-workflow --required-param value
```

### Interactive Mode
```bash
# Full interactive mode
sah flow run my-workflow --interactive

# Mixed mode (some CLI, some interactive)
sah flow run my-workflow --required-param value --interactive
```

### Boolean Parameters
```bash
# Enable boolean flag
sah flow run my-workflow --enable-feature

# Disable boolean flag  
sah flow run my-workflow --no-enable-feature

# Explicit boolean value
sah flow run my-workflow --enable-feature=true
```

### Multi-Choice Parameters
```bash
# Multiple values
sah flow run my-workflow --features logging,metrics,auth

# Single value
sah flow run my-workflow --features logging
```

### Help and Discovery
```bash
# Show workflow help
sah flow run my-workflow --help

# List all workflows
sah flow list

# Show workflow details
sah flow show my-workflow
```

These basic examples provide a foundation for understanding workflow parameters. For more advanced features like conditional parameters and parameter groups, see the [Advanced Features Guide](advanced-features.md).