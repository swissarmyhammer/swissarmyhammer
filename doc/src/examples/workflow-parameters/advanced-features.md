# Advanced Parameter Features

This guide demonstrates advanced workflow parameter features including conditional parameters, parameter groups, and complex validation scenarios.

## Conditional Parameters

Conditional parameters are only required when certain conditions are met, reducing complexity for users by only showing relevant options.

### Example 1: Cloud Deployment with Provider-Specific Options

```yaml
---
title: Multi-Cloud Deployment
description: Deploy to different cloud providers with provider-specific configuration
parameter_groups:
  - name: general
    description: General deployment settings
    parameters: [app_name, environment, cloud_provider]
    
  - name: aws_config
    description: AWS-specific configuration
    parameters: [aws_region, aws_instance_type, aws_vpc_id]
    
  - name: azure_config
    description: Azure-specific configuration  
    parameters: [azure_location, azure_vm_size, azure_resource_group]
    
  - name: ssl_config
    description: SSL/TLS configuration
    parameters: [enable_ssl, ssl_cert_provider, custom_cert_path]

parameters:
  # General parameters
  - name: app_name
    description: Application name
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
    
  - name: cloud_provider
    description: Cloud provider for deployment
    type: choice
    choices: [aws, azure, gcp]
    required: true

  # AWS-specific parameters (only required when cloud_provider == 'aws')
  - name: aws_region
    description: AWS region for deployment
    type: choice
    choices: [us-east-1, us-west-2, eu-west-1, ap-southeast-1]
    required: true
    condition: "cloud_provider == 'aws'"
    
  - name: aws_instance_type
    description: EC2 instance type
    type: choice
    choices: [t3.small, t3.medium, t3.large, c5.large, c5.xlarge]
    default: t3.medium
    condition: "cloud_provider == 'aws'"
    
  - name: aws_vpc_id
    description: VPC ID for deployment
    type: string
    required: true
    condition: "cloud_provider == 'aws' && environment == 'prod'"
    validation:
      pattern: '^vpc-[a-z0-9]{8,17}$'
      examples: ['vpc-12345678', 'vpc-abcd1234efgh5678']

  # Azure-specific parameters (only required when cloud_provider == 'azure')  
  - name: azure_location
    description: Azure location for deployment
    type: choice
    choices: [eastus, westus2, westeurope, southeastasia]
    required: true
    condition: "cloud_provider == 'azure'"
    
  - name: azure_vm_size
    description: Azure VM size
    type: choice
    choices: [Standard_B2s, Standard_D2s_v3, Standard_D4s_v3, Standard_F4s_v2]
    default: Standard_B2s
    condition: "cloud_provider == 'azure'"
    
  - name: azure_resource_group
    description: Azure resource group name
    type: string
    required: true
    condition: "cloud_provider == 'azure'"
    validation:
      pattern: '^[a-zA-Z][a-zA-Z0-9-_]*$'

  # SSL configuration (conditional on environment and requirements)
  - name: enable_ssl
    description: Enable SSL/TLS encryption
    type: boolean
    default: true
    condition: "environment in ['staging', 'prod']"
    
  - name: ssl_cert_provider
    description: SSL certificate provider
    type: choice
    choices: [letsencrypt, aws-acm, azure-keyvault, custom]
    default: letsencrypt
    condition: "enable_ssl == true"
    
  - name: custom_cert_path
    description: Path to custom SSL certificate
    type: string
    required: true
    condition: "ssl_cert_provider == 'custom'"
    validation:
      pattern: '^.*\.(pem|crt|p12)$'
      examples: ['/path/to/cert.pem', '/certs/wildcard.crt']
---

# Multi-Cloud Deployment

Deploy {{ app_name }} to {{ cloud_provider }} in {{ environment }} environment.

{% if cloud_provider == 'aws' %}
## AWS Configuration
- Region: {{ aws_region }}
- Instance Type: {{ aws_instance_type }}
{% if aws_vpc_id %}
- VPC: {{ aws_vpc_id }}
{% endif %}

{% elsif cloud_provider == 'azure' %}
## Azure Configuration
- Location: {{ azure_location }}
- VM Size: {{ azure_vm_size }}
- Resource Group: {{ azure_resource_group }}
{% endif %}

{% if enable_ssl %}
## SSL Configuration
- Provider: {{ ssl_cert_provider }}
{% if custom_cert_path %}
- Certificate Path: {{ custom_cert_path }}
{% endif %}
{% endif %}

## States

### validate_config
**Description**: Validate cloud provider and environment configuration

### deploy_infrastructure
**Description**: Deploy cloud infrastructure

**Actions:**
{% if cloud_provider == 'aws' %}
- Deploy to AWS region {{ aws_region }}
- Create EC2 instances of type {{ aws_instance_type }}
{% if aws_vpc_id %}
- Deploy in VPC {{ aws_vpc_id }}
{% endif %}

{% elsif cloud_provider == 'azure' %}
- Deploy to Azure location {{ azure_location }}
- Create VMs of size {{ azure_vm_size }}
- Use resource group {{ azure_resource_group }}
{% endif %}

### configure_ssl
**Description**: Configure SSL if enabled

**Actions:**
{% if enable_ssl %}
- Configure SSL using {{ ssl_cert_provider }}
{% if ssl_cert_provider == 'custom' %}
- Install custom certificate from {{ custom_cert_path }}
{% endif %}
{% endif %}

### complete
**Description**: Deployment completed
```

**Usage Examples:**

```bash
# AWS deployment
sah flow run multi-cloud-deploy \
  --app-name myapp \
  --environment prod \
  --cloud-provider aws \
  --aws-region us-east-1 \
  --aws-instance-type c5.large \
  --aws-vpc-id vpc-12345678

# Azure deployment (interactive for provider-specific options)
sah flow run multi-cloud-deploy \
  --app-name myapp \
  --environment staging \
  --cloud-provider azure \
  --interactive

# Development deployment (minimal SSL requirements)
sah flow run multi-cloud-deploy \
  --app-name myapp \
  --environment dev \
  --cloud-provider aws \
  --aws-region us-west-2
```

### Example 2: Complex Application Configuration

```yaml
---
title: Application Configuration
description: Configure application with feature-dependent settings
parameter_groups:
  - name: basic
    description: Basic application settings
    parameters: [app_name, app_type, enable_database]
    
  - name: database
    description: Database configuration
    parameters: [db_type, db_host, db_port, enable_ssl_db, ssl_cert_db]
    
  - name: caching
    description: Caching configuration
    parameters: [enable_cache, cache_type, redis_host, memcached_servers]
    
  - name: monitoring
    description: Monitoring and logging
    parameters: [enable_monitoring, monitoring_provider, custom_metrics_endpoint]

parameters:
  # Basic settings
  - name: app_name
    description: Application name
    type: string
    required: true
    
  - name: app_type
    description: Type of application
    type: choice
    choices: [web, api, worker, microservice]
    required: true
    
  - name: enable_database
    description: Enable database connectivity
    type: boolean
    default: true
    condition: "app_type in ['web', 'api', 'microservice']"

  # Database configuration (only if database is enabled)
  - name: db_type
    description: Database type
    type: choice
    choices: [postgresql, mysql, mongodb, redis]
    required: true
    condition: "enable_database == true"
    
  - name: db_host
    description: Database host
    type: string
    required: true
    condition: "enable_database == true"
    validation:
      pattern: '^[a-zA-Z0-9.-]+$'
      
  - name: db_port
    description: Database port
    type: number
    condition: "enable_database == true"
    # Default ports based on database type
    # This would be handled in the workflow logic
    
  - name: enable_ssl_db
    description: Enable SSL for database connections
    type: boolean
    default: true
    condition: "enable_database == true && db_type in ['postgresql', 'mysql']"
    
  - name: ssl_cert_db
    description: Database SSL certificate path
    type: string
    condition: "enable_ssl_db == true"

  # Caching configuration (for web and api applications)
  - name: enable_cache
    description: Enable application caching
    type: boolean
    default: false
    condition: "app_type in ['web', 'api']"
    
  - name: cache_type
    description: Caching backend
    type: choice
    choices: [redis, memcached, in-memory]
    default: redis
    condition: "enable_cache == true"
    
  - name: redis_host
    description: Redis server host
    type: string
    required: true
    condition: "cache_type == 'redis'"
    
  - name: memcached_servers
    description: Memcached server list (comma-separated)
    type: string
    required: true
    condition: "cache_type == 'memcached'"
    validation:
      pattern: '^[a-zA-Z0-9.-:, ]+$'
      examples: ['server1:11211,server2:11211', 'cache.example.com:11211']

  # Monitoring (for production-like environments)
  - name: enable_monitoring
    description: Enable application monitoring
    type: boolean
    default: true
    
  - name: monitoring_provider
    description: Monitoring service provider
    type: choice
    choices: [prometheus, datadog, newrelic, custom]
    default: prometheus
    condition: "enable_monitoring == true"
    
  - name: custom_metrics_endpoint
    description: Custom metrics endpoint URL
    type: string
    required: true
    condition: "monitoring_provider == 'custom'"
    validation:
      pattern: '^https?://.*'
      examples: ['https://metrics.company.com/collect', 'http://localhost:9090/metrics']
---

# Application Configuration

Configure {{ app_name }} ({{ app_type }}) with advanced features.

## Configuration Summary

### Application
- Name: {{ app_name }}
- Type: {{ app_type }}

{% if enable_database %}
### Database
- Type: {{ db_type }}
- Host: {{ db_host }}
{% if db_port %}
- Port: {{ db_port }}
{% endif %}
{% if enable_ssl_db %}
- SSL: Enabled
{% endif %}
{% endif %}

{% if enable_cache %}
### Caching
- Type: {{ cache_type }}
{% if cache_type == 'redis' %}
- Redis Host: {{ redis_host }}
{% elsif cache_type == 'memcached' %}
- Memcached Servers: {{ memcached_servers }}
{% endif %}
{% endif %}

{% if enable_monitoring %}
### Monitoring
- Provider: {{ monitoring_provider }}
{% if monitoring_provider == 'custom' %}
- Endpoint: {{ custom_metrics_endpoint }}
{% endif %}
{% endif %}
```

## Parameter Groups with Complex Relationships

Parameter groups help organize related parameters and improve the user experience, especially with conditional parameters.

### Example 3: Kubernetes Deployment with Multiple Feature Groups

```yaml
---
title: Kubernetes Application Deployment
description: Deploy application to Kubernetes with comprehensive configuration options
parameter_groups:
  - name: application
    description: Core application settings
    parameters: [app_name, image_tag, replicas, environment]
    
  - name: networking
    description: Network and ingress configuration
    parameters: [expose_service, service_type, ingress_enabled, ingress_host, tls_enabled]
    
  - name: storage
    description: Persistent storage configuration
    parameters: [needs_storage, storage_type, storage_size, storage_class]
    
  - name: security
    description: Security and access control
    parameters: [create_service_account, rbac_enabled, pod_security_policy, image_pull_secret]
    
  - name: monitoring
    description: Monitoring and observability
    parameters: [enable_prometheus, enable_jaeger, custom_annotations]
    
  - name: autoscaling
    description: Horizontal Pod Autoscaling
    parameters: [enable_hpa, min_replicas, max_replicas, target_cpu, target_memory]

parameters:
  # Application core
  - name: app_name
    description: Kubernetes application name
    type: string
    required: true
    validation:
      pattern: '^[a-z][a-z0-9-]*$'
      max_length: 63
      
  - name: image_tag
    description: Docker image tag to deploy
    type: string
    required: true
    validation:
      pattern: '^[a-zA-Z0-9._-]+:[a-zA-Z0-9._-]+$'
      examples: ['myapp:latest', 'registry.com/myapp:1.2.3']
      
  - name: replicas
    description: Number of pod replicas
    type: number
    default: 3
    validation:
      min: 1
      max: 100
      
  - name: environment
    description: Deployment environment
    type: choice
    choices: [development, staging, production]
    required: true

  # Networking group
  - name: expose_service
    description: Expose application via Kubernetes service
    type: boolean
    default: true
    
  - name: service_type
    description: Kubernetes service type
    type: choice
    choices: [ClusterIP, NodePort, LoadBalancer]
    default: ClusterIP
    condition: "expose_service == true"
    
  - name: ingress_enabled
    description: Create ingress resource
    type: boolean
    default: false
    condition: "expose_service == true"
    
  - name: ingress_host
    description: Ingress hostname
    type: string
    required: true
    condition: "ingress_enabled == true"
    validation:
      pattern: '^[a-z0-9.-]+$'
      examples: ['app.example.com', 'api-staging.company.com']
      
  - name: tls_enabled
    description: Enable TLS termination at ingress
    type: boolean
    default: true
    condition: "ingress_enabled == true"

  # Storage group
  - name: needs_storage
    description: Application requires persistent storage
    type: boolean
    default: false
    
  - name: storage_type
    description: Type of persistent storage
    type: choice
    choices: [hostPath, nfs, aws-ebs, gcp-disk, azure-disk]
    default: hostPath
    condition: "needs_storage == true"
    
  - name: storage_size
    description: Storage volume size
    type: string
    default: "10Gi"
    condition: "needs_storage == true"
    validation:
      pattern: '^\d+(Mi|Gi|Ti)$'
      examples: ['1Gi', '100Mi', '5Ti']
      
  - name: storage_class
    description: Kubernetes storage class
    type: string
    condition: "needs_storage == true && storage_type in ['aws-ebs', 'gcp-disk', 'azure-disk']"

  # Security group
  - name: create_service_account
    description: Create dedicated service account
    type: boolean
    default: true
    condition: "environment in ['staging', 'production']"
    
  - name: rbac_enabled
    description: Enable RBAC for service account
    type: boolean
    default: true
    condition: "create_service_account == true"
    
  - name: pod_security_policy
    description: Pod security policy name
    type: string
    condition: "environment == 'production'"
    
  - name: image_pull_secret
    description: Secret name for private image registry
    type: string
    condition: "image_tag contains 'private' || environment == 'production'"

  # Monitoring group
  - name: enable_prometheus
    description: Add Prometheus monitoring annotations
    type: boolean
    default: true
    condition: "environment in ['staging', 'production']"
    
  - name: enable_jaeger
    description: Enable Jaeger tracing
    type: boolean
    default: false
    condition: "environment == 'production'"
    
  - name: custom_annotations
    description: Custom pod annotations (key=value pairs, comma-separated)
    type: string
    validation:
      pattern: '^([a-z0-9.-]+=[a-zA-Z0-9._-]+)(,[a-z0-9.-]+=[a-zA-Z0-9._-]+)*$'
      examples: ['app.version=1.2.3,team=backend', 'cost-center=eng,owner=team-alpha']

  # Autoscaling group  
  - name: enable_hpa
    description: Enable Horizontal Pod Autoscaling
    type: boolean
    default: false
    condition: "environment in ['staging', 'production'] && replicas > 1"
    
  - name: min_replicas
    description: Minimum number of replicas
    type: number
    default: 2
    condition: "enable_hpa == true"
    validation:
      min: 1
      max: 10
      
  - name: max_replicas
    description: Maximum number of replicas
    type: number
    default: 10
    condition: "enable_hpa == true"
    validation:
      min: 2
      max: 100
      
  - name: target_cpu
    description: Target CPU utilization percentage
    type: number
    default: 70
    condition: "enable_hpa == true"
    validation:
      min: 10
      max: 90
      
  - name: target_memory
    description: Target memory utilization percentage
    type: number
    condition: "enable_hpa == true"
    validation:
      min: 10
      max: 90
---

# Kubernetes Application Deployment

Deploy {{ app_name }} to Kubernetes {{ environment }} environment.

## Deployment Configuration

### Application
- Image: {{ image_tag }}
- Replicas: {{ replicas }}
- Environment: {{ environment }}

{% if expose_service %}
### Networking
- Service Type: {{ service_type }}
{% if ingress_enabled %}
- Ingress Host: {{ ingress_host }}
- TLS Enabled: {{ tls_enabled }}
{% endif %}
{% endif %}

{% if needs_storage %}
### Storage
- Type: {{ storage_type }}
- Size: {{ storage_size }}
{% if storage_class %}
- Storage Class: {{ storage_class }}
{% endif %}
{% endif %}

{% if enable_hpa %}
### Autoscaling
- Min Replicas: {{ min_replicas }}
- Max Replicas: {{ max_replicas }}
- Target CPU: {{ target_cpu }}%
{% if target_memory %}
- Target Memory: {{ target_memory }}%
{% endif %}
{% endif %}
```

## Advanced Validation Examples

### Complex Pattern Validation

```yaml
parameters:
  # Kubernetes resource name validation
  - name: resource_name
    type: string
    validation:
      pattern: '^[a-z0-9]([-a-z0-9]*[a-z0-9])?$'
      min_length: 1
      max_length: 63
      examples: ['my-app', 'web-server-1', 'api']
      pattern_description: 'Lowercase alphanumeric with hyphens'

  # Docker image with optional registry
  - name: docker_image
    type: string
    validation:
      pattern: '^([a-zA-Z0-9._-]+/)?[a-zA-Z0-9._-]+:[a-zA-Z0-9._-]+$'
      examples: ['nginx:latest', 'myregistry.com/myapp:1.2.3', 'gcr.io/project/app:dev']
      pattern_description: 'Docker image with tag, optionally with registry'

  # Semantic version
  - name: version
    type: string
    validation:
      pattern: '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$'
      examples: ['1.0.0', '2.1.3-alpha.1', '1.0.0-beta+exp.sha.5114f85']
      pattern_description: 'Semantic version (major.minor.patch with optional pre-release and build metadata)'

  # CRON expression
  - name: schedule
    type: string
    validation:
      pattern: '^(\*|([0-9]|1[0-9]|2[0-9]|3[0-9]|4[0-9]|5[0-9])|\*\/([0-9]|1[0-9]|2[0-9]|3[0-9]|4[0-9]|5[0-9])) (\*|([0-9]|1[0-9]|2[0-3])|\*\/([0-9]|1[0-9]|2[0-3])) (\*|([1-9]|1[0-9]|2[0-9]|3[0-1])|\*\/([1-9]|1[0-9]|2[0-9]|3[0-1])) (\*|([1-9]|1[0-2])|\*\/([1-9]|1[0-2])) (\*|([0-6])|\*\/([0-6]))$'
      examples: ['0 2 * * *', '*/15 * * * *', '0 9-17 * * 1-5']
      pattern_description: 'CRON schedule expression'
```

### Multi-Parameter Validation Logic

```yaml
parameters:
  - name: min_value
    type: number
    validation:
      min: 0
      max: 1000
      
  - name: max_value
    type: number
    validation:
      min: 1
      max: 1000
    # This would be validated in workflow logic: max_value > min_value
    
  - name: percentage_split
    type: multi_choice
    choices: ['10', '20', '30', '40', '50', '60', '70', '80', '90']
    validation:
      min_selections: 2
      max_selections: 5
    # Would validate that percentages sum to 100 in workflow logic
```

## Usage Examples for Advanced Features

### Interactive Workflow with Conditional Groups

```bash
# Interactive mode shows only relevant parameter groups
sah flow run k8s-deploy --interactive
```

Interactive flow would show:
1. **Application** group (always shown)
2. **Networking** group (if expose_service is true)
3. **Storage** group (if needs_storage is true) 
4. **Security** group (if environment is staging/production)
5. **Monitoring** group (if environment is staging/production)
6. **Autoscaling** group (if enable_hpa is true and conditions met)

### Targeted Configuration

```bash
# Production deployment with full features
sah flow run k8s-deploy \
  --app-name myapp \
  --image-tag myregistry.com/myapp:1.2.3 \
  --environment production \
  --expose-service \
  --ingress-enabled \
  --ingress-host app.example.com \
  --needs-storage \
  --storage-type aws-ebs \
  --storage-size 100Gi \
  --enable-hpa \
  --max-replicas 20 \
  --target-cpu 80

# Simple development deployment  
sah flow run k8s-deploy \
  --app-name myapp-dev \
  --image-tag myapp:dev \
  --environment development
```

### Help Output with Grouped Parameters

```bash
sah flow run k8s-deploy --help
```

Would show organized help:
```
Application:
  --app-name <NAME>           Kubernetes application name
  --image-tag <TAG>          Docker image tag to deploy
  --replicas <COUNT>         Number of pod replicas [default: 3]
  --environment <ENV>        Deployment environment [choices: development, staging, production]

Networking (when expose-service is enabled):
  --expose-service           Expose application via service [default: true]
  --service-type <TYPE>      Service type [choices: ClusterIP, NodePort, LoadBalancer]
  --ingress-enabled          Create ingress resource
  --ingress-host <HOST>      Ingress hostname (required if ingress-enabled)
  --tls-enabled             Enable TLS at ingress [default: true]

Storage (when needs-storage is enabled):
  --needs-storage           Application requires persistent storage
  --storage-type <TYPE>     Storage type [choices: hostPath, nfs, aws-ebs, gcp-disk, azure-disk]
  --storage-size <SIZE>     Storage volume size [default: 10Gi]

...
```

These advanced features enable creating sophisticated workflows that adapt to user needs while maintaining simplicity for basic use cases.