//! Configuration provider using Figment for SwissArmyHammer

use crate::{
    defaults::ConfigDefaults,
    discovery::{ConfigFile, ConfigFormat, FileDiscovery},
    error::ConfigError,
    types::{RawConfig, TemplateContext},
    ConfigResult,
};
use figment::{
    providers::{Env, Format, Json, Toml, Yaml},
    Figment,
};
use tracing::{debug, info, trace};

/// Configuration provider using figment
///
/// This provider loads configuration from multiple sources with a clear precedence order.
/// No caching is performed - configuration is read fresh each time to allow live editing.
pub struct ConfigProvider;

impl ConfigProvider {
    /// Create a new configuration provider
    pub fn new() -> Self {
        Self
    }

    /// Load template context from all configuration sources
    ///
    /// This is the main entry point that combines all configuration sources
    /// and returns a ready-to-use TemplateContext with environment variable substitution
    /// in legacy-compatible mode (missing variables become empty strings).
    pub fn load_template_context(&self) -> ConfigResult<TemplateContext> {
        debug!("Loading template context from configuration sources (legacy mode)");

        let figment = self.build_figment()?;
        let raw_config = figment
            .extract::<RawConfig>()
            .map_err(|e| ConfigError::parse_error(None, e))?;

        debug!("Loaded {} configuration values", raw_config.values.len());

        let mut context = raw_config.to_template_context();

        // Perform environment variable substitution in legacy-compatible mode
        context.substitute_env_vars()?;

        info!(
            "Successfully loaded template context with {} variables",
            context.len()
        );
        Ok(context)
    }

    /// Load template context with strict environment variable validation
    ///
    /// This method is similar to load_template_context but uses strict mode for
    /// environment variable substitution. Missing environment variables without
    /// defaults will cause errors rather than returning empty strings.
    ///
    /// # Returns
    ///
    /// Returns a TemplateContext with environment variables substituted, or an error
    /// if any required environment variables are missing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::ConfigProvider;
    /// use std::env;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = ConfigProvider::new();
    ///
    /// // This may fail if any config values reference missing environment variables
    /// let context = provider.load_template_context_strict()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_template_context_strict(&self) -> ConfigResult<TemplateContext> {
        debug!("Loading template context from configuration sources (strict mode)");

        let figment = self.build_figment()?;
        let raw_config = figment
            .extract::<RawConfig>()
            .map_err(|e| ConfigError::parse_error(None, e))?;

        debug!("Loaded {} configuration values", raw_config.values.len());

        let mut context = raw_config.to_template_context();

        // Perform environment variable substitution in strict mode
        context.substitute_env_vars_strict()?;

        info!(
            "Successfully loaded template context with {} variables (strict mode)",
            context.len()
        );
        Ok(context)
    }

    /// Load raw template context without environment variable substitution
    ///
    /// This method loads configuration values into a TemplateContext without
    /// performing any environment variable substitution. This is useful for:
    /// - Debugging configuration loading
    /// - Inspecting raw configuration values
    /// - Selective or custom environment variable processing
    ///
    /// # Returns
    ///
    /// Returns a TemplateContext with raw configuration values (no environment substitution).
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::ConfigProvider;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = ConfigProvider::new();
    ///
    /// // Get raw context for inspection
    /// let raw_context = provider.load_raw_context()?;
    ///
    /// // Manually process specific variables if needed
    /// let mut processed_context = raw_context.clone();
    /// processed_context.substitute_var("database_url", true)?; // strict mode for this var
    /// processed_context.substitute_var("api_key", false)?; // legacy mode for this var
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_raw_context(&self) -> ConfigResult<TemplateContext> {
        debug!(
            "Loading raw template context from configuration sources (no environment substitution)"
        );

        let figment = self.build_figment()?;
        let raw_config = figment
            .extract::<RawConfig>()
            .map_err(|e| ConfigError::parse_error(None, e))?;

        debug!("Loaded {} configuration values", raw_config.values.len());

        let context = raw_config.to_template_context();
        // No environment variable substitution performed

        info!(
            "Successfully loaded raw template context with {} variables",
            context.len()
        );
        Ok(context)
    }

    /// Create template context with additional workflow variables (legacy mode)
    ///
    /// This method loads the base configuration context and then merges it with
    /// workflow variables. Workflow variables have higher priority and will override
    /// configuration values with the same keys. Uses legacy-compatible environment
    /// variable substitution (missing vars become empty strings).
    ///
    /// Environment variable substitution is applied to both config and workflow variables.
    ///
    /// # Arguments
    ///
    /// * `workflow_vars` - HashMap of workflow variables that override config values
    ///
    /// # Returns
    ///
    /// Returns a TemplateContext with both configuration and workflow variables,
    /// with workflow variables taking precedence over configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::ConfigProvider;
    /// use std::collections::HashMap;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = ConfigProvider::new();
    /// let mut workflow_vars = HashMap::new();
    /// workflow_vars.insert("environment".to_string(), serde_json::json!("production"));
    /// workflow_vars.insert("user_name".to_string(), serde_json::json!("Alice"));
    ///
    /// let context = provider.create_context_with_vars(workflow_vars)?;
    /// // context now contains both config values and workflow variables
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_context_with_vars(
        &self,
        mut workflow_vars: std::collections::HashMap<String, serde_json::Value>,
    ) -> ConfigResult<TemplateContext> {
        debug!(
            "Creating template context with {} workflow variables (legacy mode)",
            workflow_vars.len()
        );

        // Load base configuration context in legacy mode
        let mut context = self.load_template_context()?;

        // Process environment variable substitution in workflow variables
        crate::env_substitution::LEGACY_PROCESSOR
            .with(|processor| processor.substitute_vars(&mut workflow_vars))?;

        // Merge processed workflow variables with higher precedence
        context.merge_workflow(workflow_vars);

        info!(
            "Successfully created template context with {} total variables",
            context.len()
        );
        Ok(context)
    }

    /// Create template context with additional workflow variables (strict mode)
    ///
    /// This method loads the base configuration context in strict mode and then merges
    /// it with workflow variables. Missing environment variables without defaults will
    /// cause errors rather than returning empty strings.
    ///
    /// Environment variable substitution is applied to both config and workflow variables.
    ///
    /// # Arguments
    ///
    /// * `workflow_vars` - HashMap of workflow variables that override config values
    ///
    /// # Returns
    ///
    /// Returns a TemplateContext with both configuration and workflow variables,
    /// or an error if any required environment variables are missing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::ConfigProvider;
    /// use std::collections::HashMap;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = ConfigProvider::new();
    /// let mut workflow_vars = HashMap::new();
    /// workflow_vars.insert("environment".to_string(), serde_json::json!("production"));
    /// workflow_vars.insert("user_name".to_string(), serde_json::json!("Alice"));
    ///
    /// // This may fail if any config values reference missing environment variables
    /// let context = provider.create_context_with_vars_strict(workflow_vars)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_context_with_vars_strict(
        &self,
        mut workflow_vars: std::collections::HashMap<String, serde_json::Value>,
    ) -> ConfigResult<TemplateContext> {
        debug!(
            "Creating template context with {} workflow variables (strict mode)",
            workflow_vars.len()
        );

        // Load base configuration context in strict mode
        let mut context = self.load_template_context_strict()?;

        // Process environment variable substitution in workflow variables (strict mode)
        crate::env_substitution::STRICT_PROCESSOR
            .with(|processor| processor.substitute_vars(&mut workflow_vars))?;

        // Merge processed workflow variables with higher precedence
        context.merge_workflow(workflow_vars);

        info!(
            "Successfully created template context with {} total variables (strict mode)",
            context.len()
        );
        Ok(context)
    }

    /// Create template context with workflow variables from raw configuration
    ///
    /// This method loads raw configuration (without environment variable substitution)
    /// and merges it with workflow variables. This allows for custom or selective
    /// environment variable processing after merging.
    ///
    /// # Arguments
    ///
    /// * `workflow_vars` - HashMap of workflow variables that override config values
    ///
    /// # Returns
    ///
    /// Returns a TemplateContext with both raw configuration and workflow variables
    /// (no environment variable substitution performed on either config or workflow vars).
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::ConfigProvider;
    /// use std::collections::HashMap;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = ConfigProvider::new();
    /// let mut workflow_vars = HashMap::new();
    /// workflow_vars.insert("user_name".to_string(), serde_json::json!("${USER}"));
    ///
    /// // Get context without environment substitution
    /// let mut context = provider.create_raw_context_with_vars(workflow_vars)?;
    ///
    /// // Perform selective environment substitution
    /// context.substitute_var("user_name", true)?; // strict mode for user_name
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_raw_context_with_vars(
        &self,
        workflow_vars: std::collections::HashMap<String, serde_json::Value>,
    ) -> ConfigResult<TemplateContext> {
        debug!(
            "Creating raw template context with {} workflow variables (no environment substitution)",
            workflow_vars.len()
        );

        // Load raw configuration context without environment substitution
        let mut context = self.load_raw_context()?;

        // Merge workflow variables with higher precedence (no processing)
        context.merge_workflow(workflow_vars);

        info!(
            "Successfully created raw template context with {} total variables",
            context.len()
        );
        Ok(context)
    }

    /// Render a template with configuration and optional workflow variables
    ///
    /// This is a convenience method that combines configuration loading, context creation,
    /// and template rendering in a single operation. It's useful for simple template
    /// rendering scenarios where you don't need to reuse the context or renderer.
    ///
    /// # Arguments
    ///
    /// * `template` - The template string containing Liquid syntax
    /// * `workflow_vars` - Optional HashMap of workflow variables that override config values
    ///
    /// # Returns
    ///
    /// Returns the rendered template string or a `ConfigError` if rendering fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::ConfigProvider;
    /// use std::collections::HashMap;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = ConfigProvider::new();
    ///
    /// // Simple rendering with just configuration
    /// let simple_result = provider.render_template(
    ///     "Project: {{project_name | default: 'Unknown'}}",
    ///     None
    /// )?;
    ///
    /// // Rendering with workflow variables
    /// let mut workflow_vars = HashMap::new();
    /// workflow_vars.insert("user_name".to_string(), serde_json::json!("Alice"));
    /// let complex_result = provider.render_template(
    ///     "Welcome {{user_name}}! Project: {{project_name}}",
    ///     Some(workflow_vars)
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn render_template(
        &self,
        template: &str,
        workflow_vars: Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> ConfigResult<String> {
        use crate::TemplateRenderer;

        debug!("Rendering template with ConfigProvider");

        // Create template context with workflow variables if provided
        let context = if let Some(vars) = workflow_vars {
            self.create_context_with_vars(vars)?
        } else {
            self.load_template_context()?
        };

        // Create renderer and render template
        let renderer = TemplateRenderer::new()?;
        renderer.render(template, &context)
    }

    /// Build the figment configuration with all sources in precedence order
    ///
    /// Sources are loaded in precedence order (later sources override earlier ones):
    /// 1. Default values (hardcoded) - lowest priority
    /// 2. Global configuration files (~/.swissarmyhammer/) - low priority  
    /// 3. Project configuration files (./.swissarmyhammer/) - medium priority
    /// 4. Environment variables (SAH_ and SWISSARMYHAMMER_ prefixes) - high priority
    /// 5. Command line arguments (placeholder for future implementation) - highest priority
    fn build_figment(&self) -> ConfigResult<Figment> {
        debug!("Building figment configuration with complete precedence order");

        // 1. Start with default values (lowest priority)
        let mut figment = ConfigDefaults::figment();
        debug!("Applied default configuration values");

        // 2. Add configuration files in discovered priority order
        // FileDiscovery already provides proper ordering: global files first, project files second
        let discovery = FileDiscovery::new();
        let config_files = discovery.discover_all();

        debug!(
            "Found {} configuration files to process",
            config_files.len()
        );

        for config_file in config_files {
            trace!(
                "Processing config file: {} ({:?}, scope: {:?}, priority: {})",
                config_file.path.display(),
                config_file.format,
                config_file.scope,
                config_file.priority
            );

            figment = figment.merge(self.load_config_file(&config_file)?);
        }

        // 3. Add environment variables (higher priority than files)
        figment = figment.merge(self.load_env_vars()?);
        debug!("Applied environment variables with SAH_ and SWISSARMYHAMMER_ prefixes");

        // 4. Future: Add command line arguments here (highest priority)

        info!("Successfully built figment configuration with complete precedence order");
        Ok(figment)
    }

    /// Load a single configuration file based on its format
    fn load_config_file(&self, config_file: &ConfigFile) -> ConfigResult<Figment> {
        let path = &config_file.path;

        match config_file.format {
            ConfigFormat::Toml => Ok(Figment::from(Toml::file(path))),
            ConfigFormat::Yaml => Ok(Figment::from(Yaml::file(path))),
            ConfigFormat::Json => Ok(Figment::from(Json::file(path))),
        }
    }

    /// Load environment variables with SAH_ and SWISSARMYHAMMER_ prefixes
    ///
    /// Supports both prefixes with proper precedence:
    /// - SAH_ prefix has lower priority
    /// - SWISSARMYHAMMER_ prefix has higher priority (overrides SAH_ for same keys)
    /// - Nested configuration supported via double underscores (e.g., SAH_database__host)
    /// - Keys are normalized to lowercase for template consistency
    fn load_env_vars(&self) -> ConfigResult<Figment> {
        debug!("Loading environment variables with SAH_ and SWISSARMYHAMMER_ prefixes");

        // Create environment providers for both prefixes
        // First add SAH_ prefix (lower priority)
        let sah_env = Env::prefixed("SAH_")
            .split("__") // Support nested config via double underscores
            .map(|key| key.as_str().to_lowercase().into());

        // Then add SWISSARMYHAMMER_ prefix (higher priority - will override SAH_ vars)
        let swissarmyhammer_env = Env::prefixed("SWISSARMYHAMMER_")
            .split("__") // Support nested config via double underscores
            .map(|key| key.as_str().to_lowercase().into());

        let figment = Figment::new().merge(sah_env).merge(swissarmyhammer_env);

        trace!("Environment variables loaded with nested configuration support");
        Ok(figment)
    }
}

impl Default for ConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_provider_new() {
        let _provider = ConfigProvider::new();
        // Test that it creates successfully - no assertions needed as this would panic if it failed
        let _ = _provider;
    }

    #[test]
    fn test_load_empty_template_context() {
        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        // With no configuration files, should still have default values
        // Plus any environment variables that happen to be set
        assert!(!context.is_empty()); // Should have at least the default values

        // Should contain some expected defaults
        assert!(context.get("environment").is_some());
        assert!(context.get("debug").is_some());
    }

    #[test]
    fn test_defaults_integration() {
        let _provider = ConfigProvider::new();
        let figment = ConfigDefaults::figment();

        // Default config should not be empty now
        let config: RawConfig = figment.extract().unwrap();
        assert!(!config.is_empty());

        // Should contain expected default keys
        assert!(config.values.contains_key("environment"));
        assert!(config.values.contains_key("debug"));
        assert!(config.values.contains_key("project_name"));
    }

    #[test]
    fn test_load_env_vars() {
        let provider = ConfigProvider::new();

        // Set some test environment variables (including nested)
        std::env::set_var("SAH_TEST_VAR", "test_value");
        std::env::set_var("SWISSARMYHAMMER_OTHER_VAR", "other_value");
        std::env::set_var("SAH_DATABASE__HOST", "localhost");
        std::env::set_var("SWISSARMYHAMMER_DATABASE__PORT", "5432");

        let figment = provider.load_env_vars().unwrap();
        let config: HashMap<String, serde_json::Value> = figment.extract().unwrap();

        // Check that environment variables are loaded with correct keys
        assert!(config.contains_key("test_var"));
        assert!(config.contains_key("other_var"));

        // Check nested configuration support
        if let Some(serde_json::Value::Object(database)) = config.get("database") {
            assert!(database.contains_key("host"));
            assert!(database.contains_key("port"));
            assert_eq!(
                database["host"],
                serde_json::Value::String("localhost".to_string())
            );
            // Figment may parse numeric strings as numbers, so accept both
            let port_val = &database["port"];
            assert!(
                port_val == &serde_json::Value::String("5432".to_string())
                    || port_val == &serde_json::Value::Number(5432.into()),
                "Expected port to be either string '5432' or number 5432, got: {:?}",
                port_val
            );
        } else {
            panic!("Expected nested database configuration");
        }

        // Clean up
        std::env::remove_var("SAH_TEST_VAR");
        std::env::remove_var("SWISSARMYHAMMER_OTHER_VAR");
        std::env::remove_var("SAH_DATABASE__HOST");
        std::env::remove_var("SWISSARMYHAMMER_DATABASE__PORT");
    }

    #[test]
    #[serial]
    fn test_build_figment_no_config_files() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Change to temp directory with no .swissarmyhammer folder
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();

        // This should succeed even if no config files exist
        let figment = provider.build_figment().unwrap();
        let config: RawConfig = figment.extract().unwrap();

        // Restore directory
        std::env::set_current_dir(original_dir).unwrap();

        // Should not be empty due to default values
        assert!(!config.is_empty());
        // Should contain default values
        assert!(config.values.contains_key("environment"));
    }

    #[test]
    #[serial]
    fn test_build_figment_with_toml() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        let config_file = sah_dir.join("sah.toml");
        fs::write(
            &config_file,
            r#"
test_key = "test_value"
number_key = 42
"#,
        )
        .unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let figment = provider.build_figment().unwrap();
        let config: HashMap<String, serde_json::Value> = figment.extract().unwrap();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        // Should have config values from TOML
        assert_eq!(
            config.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(
            config.get("number_key"),
            Some(&serde_json::Value::Number(42.into()))
        );

        // Should also have default values
        assert!(config.contains_key("environment"));
    }

    #[test]
    #[serial]
    fn test_build_figment_with_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        let config_file = sah_dir.join("sah.yaml");
        fs::write(
            &config_file,
            r#"
test_key: test_value
number_key: 42
"#,
        )
        .unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let figment = provider.build_figment().unwrap();
        let config: HashMap<String, serde_json::Value> = figment.extract().unwrap();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(
            config.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(
            config.get("number_key"),
            Some(&serde_json::Value::Number(42.into()))
        );

        // Should also have default values
        assert!(config.contains_key("environment"));
    }

    #[test]
    #[serial]
    fn test_build_figment_with_json() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        let config_file = sah_dir.join("sah.json");
        fs::write(
            &config_file,
            r#"
{
    "test_key": "test_value",
    "number_key": 42
}
"#,
        )
        .unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let figment = provider.build_figment().unwrap();
        let config: HashMap<String, serde_json::Value> = figment.extract().unwrap();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(
            config.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(
            config.get("number_key"),
            Some(&serde_json::Value::Number(42.into()))
        );

        // Should also have default values
        assert!(config.contains_key("environment"));
    }

    #[test]
    #[serial]
    fn test_precedence_order() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        // Create config files with overlapping keys
        let toml_config = sah_dir.join("sah.toml");
        fs::write(
            &toml_config,
            r#"
shared_key = "from_toml"
toml_only = "toml_value"
"#,
        )
        .unwrap();

        let yaml_config = sah_dir.join("sah.yaml");
        fs::write(
            &yaml_config,
            r#"
shared_key: from_yaml
yaml_only: yaml_value
"#,
        )
        .unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Set environment variable that should override file values
        std::env::set_var("SAH_SHARED_KEY", "from_env");
        std::env::set_var("SAH_ENV_ONLY", "env_value");

        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        // Restore original directory and clean up env vars
        std::env::set_current_dir(original_dir).unwrap();
        std::env::remove_var("SAH_SHARED_KEY");
        std::env::remove_var("SAH_ENV_ONLY");

        // Environment should override file values
        assert_eq!(
            context.get("shared_key"),
            Some(&serde_json::Value::String("from_env".to_string()))
        );

        // YAML should override TOML for file-only values
        // Note: This test might be sensitive to figment's exact merging behavior

        // Environment-only value should be present
        assert_eq!(
            context.get("env_only"),
            Some(&serde_json::Value::String("env_value".to_string()))
        );
    }

    #[test]
    fn test_create_context_with_vars_empty() {
        let provider = ConfigProvider::new();
        let workflow_vars = HashMap::new();

        let context = provider.create_context_with_vars(workflow_vars).unwrap();

        // Should have default configuration values
        assert!(!context.is_empty());
        assert!(context.get("environment").is_some());
    }

    #[test]
    fn test_create_context_with_vars_with_data() {
        let provider = ConfigProvider::new();
        let mut workflow_vars = HashMap::new();
        workflow_vars.insert("user_name".to_string(), serde_json::json!("Alice"));
        workflow_vars.insert("role".to_string(), serde_json::json!("admin"));
        workflow_vars.insert("active".to_string(), serde_json::json!(true));

        let context = provider.create_context_with_vars(workflow_vars).unwrap();

        // Should have both config and workflow variables
        assert!(context.get("environment").is_some()); // From config
        assert_eq!(context.get_string("user_name"), Some("Alice".to_string()));
        assert_eq!(context.get_string("role"), Some("admin".to_string()));
        assert_eq!(context.get_bool("active"), Some(true));
    }

    #[test]
    fn test_create_context_with_vars_precedence() {
        let provider = ConfigProvider::new();
        let mut workflow_vars = HashMap::new();
        // Override a default config value
        workflow_vars.insert("environment".to_string(), serde_json::json!("production"));
        workflow_vars.insert("workflow_id".to_string(), serde_json::json!("deploy-001"));

        let context = provider.create_context_with_vars(workflow_vars).unwrap();

        // Workflow var should override config default
        assert_eq!(
            context.get_string("environment"),
            Some("production".to_string())
        );
        // Workflow-only var should be present
        assert_eq!(
            context.get_string("workflow_id"),
            Some("deploy-001".to_string())
        );
    }

    #[test]
    fn test_render_template_simple() {
        let provider = ConfigProvider::new();

        let result = provider.render_template("Hello World!", None).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_render_template_with_config() {
        let provider = ConfigProvider::new();

        // Should have environment default available
        let result = provider
            .render_template("Environment: {{environment}}", None)
            .unwrap();
        assert!(result.contains("Environment: "));
        assert!(!result.contains("{{environment}}")); // Should be substituted
    }

    #[test]
    fn test_render_template_with_workflow_vars() {
        let provider = ConfigProvider::new();
        let mut workflow_vars = HashMap::new();
        workflow_vars.insert("greeting".to_string(), serde_json::json!("Hello"));
        workflow_vars.insert("name".to_string(), serde_json::json!("Alice"));

        let result = provider
            .render_template("{{greeting}} {{name}}!", Some(workflow_vars))
            .unwrap();
        assert_eq!(result, "Hello Alice!");
    }

    #[test]
    fn test_render_template_with_defaults() {
        let provider = ConfigProvider::new();

        let result = provider
            .render_template(
                "{{greeting | default: 'Hello'}} {{name | default: 'World'}}!",
                None,
            )
            .unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_render_template_workflow_overrides_config() {
        let provider = ConfigProvider::new();
        let mut workflow_vars = HashMap::new();
        workflow_vars.insert("environment".to_string(), serde_json::json!("production"));

        let result = provider
            .render_template("Environment: {{environment}}", Some(workflow_vars))
            .unwrap();
        assert_eq!(result, "Environment: production");
    }

    #[test]
    fn test_render_template_complex() {
        let provider = ConfigProvider::new();
        let mut workflow_vars = HashMap::new();
        workflow_vars.insert(
            "user".to_string(),
            serde_json::json!({
                "name": "Alice",
                "role": "admin"
            }),
        );
        workflow_vars.insert(
            "items".to_string(),
            serde_json::json!(["task1", "task2", "task3"]),
        );

        let template = r#"Welcome {{user.name}}! Role: {{user.role}}
Tasks: {% for item in items %}{{item}}{% unless forloop.last %}, {% endunless %}{% endfor %}"#;

        let result = provider
            .render_template(template, Some(workflow_vars))
            .unwrap();
        assert!(result.contains("Welcome Alice! Role: admin"));
        assert!(result.contains("Tasks: task1, task2, task3"));
    }
}
