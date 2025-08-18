---
title: Microservice Deployment
description: Deploy microservice with advanced configuration options
parameter_groups:
  - name: application
    description: Application configuration
    parameters: [service_name, version, replicas]
    
  - name: infrastructure
    description: Infrastructure settings
    parameters: [environment, region, instance_type]
    
  - name: security
    description: Security configuration
    parameters: [enable_ssl, cert_provider, custom_cert_path, auth_methods]
    
  - name: monitoring
    description: Monitoring and observability
    parameters: [log_level, metrics_enabled, tracing_enabled]

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
      examples: ['user-service', 'payment-api', 'notification-worker']
    
  - name: version
    description: Service version to deploy
    type: string
    required: true
    validation:
      pattern: '^\d+\.\d+\.\d+$'
      examples: ['1.0.0', '2.1.5', '0.3.2']
    
  - name: replicas
    description: Number of service replicas
    type: number
    default: 2
    validation:
      min: 1
      max: 20
    
  # Infrastructure group
  - name: environment
    description: Deployment environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  - name: region
    description: AWS region for deployment
    type: choice
    choices: [us-east-1, us-west-2, eu-west-1, ap-southeast-1]
    required: true
    condition: "environment in ['staging', 'prod']"
    
  - name: instance_type
    description: EC2 instance type
    type: choice
    choices: [t3.small, t3.medium, t3.large, c5.large, c5.xlarge, m5.large]
    default: t3.medium
    condition: "environment in ['staging', 'prod']"
    
  # Security group
  - name: enable_ssl
    description: Enable SSL/TLS encryption
    type: boolean
    default: true
    condition: "environment in ['staging', 'prod']"
    
  - name: cert_provider
    description: SSL certificate provider
    type: choice
    choices: [letsencrypt, aws-acm, custom]
    default: letsencrypt
    condition: "enable_ssl == true"
    
  - name: custom_cert_path
    description: Path to custom SSL certificate
    type: string
    required: true
    condition: "cert_provider == 'custom'"
    validation:
      pattern: '^.*\.(pem|crt|p12)$'
      examples: ['/etc/ssl/certs/service.pem', '/certs/wildcard.crt']
    
  - name: auth_methods
    description: Authentication methods to enable
    type: multi_choice
    choices: [basic, oauth2, api_key, jwt, mtls]
    validation:
      min_selections: 1
      max_selections: 3
    
  # Monitoring group
  - name: log_level
    description: Application log level
    type: choice
    choices: [trace, debug, info, warn, error]
    default: info
    
  - name: metrics_enabled
    description: Enable metrics collection
    type: boolean
    default: true
    condition: "environment in ['staging', 'prod']"
    
  - name: tracing_enabled
    description: Enable distributed tracing
    type: boolean
    default: false
    condition: "environment == 'prod'"
---

# Microservice Deployment

Deploy {{ service_name }} v{{ version }} to {{ environment }} with {{ replicas }} replica(s).

## Configuration Summary

### Application
- **Service**: {{ service_name }}
- **Version**: {{ version }}
- **Replicas**: {{ replicas }}
- **Environment**: {{ environment }}

{% if region %}
### Infrastructure  
- **Region**: {{ region }}
- **Instance Type**: {{ instance_type }}
{% endif %}

{% if enable_ssl %}
### Security
- **SSL Enabled**: Yes
- **Certificate Provider**: {{ cert_provider }}
{% if custom_cert_path %}
- **Certificate Path**: {{ custom_cert_path }}
{% endif %}
- **Authentication Methods**: 
{% for method in auth_methods %}
  - {{ method | upcase }}
{% endfor %}
{% endif %}

### Monitoring
- **Log Level**: {{ log_level | upcase }}
{% if metrics_enabled %}
- **Metrics**: Enabled
{% endif %}
{% if tracing_enabled %}
- **Tracing**: Enabled
{% endif %}

## States

### validate_deployment
**Description**: Validate deployment configuration and prerequisites

**Actions:**
- Verify service name and version format
- Check environment prerequisites
- Validate authentication method compatibility
{% if region %}
- Confirm {{ region }} region availability
{% endif %}
{% if custom_cert_path %}
- Validate custom SSL certificate at {{ custom_cert_path }}
{% endif %}

**Transitions:**
- Always → setup_infrastructure

### setup_infrastructure
**Description**: Provision infrastructure resources

**Actions:**
{% if environment != 'dev' %}
- Provision {{ instance_type }} instances in {{ region }}
- Set up load balancer configuration
- Configure auto-scaling groups (min: {{ replicas }}, max: {{ replicas * 3 }})
{% else %}
- Set up development environment containers
{% endif %}
- Create service discovery configuration
- Set up network security groups

**Transitions:**
- On success → build_service
- On failure → cleanup_infrastructure

### build_service
**Description**: Build and prepare service for deployment

**Actions:**
- Build {{ service_name }} v{{ version }}
- Run unit tests and security scans
{% if environment == 'prod' %}
- Generate production optimized build
- Create release artifacts
{% endif %}
- Package service for deployment

**Transitions:**
- On success → deploy_service
- On failure → cleanup_infrastructure

### deploy_service
**Description**: Deploy the microservice

**Actions:**
- Deploy {{ service_name }} with {{ replicas }} replica(s)
{% if enable_ssl %}
- Configure SSL/TLS with {{ cert_provider }} certificates
{% if cert_provider == 'letsencrypt' %}
- Obtain Let's Encrypt certificates
{% elsif cert_provider == 'aws-acm' %}
- Configure AWS Certificate Manager
{% elsif cert_provider == 'custom' %}
- Install custom certificate from {{ custom_cert_path }}
{% endif %}
{% endif %}
- Configure authentication methods:
{% for method in auth_methods %}
  - {{ method | upcase }} authentication
{% endfor %}
- Set log level to {{ log_level }}

**Transitions:**
- On success → configure_monitoring
- On failure → rollback_deployment

### configure_monitoring
**Description**: Set up monitoring and observability

**Actions:**
{% if metrics_enabled %}
- Configure Prometheus metrics collection
- Set up custom service metrics
- Create Grafana dashboards
{% endif %}
{% if tracing_enabled %}
- Configure distributed tracing with Jaeger
- Set up trace sampling configuration
- Create tracing dashboards
{% endif %}
- Configure log aggregation with {{ log_level }} level
- Set up health check endpoints

**Transitions:**
- Always → health_verification

### health_verification
**Description**: Verify service health and functionality

**Actions:**
- Wait for service startup (60 seconds)
- Check all {{ replicas }} replica(s) are healthy
- Test service endpoints
{% for method in auth_methods %}
- Verify {{ method | upcase }} authentication
{% endfor %}
{% if enable_ssl %}
- Validate SSL certificate and encryption
{% endif %}
{% if metrics_enabled %}
- Verify metrics collection is working
{% endif %}
- Run smoke tests for critical functionality

**Transitions:**
- If all checks pass → deployment_complete
- If any check fails → rollback_deployment

### deployment_complete
**Description**: Finalize successful deployment

**Actions:**
- Update service registry with new version
- Update load balancer to route traffic
{% if environment == 'prod' %}
- Send production deployment notifications
- Update deployment tracking systems
{% endif %}
- Create deployment summary report
- Clean up old service versions

### rollback_deployment
**Description**: Rollback failed deployment

**Actions:**
- Stop new service instances
- Restore previous service version if available
- Revert load balancer configuration
- Update service registry
- Log rollback details and failure reasons

**Transitions:**
- Always → cleanup_infrastructure

### cleanup_infrastructure
**Description**: Clean up failed deployment resources

**Actions:**
- Terminate failed service instances
- Clean up unused infrastructure resources
- Remove incomplete configurations
- Log cleanup actions and failure details