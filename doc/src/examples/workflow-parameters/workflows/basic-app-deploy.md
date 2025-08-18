---
title: Basic Application Deployment
description: Deploy a simple web application to various environments
parameters:
  - name: app_name
    description: Application name
    type: string
    required: true
    validation:
      pattern: '^[a-z][a-z0-9-]*$'
      min_length: 3
      max_length: 30
      examples: ['my-webapp', 'user-service', 'api-gateway']
    
  - name: environment
    description: Target deployment environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  - name: port
    description: Application port number
    type: number
    default: 8080
    validation:
      min: 1024
      max: 65535
      
  - name: enable_ssl
    description: Enable HTTPS encryption
    type: boolean
    default: false
    
  - name: replicas
    description: Number of application instances
    type: number
    default: 1
    validation:
      min: 1
      max: 10
---

# Basic Application Deployment

Deploy {{ app_name }} to {{ environment }} environment with {{ replicas }} instance(s) on port {{ port }}.

{% if enable_ssl %}
SSL encryption is **enabled** for secure connections.
{% else %}
SSL encryption is **disabled** - using HTTP connections.
{% endif %}

## States

### validate_config
**Description**: Validate deployment configuration and prerequisites

**Actions:**
- Verify application name format
- Check target environment availability  
- Validate port is not in use
- Confirm SSL certificate if SSL enabled

**Transitions:**
- Always → build_application

### build_application
**Description**: Build the application for deployment

**Actions:**
- Pull latest application code
- Build application for {{ environment }} environment
{% if environment == 'prod' %}
- Run production optimizations
- Generate production assets
{% endif %}

**Transitions:**
- On success → deploy
- On failure → cleanup

### deploy
**Description**: Deploy the application

**Actions:**
- Deploy {{ app_name }} to {{ environment }}
- Configure {{ replicas }} instance(s)
- Set up port {{ port }} binding
{% if enable_ssl %}
- Configure SSL/TLS encryption
- Set up HTTPS redirects
{% endif %}
- Start application services

**Transitions:**
- On success → health_check
- On failure → cleanup

### health_check
**Description**: Verify deployment is healthy

**Actions:**
- Wait for application startup (30 seconds)
{% if enable_ssl %}
- Test HTTPS endpoint on port {{ port }}
{% else %}
- Test HTTP endpoint on port {{ port }}
{% endif %}
- Verify {{ replicas }} instance(s) are running
- Check application responds correctly

**Transitions:**
- If health check passes → complete
- If health check fails → cleanup

### complete
**Description**: Deployment completed successfully

**Actions:**
- Log successful deployment details
- Update deployment status
{% if environment == 'prod' %}
- Send production deployment notification
{% endif %}

### cleanup
**Description**: Clean up failed deployment

**Actions:**
- Stop failed application instances
- Remove incomplete deployment artifacts
- Free up allocated resources
- Log failure details