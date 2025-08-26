# SwissArmyHammer Configuration Examples

This directory contains practical examples of SwissArmyHammer configuration files in all supported formats (TOML, YAML, JSON). Each example demonstrates common configuration patterns and use cases.

## Example Files

### Basic Examples
- [`basic.toml`](basic.toml) - Simple TOML configuration with common settings
- [`basic.yaml`](basic.yaml) - Equivalent YAML configuration  
- [`basic.json`](basic.json) - Equivalent JSON configuration

### Web Development Examples  
- [`web-app.toml`](web-app.toml) - Configuration for web application development
- [`web-app.yaml`](web-app.yaml) - YAML version of web app configuration
- [`web-app.json`](web-app.json) - JSON version of web app configuration

### DevOps Examples
- [`devops.toml`](devops.toml) - DevOps and deployment configuration
- [`devops.yaml`](devops.yaml) - YAML version for DevOps workflows
- [`devops.json`](devops.json) - JSON version for automated tooling

### Advanced Examples
- [`advanced.toml`](advanced.toml) - Advanced features with environment variables
- [`advanced.yaml`](advanced.yaml) - Complex YAML with substitution patterns
- [`advanced.json`](advanced.json) - Feature-rich JSON configuration

### Template Examples
- [`templates/`](templates/) - Example liquid templates using configuration values

## Usage

### Global Configuration

Copy any example file to your global SwissArmyHammer directory:

```bash
# Create global config directory
mkdir -p ~/.swissarmyhammer

# Copy example (choose your preferred format)
cp examples/configuration/basic.toml ~/.swissarmyhammer/sah.toml
cp examples/configuration/basic.yaml ~/.swissarmyhammer/sah.yaml  
cp examples/configuration/basic.json ~/.swissarmyhammer/sah.json
```

### Project Configuration

Copy any example file to your project's SwissArmyHammer directory:

```bash
# Create project config directory
mkdir -p ./.swissarmyhammer

# Copy example (choose your preferred format)
cp examples/configuration/web-app.toml ./.swissarmyhammer/sah.toml
cp examples/configuration/web-app.yaml ./.swissarmyhammer/sah.yaml
cp examples/configuration/web-app.json ./.swissarmyhammer/sah.json
```

### Testing Configuration

Test any configuration file with the SwissArmyHammer CLI:

```bash
# Copy configuration file to test location
cp examples/configuration/basic.toml ./.swissarmyhammer/sah.toml

# Validate configuration
sah validate

# Show loaded configuration
sah doctor

# Test with a template
echo "Project: {{app.name}} v{{app.version}}" | sah template render --stdin
```

## Environment Variables

All examples support environment variable overrides. Set environment variables using the `SAH_` or `SWISSARMYHAMMER_` prefix:

```bash
# Override app name
export SAH_APP_NAME="MyCustomApp"

# Override database settings  
export SAH_DATABASE_HOST="custom-db.example.com"
export SAH_DATABASE_PORT="3306"

# Override feature flags
export SAH_FEATURES_DEBUG="true"
export SAH_FEATURES_TELEMETRY="false"

# Test the overrides
sah doctor
```

## Customization

Use these examples as starting points for your own configuration:

1. **Choose your format** - Pick TOML, YAML, or JSON based on your preference
2. **Start simple** - Begin with basic examples and add complexity as needed  
3. **Test thoroughly** - Use `sah doctor` and `sah validate` to verify configuration
4. **Use environment variables** - Leverage env vars for deployment-specific settings
5. **Document your config** - Add comments explaining your configuration choices

## Format Comparison

| Feature | TOML | YAML | JSON |
|---------|------|------|------|
| **Comments** | ✓ | ✓ | ✗ |
| **Readability** | Excellent | Excellent | Good |
| **Nesting** | Good | Excellent | Good |
| **Types** | Strong | Strong | Limited |
| **Tools** | Good | Excellent | Universal |
| **Generation** | Manual | Manual/Generated | Generated |

Choose the format that best fits your workflow and team preferences.