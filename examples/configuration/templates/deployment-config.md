---
title: Deployment Configuration Generator
description: Generates deployment configuration using environment-aware settings
arguments:
  - name: target_environment
    description: Target environment for deployment (development, staging, production)
    required: true
  - name: deployment_notes
    description: Additional deployment notes
    required: false
---

# Deployment Configuration: {{app.name}}

**Target Environment**: {{target_environment | capitalize}}  
**Application Version**: {{app.version}}
{% if build.commit_hash -%}
**Build Commit**: {{build.commit_hash}}
{% endif %}
{% if build.build_timestamp -%}
**Build Time**: {{build.build_timestamp}}
{% endif %}

## Environment Configuration

### Application Settings
- **Name**: {{app.name}}
- **Environment**: {{target_environment}}
{% if app.debug -%}
- **Debug Mode**: {% if target_environment == "production" %}❌ Disabled (forced for production){% else %}✅ Enabled{% endif %}
{% else -%}
- **Debug Mode**: ❌ Disabled
{% endif %}
- **Log Level**: {{app.log_level | default: "info" | upcase}}

### Database Configuration

#### Primary Database
```yaml
host: {{database.host}}
port: {{database.port}}
database: {{database.database}}
ssl_enabled: {{database.ssl_enabled}}
timeout_seconds: {{database.timeout_seconds}}
max_connections: {{database.max_connections}}
```

{% if database.credentials -%}
#### Credentials (Use secrets management in production)
```bash
# Environment variables for database credentials
DATABASE_USER={{database.credentials.username}}
DATABASE_PASSWORD=${DATABASE_PASSWORD}  # Set securely
```
{% endif %}

### Feature Flags

Production-ready features:
{% for feature in features -%}
{% if feature[1] -%}
- ✅ `{{feature[0]}}`: Enabled
{% else -%}
- ❌ `{{feature[0]}}`: Disabled
{% endif -%}
{% endfor %}

### Cache Configuration

{% if cache -%}
```yaml
cache:
  enabled: {{cache.enabled}}
  provider: {{cache.provider}}
  {% if cache.ttl_seconds -%}ttl_seconds: {{cache.ttl_seconds}}{% endif %}
  {% if cache.max_size_mb -%}max_size_mb: {{cache.max_size_mb}}{% endif %}
```
{% endif %}

## Environment Variables

### Required Environment Variables

```bash
# Application
export SAH_APP_NAME="{{app.name}}"
export SAH_APP_VERSION="{{app.version}}"
export SAH_APP_ENVIRONMENT="{{target_environment}}"

# Database
export SAH_DATABASE_HOST="{{database.host}}"
export SAH_DATABASE_PORT="{{database.port}}"
export SAH_DATABASE_DATABASE="{{database.database}}"
export SAH_DATABASE_CREDENTIALS_USERNAME="{{database.credentials.username | default: 'app_user'}}"
export SAH_DATABASE_CREDENTIALS_PASSWORD="$DATABASE_PASSWORD"

{% if api -%}
# API Configuration
export SAH_API_BASE_URL="{{api.base_url}}"
export SAH_API_VERSION="{{api.version}}"
export SAH_API_TIMEOUT_SECONDS="{{api.timeout_seconds}}"
{% endif %}

# Feature Flags
{% for feature in features -%}
export SAH_FEATURES_{{feature[0] | upcase}}="{{feature[1] | downcase}}"
{% endfor %}
```

## Security Considerations

{% if target_environment == "production" -%}
### Production Security Checklist

- [ ] All passwords and secrets use secure secret management
- [ ] SSL/TLS is enabled for all database connections
- [ ] Debug mode is disabled
- [ ] Logging level is set to `warn` or `error`
- [ ] Feature flags for experimental features are disabled
- [ ] API timeouts are set to reasonable values
- [ ] Cache settings are optimized for production load

### Secrets Management

The following values should be managed as secrets:
- `DATABASE_PASSWORD`
{% if api -%}
- API keys and authentication tokens
{% endif %}
- JWT secrets and signing keys
- Third-party service credentials

{% elsif target_environment == "staging" -%}
### Staging Environment Notes

- Debug mode can be enabled for testing
- Use production-like database configuration
- Enable monitoring and logging for testing
- Feature flags can be enabled for testing new features

{% else -%}
### Development Environment Notes

- Debug mode is enabled for development convenience
- Local database connections are acceptable
- Feature flags can be freely enabled for testing
- Relaxed security settings for development productivity
{% endif %}

## Health Checks

### Database Health Check
```bash
# Test database connectivity
psql -h {{database.host}} -p {{database.port}} -U {{database.credentials.username | default: 'app_user'}} -d {{database.database}} -c "SELECT 1;"
```

{% if api -%}
### API Health Check
```bash
# Test API connectivity
curl -f {{api.base_url}}/{{api.version}}/health || echo "API health check failed"
```
{% endif %}

{% if cache -%}
### Cache Health Check
```bash
# Test cache connectivity (adjust for your cache provider)
{% if cache.provider == "redis" -%}
redis-cli ping || echo "Redis cache unavailable"
{% elsif cache.provider == "memory" -%}
# Memory cache - no external dependency to check
echo "Memory cache is application-internal"
{% else -%}
# Test {{cache.provider}} cache connectivity
echo "Configure health check for {{cache.provider}} cache"
{% endif %}
```
{% endif %}

## Deployment Steps

1. **Pre-deployment**:
   - Validate configuration with `sah validate`
   - Run tests in staging environment
   - Verify all required environment variables are set

2. **Deployment**:
   - Deploy application with environment-specific configuration
   - Run database migrations if required
   - Verify health checks pass

3. **Post-deployment**:
   - Monitor application logs
   - Verify feature flags are working as expected
   - Test critical user workflows

{% if deployment_notes -%}
## Additional Deployment Notes

{{deployment_notes}}
{% endif %}

---
**Generated for**: {{target_environment | capitalize}} environment  
**Application**: {{app.name}} v{{app.version}}  
**Generated**: {{variables.build_date | default: "unknown date"}}