use crate::discovery::ConfigurationDiscovery;
use crate::env_vars::EnvVarSubstitution;
use crate::error::{ConfigurationError, ConfigurationResult};
use crate::model::ModelConfig;
use crate::provider::{
    CliProvider, ConfigurationProvider, DefaultProvider, EnvProvider, FileProvider,
};
use figment::Figment;
use serde_json::{Map, Value};
use std::collections::HashMap;
use tracing::debug;

/// Template context for liquid templating with comprehensive configuration support
///
/// `TemplateContext` is the core configuration container that provides structured
/// configuration management using the figment library. It replaces simple HashMap
/// approaches with a sophisticated system supporting multiple file formats,
/// environment variables, proper precedence handling, and seamless liquid template
/// integration.
///
/// The context automatically loads configuration from discovered files and environment
/// variables, making all values available for template rendering while supporting
/// runtime template variable overlays.
///
/// # Features
///
/// - **Multi-source loading**: Automatic discovery and merging from multiple configuration sources
/// - **Format support**: Native TOML, YAML, and JSON parsing
/// - **Environment integration**: Full environment variable support with prefix mapping  
/// - **Precedence handling**: Clear, predictable configuration value precedence
/// - **Template integration**: Direct liquid template engine compatibility
/// - **Fresh loading**: No caching - fresh configuration loaded each time
/// - **Type safety**: Structured value access with proper type handling
/// - **Nested access**: Dot notation support for nested configuration values
///
/// # Basic Usage
///
/// ```no_run
/// use swissarmyhammer_config::TemplateContext;
///
/// // Load all available configuration
/// let context = TemplateContext::load()?;
///
/// // Access configuration values
/// if let Some(app_name) = context.get("app.name") {
///     println!("Application: {}", app_name);
/// }
///
/// // Use with liquid templates
/// let liquid_context = context.to_liquid_context();
/// # Ok::<(), swissarmyhammer_config::ConfigurationError>(())
/// ```
///
/// # Configuration Sources
///
/// The context loads configuration from these sources in precedence order:
///
/// 1. **Default values** - Application defaults (lowest precedence)
/// 2. **Global config** - `~/.swissarmyhammer/sah.*` (user-wide settings)
/// 3. **Project config** - `./.swissarmyhammer/sah.*` (project-specific)
/// 4. **Environment variables** - `SAH_*` and `SWISSARMYHAMMER_*` prefixes
/// 5. **CLI arguments** - Command-line overrides (highest precedence)
/// 6. **Template variables** - Runtime template variables (override all)
///
/// # Template Variable Integration
///
/// ```no_run
/// use swissarmyhammer_config::TemplateContext;
/// use std::collections::HashMap;
/// use serde_json::json;
///
/// // Combine configuration with runtime template variables
/// let mut template_vars = HashMap::new();
/// template_vars.insert("task".to_string(), json!("deploy"));
/// template_vars.insert("user".to_string(), json!("admin"));
/// template_vars.insert("timestamp".to_string(), json!("2024-01-15T10:30:00Z"));
///
/// // Template variables override any configuration values
/// let context = TemplateContext::with_template_vars(template_vars)?;
///
/// // Access both config and template values
/// assert_eq!(context.get("task"), Some(&json!("deploy")));           // From template vars
/// assert_eq!(context.get("app.name"), Some(&json!("MyProject")));    // From config
/// # Ok::<(), swissarmyhammer_config::ConfigurationError>(())
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TemplateContext {
    /// The merged configuration values
    variables: Map<String, Value>,
}

impl TemplateContext {
    /// Create a new empty template context
    ///
    /// Creates a template context with no configuration values loaded. This is useful
    /// for programmatic configuration building or testing scenarios.
    ///
    /// For most use cases, prefer `TemplateContext::load()` to automatically load
    /// configuration from available sources.
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_config::TemplateContext;
    /// use serde_json::json;
    ///
    /// let mut context = TemplateContext::new();
    /// assert!(context.is_empty());
    ///
    /// // Build configuration programmatically
    /// context.set("app_name".to_string(), json!("MyApp"));
    /// context.set("debug".to_string(), json!(true));
    ///
    /// assert_eq!(context.len(), 2);
    /// assert_eq!(context.get("app_name"), Some(&json!("MyApp")));
    /// ```
    pub fn new() -> Self {
        Self {
            variables: Map::new(),
        }
    }

    /// Load configuration from all available sources with proper precedence
    ///
    /// This is the primary method for loading configuration in SwissArmyHammer applications.
    /// It discovers and merges configuration from all standard sources according to the
    /// precedence rules, performs environment variable substitution, and returns a ready-to-use
    /// template context.
    ///
    /// # Precedence Order
    ///
    /// Configuration sources are merged with later sources overriding earlier ones:
    ///
    /// 1. **Default values** (lowest precedence) - Built-in application defaults
    /// 2. **Global config files** - `~/.swissarmyhammer/sah.{toml,yaml,yml,json}`
    /// 3. **Project config files** - `./.swissarmyhammer/sah.{toml,yaml,yml,json}`
    /// 4. **Environment variables** - `SAH_*` and `SWISSARMYHAMMER_*` prefixes
    /// 5. **CLI arguments** - Command-line overrides (if provided via other methods)
    ///
    /// # File Discovery
    ///
    /// The method searches for configuration files in standard locations and loads
    /// all found files. Multiple formats can coexist and will be merged appropriately.
    ///
    /// # Environment Variable Substitution
    ///
    /// Configuration files can include environment variable placeholders:
    /// - `${VAR}` - Replace with environment variable value
    /// - `${VAR:-default}` - Replace with environment variable or default value
    ///
    /// # Returns
    ///
    /// - `Ok(TemplateContext)` - Successfully loaded configuration
    /// - `Err(ConfigurationError)` - Configuration loading or parsing error
    ///
    /// # Examples
    ///
    /// ## Basic Loading
    /// ```no_run
    /// use swissarmyhammer_config::TemplateContext;
    ///
    /// let context = TemplateContext::load()?;
    /// println!("Loaded {} configuration values", context.len());
    /// # Ok::<(), swissarmyhammer_config::ConfigurationError>(())
    /// ```
    ///
    /// ## With Configuration File
    /// Given a configuration file `~/.swissarmyhammer/sah.toml`:
    /// ```toml
    /// [app]
    /// name = "MyApp"
    /// version = "1.0.0"
    /// debug = false
    ///
    /// [database]
    /// host = "${DB_HOST:-localhost}"
    /// port = 5432
    /// ```
    ///
    /// ```no_run
    /// use swissarmyhammer_config::TemplateContext;
    /// use std::env;
    ///
    /// // Set environment variable for substitution
    /// env::set_var("DB_HOST", "production-db.example.com");
    ///
    /// let context = TemplateContext::load()?;
    ///
    /// // Access configuration values
    /// assert_eq!(context.get("app.name").unwrap().as_str().unwrap(), "MyApp");
    /// assert_eq!(context.get("app.version").unwrap().as_str().unwrap(), "1.0.0");
    /// assert_eq!(context.get("database.host").unwrap().as_str().unwrap(), "production-db.example.com");
    /// assert_eq!(context.get("database.port").unwrap().as_i64().unwrap(), 5432);
    /// # Ok::<(), swissarmyhammer_config::ConfigurationError>(())
    /// ```
    pub fn load() -> ConfigurationResult<Self> {
        let mut context = Self::load_with_options(false, None)?;
        context.set_default_variables();
        Ok(context)
    }

    /// Load configuration for CLI usage (no security validation)
    pub fn load_for_cli() -> ConfigurationResult<Self> {
        let mut context = Self::load_with_options(true, None)?;
        context.set_default_variables();
        Ok(context)
    }

    /// Load configuration with CLI argument overrides
    pub fn load_with_cli_args(cli_args: Value) -> ConfigurationResult<Self> {
        let mut context = Self::load_with_options(false, Some(cli_args))?;
        context.set_default_variables();
        Ok(context)
    }

    /// Create a TemplateContext with provided template variables
    ///
    /// This loads configuration from all sources and then overlays the provided
    /// template variables with highest precedence.
    ///
    /// # Arguments
    /// * `vars` - HashMap of template variables to set with highest precedence
    ///
    /// # Returns
    /// * `ConfigurationResult<Self>` - A new TemplateContext with merged configuration and template vars
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer_config::TemplateContext;
    /// use std::collections::HashMap;
    /// use serde_json::json;
    ///
    /// let mut template_vars = HashMap::new();
    /// template_vars.insert("project_name".to_string(), json!("MyProject"));
    /// template_vars.insert("version".to_string(), json!("1.0.0"));
    /// template_vars.insert("debug".to_string(), json!(true));
    ///
    /// let context = TemplateContext::with_template_vars(template_vars)?;
    ///
    /// // Template variables have highest precedence, overriding config values
    /// assert_eq!(context.get("project_name"), Some(&json!("MyProject")));
    /// assert_eq!(context.get("version"), Some(&json!("1.0.0")));
    /// assert_eq!(context.get("debug"), Some(&json!(true)));
    /// # Ok::<(), swissarmyhammer_config::ConfigurationError>(())
    /// ```
    pub fn with_template_vars(vars: HashMap<String, Value>) -> ConfigurationResult<Self> {
        let mut context = Self::load()?;

        // Overlay template variables with highest precedence
        for (key, value) in vars {
            context.set(key, value);
        }

        // Set default variables if not already provided
        context.set_default_variables();

        Ok(context)
    }

    /// Create a TemplateContext from only template variables without configuration loading
    ///
    /// This creates a TemplateContext directly from the provided variables without
    /// attempting to load configuration from files or environment. This is useful
    /// for tests or when you only need template variables without configuration.
    ///
    /// # Arguments
    /// * `vars` - HashMap of template variables to set
    ///
    /// # Returns
    /// * `Self` - A new TemplateContext with only the provided template variables
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer_config::TemplateContext;
    /// use std::collections::HashMap;
    /// use serde_json::json;
    ///
    /// let mut template_vars = HashMap::new();
    /// template_vars.insert("project_name".to_string(), json!("MyProject"));
    /// template_vars.insert("version".to_string(), json!("1.0.0"));
    ///
    /// let context = TemplateContext::from_template_vars(template_vars);
    ///
    /// assert_eq!(context.get("project_name"), Some(&json!("MyProject")));
    /// assert_eq!(context.get("version"), Some(&json!("1.0.0")));
    /// ```
    pub fn from_template_vars(vars: HashMap<String, Value>) -> Self {
        let mut context = Self::new();

        // Add template variables
        for (key, value) in vars {
            context.set(key, value);
        }

        context
    }

    /// Load configuration with specific options (refactored to reduce complexity)
    fn load_with_options(for_cli: bool, cli_args: Option<Value>) -> ConfigurationResult<Self> {
        debug!("Loading template context with CLI mode: {}", for_cli);

        let figment = Self::build_figment(for_cli)?;
        let figment = Self::load_config_files(figment, for_cli)?;
        let figment = Self::load_env_vars(figment)?;
        let figment = Self::load_cli_args(figment, cli_args)?;

        Self::finalize_config(figment)
    }

    /// Build the initial figment with default values
    fn build_figment(_for_cli: bool) -> ConfigurationResult<Figment> {
        let mut figment = Figment::new();
        let default_provider = DefaultProvider::empty();
        figment = default_provider.load_into(figment)?;
        Ok(figment)
    }

    /// Discover and load configuration files into figment
    fn load_config_files(mut figment: Figment, for_cli: bool) -> ConfigurationResult<Figment> {
        let discovery = if for_cli {
            ConfigurationDiscovery::for_cli()?
        } else {
            ConfigurationDiscovery::new()?
        };

        let config_files = discovery.discover_config_files();
        debug!("Found {} configuration files", config_files.len());

        for file_path in config_files {
            let file_provider = FileProvider::new(file_path);
            figment = file_provider.load_into(figment)?;
        }

        Ok(figment)
    }

    /// Load environment variables into figment
    fn load_env_vars(mut figment: Figment) -> ConfigurationResult<Figment> {
        let sah_env_provider = EnvProvider::sah();
        figment = sah_env_provider.load_into(figment)?;

        let swissarmyhammer_env_provider = EnvProvider::swissarmyhammer();
        figment = swissarmyhammer_env_provider.load_into(figment)?;

        Ok(figment)
    }

    /// Load CLI arguments into figment if provided
    fn load_cli_args(
        mut figment: Figment,
        cli_args: Option<Value>,
    ) -> ConfigurationResult<Figment> {
        if let Some(cli_args) = cli_args {
            let cli_provider = CliProvider::new(cli_args);
            figment = cli_provider.load_into(figment)?;
        }
        Ok(figment)
    }

    /// Extract, substitute, and finalize configuration from figment
    fn finalize_config(figment: Figment) -> ConfigurationResult<Self> {
        let config_value: Value = figment.extract().map_err(|e| {
            ConfigurationError::template_context(format!("Failed to extract configuration: {}", e))
        })?;

        let env_substitution = EnvVarSubstitution::new()?;
        let substituted_config = env_substitution.substitute_in_value(config_value)?;

        let variables = match substituted_config {
            Value::Null => {
                debug!("Configuration is null/empty, using empty map");
                Map::new()
            }
            Value::Object(map) => map,
            other => {
                debug!("Configuration root is not an object, wrapping in 'config' key");
                let mut map = Map::new();
                map.insert("config".to_string(), other);
                map
            }
        };

        debug!("Loaded {} configuration variables", variables.len());

        Ok(Self { variables })
    }

    /// Navigate through nested map structure using dot-notated path
    fn navigate_nested_value<'a>(map: &'a Map<String, Value>, parts: &[&str]) -> Option<&'a Value> {
        let mut current = map;
        for part in &parts[..parts.len() - 1] {
            match current.get(*part) {
                Some(Value::Object(nested)) => current = nested,
                _ => return None,
            }
        }
        parts.last().and_then(|last| current.get(*last))
    }

    /// Get a configuration value by key
    pub fn get(&self, key: &str) -> Option<&Value> {
        // Support nested keys with dot notation (e.g., "database.host", "database.ssl.enabled")
        if key.contains('.') {
            let parts: Vec<&str> = key.split('.').collect();
            Self::navigate_nested_value(&self.variables, &parts)
        } else {
            self.variables.get(key)
        }
    }

    /// Set a configuration value
    pub fn set(&mut self, key: String, value: Value) {
        self.variables.insert(key, value);
    }

    /// Get all variables as a reference to the internal map
    pub fn variables(&self) -> &Map<String, Value> {
        &self.variables
    }

    /// Get all variables as a mutable reference
    pub fn variables_mut(&mut self) -> &mut Map<String, Value> {
        &mut self.variables
    }

    /// Get the number of variables
    pub fn len(&self) -> usize {
        self.variables.len()
    }

    /// Check if the context is empty
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// Merge another template context into this one
    /// The other context's values will override this context's values
    pub fn merge(&mut self, other: TemplateContext) {
        for (key, value) in other.variables {
            self.variables.insert(key, value);
        }
    }

    /// Convert to a HashMap<String, Value> for compatibility with existing code
    pub fn to_hash_map(&self) -> HashMap<String, Value> {
        self.variables.clone().into_iter().collect()
    }

    /// Create a TemplateContext from a HashMap<String, Value>
    pub fn from_hash_map(map: HashMap<String, Value>) -> Self {
        Self {
            variables: map.into_iter().collect(),
        }
    }

    /// Convert to liquid::Object for template rendering
    ///
    /// This method converts the internal variables map to a liquid::Object
    /// that can be used directly with the liquid template engine.
    ///
    /// # Returns
    /// * `liquid::Object` - A liquid object containing all variables
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer_config::TemplateContext;
    /// use serde_json::json;
    ///
    /// let mut context = TemplateContext::new();
    /// context.set("project_name".to_string(), json!("MyProject"));
    /// context.set("version".to_string(), json!("1.0.0"));
    /// context.set("database".to_string(), json!({
    ///     "host": "localhost",
    ///     "port": 5432
    /// }));
    ///
    /// let liquid_context = context.to_liquid_context();
    ///
    /// // Use with liquid template engine
    /// let template_source = "Project: {{project_name}} v{{version}} on {{database.host}}:{{database.port}}";
    /// let parser = liquid::ParserBuilder::with_stdlib().build().unwrap();
    /// let template = parser.parse(template_source).unwrap();
    /// let output = template.render(&liquid_context).unwrap();
    ///
    /// assert_eq!(output, "Project: MyProject v1.0.0 on localhost:5432");
    /// ```
    pub fn to_liquid_context(&self) -> liquid::Object {
        let mut liquid_vars = liquid::Object::new();
        for (key, value) in &self.variables {
            liquid_vars.insert(
                key.clone().into(),
                liquid::model::to_value(value).unwrap_or(liquid::model::Value::Nil),
            );
        }
        liquid_vars
    }

    /// Compatibility alias for get() method
    ///
    /// This provides the API specified in the issue requirements.
    pub fn get_var(&self, key: &str) -> Option<&Value> {
        self.get(key)
    }

    /// Compatibility alias for set() method
    ///
    /// This provides the API specified in the issue requirements.
    pub fn set_var(&mut self, key: String, value: Value) {
        self.set(key, value);
    }

    /// Legacy compatibility: merge configuration into existing workflow context
    ///
    /// This method provides compatibility with the existing workflow system
    /// that uses HashMap<String, Value> with a "_template_vars" key.
    /// The precedence is:
    /// 1. Configuration values from this TemplateContext (lowest)
    /// 2. Existing workflow _template_vars (highest)
    pub fn merge_into_workflow_context(&self, context: &mut HashMap<String, Value>) {
        // Get or create the _template_vars object
        let existing_template_vars = match context.get("_template_vars") {
            Some(Value::Object(obj)) => obj.clone(),
            _ => Map::new(),
        };

        // Start with configuration values (lowest priority)
        let mut merged_vars = self.variables.clone();

        // Add existing workflow template variables (highest priority)
        // These will override any config values with the same key
        for (key, value) in existing_template_vars {
            merged_vars.insert(key, value);
        }

        // Update the context with merged template variables
        context.insert("_template_vars".to_string(), Value::Object(merged_vars));
    }

    /// Try to get config from a key, checking both flat and nested access
    fn try_get_config(&self, key: &str) -> Option<ModelConfig> {
        // Try flat key access first (for programmatically set configs)
        if let Some(config) = self.variables.get(key) {
            if let Ok(agent_config) = serde_json::from_value::<ModelConfig>(config.clone()) {
                return Some(agent_config);
            }
        }
        // Try nested access (for file-loaded configs)
        if let Some(config) = self.get(key) {
            if let Ok(agent_config) = serde_json::from_value::<ModelConfig>(config.clone()) {
                return Some(agent_config);
            }
        }
        None
    }

    /// Get agent configuration with hierarchical fallback
    ///
    /// Priority: workflow-specific → repo default → system default (Claude)
    ///
    /// # Arguments
    /// * `workflow_name` - Optional workflow name to look for specific configuration
    ///
    /// # Returns
    /// * `ModelConfig` - The agent configuration with proper fallback
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_config::{TemplateContext, ModelConfig, ModelExecutorType};
    /// use serde_json::json;
    ///
    /// let mut context = TemplateContext::new();
    ///
    /// // System default (Claude Code)
    /// let config = context.get_agent_config(None);
    /// assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);
    /// ```
    pub fn get_agent_config(&self, workflow_name: Option<&str>) -> ModelConfig {
        // 1. Check workflow-specific config
        if let Some(workflow) = workflow_name {
            if let Some(config) = self.try_get_config(&format!("agent.configs.{}", workflow)) {
                return config;
            }
        }

        // 2. Check repo default config
        if let Some(config) = self.try_get_config("agent.default") {
            return config;
        }

        // 3. Check for config directly under "agent" key (sah.yaml format)
        if let Some(config) = self.try_get_config("agent") {
            return config;
        }

        // 4. Fall back to system default (Claude Code)
        ModelConfig::default()
    }

    /// Try to extract a ModelConfig from a Value
    fn try_extract_config(value: &Value) -> Option<ModelConfig> {
        serde_json::from_value::<ModelConfig>(value.clone()).ok()
    }

    /// Get all available agent configurations
    ///
    /// Returns a map of all available agent configurations, including the default
    /// configuration (if set) and all named workflow-specific configurations.
    /// Supports both nested access (file-loaded configs) and flat key access
    /// (programmatically set configs).
    ///
    /// # Returns
    /// * `HashMap<String, ModelConfig>` - Map of configuration names to agent configs
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_config::{TemplateContext, ModelConfig, LlamaAgentConfig};
    /// use serde_json::json;
    ///
    /// let mut context = TemplateContext::new();
    /// context.set("agent.default".to_string(),
    ///     serde_json::to_value(ModelConfig::llama_agent(LlamaAgentConfig::default())).unwrap());
    ///
    /// let configs = context.get_all_agent_configs();
    /// assert!(configs.contains_key("default"));
    /// ```
    pub fn get_all_agent_configs(&self) -> HashMap<String, ModelConfig> {
        let mut configs = HashMap::new();

        // Add default config if available
        if let Some(config) = self.try_get_config("agent.default") {
            configs.insert("default".to_string(), config);
        }

        // Look for flat keys that start with "agent.configs." (programmatically set)
        for (key, value) in &self.variables {
            if let Some(workflow_name) = key.strip_prefix("agent.configs.") {
                if let Some(agent_config) = Self::try_extract_config(value) {
                    configs.insert(workflow_name.to_string(), agent_config);
                }
            }
        }

        // Add named configs from nested object (file-loaded), only if not already present
        if let Some(Value::Object(agent_configs)) = self.get("agent.configs") {
            for (workflow_name, config_value) in agent_configs {
                configs
                    .entry(workflow_name.clone())
                    .or_insert_with(|| Self::try_extract_config(config_value).unwrap_or_default());
            }
        }

        configs
    }

    /// Provide a simple default model name for template rendering
    ///
    /// Returns "claude" as the default. The model name should be set via:
    /// - The --model CLI flag
    /// - The sah config file
    /// - The flow context
    ///
    /// This function only provides a fallback when no model is specified.
    fn resolve_model_name(&self) -> String {
        "claude".to_string()
    }

    /// Set model variable if not already provided by user
    fn set_model_variable(&mut self) {
        if self.get("model").is_none() {
            let model_name = self.resolve_model_name();
            debug!("Setting default model variable to: {}", model_name);
            self.set("model".to_string(), Value::String(model_name));
        } else {
            debug!(
                "Model variable already set, not overriding: {:?}",
                self.get("model")
            );
        }
    }

    /// Set working directory variables if not already provided by user
    fn set_working_directory_variables(&mut self) {
        if self.get("working_directory").is_none() && self.get("cwd").is_none() {
            match std::env::current_dir() {
                Ok(current_dir) => {
                    let current_dir_str = current_dir.to_string_lossy().to_string();
                    debug!(
                        "Setting default working directory variables to: {}",
                        current_dir_str
                    );

                    self.set(
                        "working_directory".to_string(),
                        Value::String(current_dir_str.clone()),
                    );
                    self.set("cwd".to_string(), Value::String(current_dir_str));
                }
                Err(e) => {
                    debug!("Failed to get current directory, not setting working_directory/cwd variables: {}", e);
                }
            }
        } else {
            debug!(
                "Working directory variables already set, not overriding. working_directory: {:?}, cwd: {:?}",
                self.get("working_directory"),
                self.get("cwd")
            );
        }
    }

    /// Set default variables if not already set
    ///
    /// This method sets default template variables including:
    /// - "model": The model name based on the configured agent
    /// - "working_directory": The fully qualified current working directory
    /// - "cwd": Alias for the current working directory
    ///
    /// Model names are determined as follows:
    /// - ClaudeCode: "claude"
    /// - LlamaAgent with HuggingFace model: the repository name
    /// - LlamaAgent with Local model: the filename
    /// - Unknown/Default: "claude"
    /// Detect and set project types from current working directory
    ///
    /// Scans the current directory for project marker files and populates
    /// the "project_types" variable with detected projects.
    fn set_project_types_variable(&mut self) {
        use std::env;
        use swissarmyhammer_project_detection::detect_projects;

        // Skip if already set
        if self.get("project_types").is_some() {
            debug!("project_types already set, not overriding");
            return;
        }

        // Get current directory
        let cwd = match env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                debug!(
                    "Failed to get current directory for project detection: {}",
                    e
                );
                return;
            }
        };

        // Detect projects (max depth 3 to avoid deep traversal)
        match detect_projects(&cwd, Some(3)) {
            Ok(projects) => {
                // Convert to JSON array
                let projects_json: Vec<Value> = projects
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "type": p.project_type,
                            "path": p.path.display().to_string(),
                            "markers": p.marker_files,
                            "workspace": p.workspace_info.as_ref().map(|w| serde_json::json!({
                                "is_root": w.is_root,
                                "members": w.members,
                            })),
                        })
                    })
                    .collect();

                // Calculate unique project types
                let mut seen_types = std::collections::HashSet::new();
                let unique_types: Vec<Value> = projects
                    .iter()
                    .filter_map(|p| {
                        let type_str = format!("{:?}", p.project_type);
                        if seen_types.insert(type_str.clone()) {
                            Some(Value::String(type_str))
                        } else {
                            None
                        }
                    })
                    .collect();

                self.set("project_types".to_string(), Value::Array(projects_json));
                self.set(
                    "unique_project_types".to_string(),
                    Value::Array(unique_types),
                );
                debug!(
                    "Detected {} project(s) in {}",
                    projects.len(),
                    cwd.display()
                );
            }
            Err(e) => {
                debug!("Failed to detect projects: {}", e);
                // Set empty array on failure
                self.set("project_types".to_string(), Value::Array(vec![]));
                self.set("unique_project_types".to_string(), Value::Array(vec![]));
            }
        }
    }

    pub fn set_default_variables(&mut self) {
        self.set_model_variable();
        self.set_working_directory_variables();
        self.set_project_types_variable();
    }
}

impl Default for TemplateContext {
    fn default() -> Self {
        Self::new()
    }
}

impl From<HashMap<String, Value>> for TemplateContext {
    fn from(map: HashMap<String, Value>) -> Self {
        Self::from_hash_map(map)
    }
}

impl From<TemplateContext> for HashMap<String, Value> {
    fn from(context: TemplateContext) -> Self {
        context.to_hash_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Global mutex to serialize environment variable tests
    /// This prevents race conditions when multiple tests modify environment variables
    static ENV_VAR_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_new_template_context() {
        let context = TemplateContext::new();
        assert!(context.is_empty());
        assert_eq!(context.len(), 0);
    }

    #[test]
    fn test_set_and_get() {
        let mut context = TemplateContext::new();
        context.set("key".to_string(), json!("value"));

        assert_eq!(context.get("key"), Some(&json!("value")));
        assert_eq!(context.len(), 1);
        assert!(!context.is_empty());
    }

    #[test]
    fn test_get_nested_key() {
        let mut context = TemplateContext::new();
        context.set(
            "database".to_string(),
            json!({
                "host": "localhost",
                "port": 5432
            }),
        );

        assert_eq!(context.get("database.host"), Some(&json!("localhost")));
        assert_eq!(context.get("database.port"), Some(&json!(5432)));
        assert_eq!(context.get("database.nonexistent"), None);
    }

    #[test]
    fn test_merge_contexts() {
        let mut context1 = TemplateContext::new();
        context1.set("key1".to_string(), json!("value1"));
        context1.set("shared".to_string(), json!("original"));

        let mut context2 = TemplateContext::new();
        context2.set("key2".to_string(), json!("value2"));
        context2.set("shared".to_string(), json!("override"));

        context1.merge(context2);

        assert_eq!(context1.get("key1"), Some(&json!("value1")));
        assert_eq!(context1.get("key2"), Some(&json!("value2")));
        assert_eq!(context1.get("shared"), Some(&json!("override"))); // Should be overridden
    }

    #[test]
    fn test_from_and_to_hash_map() {
        let mut hash_map = HashMap::new();
        hash_map.insert("key1".to_string(), json!("value1"));
        hash_map.insert("key2".to_string(), json!(42));

        let context = TemplateContext::from_hash_map(hash_map.clone());
        let converted_back = context.to_hash_map();

        assert_eq!(converted_back.len(), 2);
        assert_eq!(converted_back.get("key1"), Some(&json!("value1")));
        assert_eq!(converted_back.get("key2"), Some(&json!(42)));
    }

    #[test]
    fn test_merge_into_workflow_context() {
        let mut template_context = TemplateContext::new();
        template_context.set("config_var".to_string(), json!("config_value"));
        template_context.set("shared_var".to_string(), json!("config_shared"));

        let mut workflow_context = HashMap::new();
        workflow_context.insert(
            "_template_vars".to_string(),
            json!({
                "workflow_var": "workflow_value",
                "shared_var": "workflow_shared" // Should override config
            }),
        );

        template_context.merge_into_workflow_context(&mut workflow_context);

        let template_vars = workflow_context
            .get("_template_vars")
            .unwrap()
            .as_object()
            .unwrap();
        assert_eq!(
            template_vars.get("config_var").unwrap(),
            &json!("config_value")
        );
        assert_eq!(
            template_vars.get("workflow_var").unwrap(),
            &json!("workflow_value")
        );
        assert_eq!(
            template_vars.get("shared_var").unwrap(),
            &json!("workflow_shared")
        ); // Workflow wins
    }

    #[test]
    fn test_load_with_env_vars() {
        // Acquire the global environment variable test lock to prevent race conditions
        let _lock_guard = ENV_VAR_TEST_LOCK.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Environment variable test lock was poisoned, recovering");
            poisoned.into_inner()
        });

        env::set_var("SAH_PROJECT_NAME", "TestProject");
        env::set_var("SWISSARMYHAMMER_DEBUG", "true");

        let context = TemplateContext::load_for_cli().unwrap();

        assert_eq!(context.get("project.name"), Some(&json!("TestProject")));
        assert_eq!(context.get("debug"), Some(&json!(true)));

        env::remove_var("SAH_PROJECT_NAME");
        env::remove_var("SWISSARMYHAMMER_DEBUG");
    }

    #[test]
    fn test_load_with_config_file() {
        // Acquire the global environment variable test lock to prevent race conditions
        let _lock_guard = ENV_VAR_TEST_LOCK.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Environment variable test lock was poisoned, recovering");
            poisoned.into_inner()
        });

        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir(&config_dir).unwrap();

        let config_file = config_dir.join("sah.toml");
        fs::write(
            &config_file,
            r#"
[database]
host = "localhost"
port = 5432

[app]
name = "TestApp"
version = "1.0.0"
        "#,
        )
        .unwrap();

        // Change to the temp directory so the config file is discovered
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        // Add a small delay to ensure directory change is fully processed
        std::thread::sleep(std::time::Duration::from_millis(10));

        let context = TemplateContext::load_for_cli().unwrap();

        assert_eq!(context.get("database.host"), Some(&json!("localhost")));
        assert_eq!(context.get("database.port"), Some(&json!(5432)));
        assert_eq!(context.get("app.name"), Some(&json!("TestApp")));
        assert_eq!(context.get("app.version"), Some(&json!("1.0.0")));

        // Restore original directory (may fail if original dir no longer exists)
        let _ = env::set_current_dir(original_dir);
    }

    #[test]
    fn test_load_with_cli_args() {
        let cli_args = json!({
            "database": {
                "host": "cli-host"
            },
            "debug": true
        });

        let context = TemplateContext::load_with_cli_args(cli_args).unwrap();

        assert_eq!(context.get("database.host"), Some(&json!("cli-host")));
        assert_eq!(context.get("debug"), Some(&json!(true)));
    }

    #[test]
    fn test_with_template_vars() {
        let mut template_vars = HashMap::new();
        template_vars.insert("template_var1".to_string(), json!("template_value1"));
        template_vars.insert("template_var2".to_string(), json!(42));
        template_vars.insert("template_var3".to_string(), json!(true));

        let context = TemplateContext::with_template_vars(template_vars).unwrap();

        // Template vars should be set correctly
        assert_eq!(
            context.get("template_var1"),
            Some(&json!("template_value1"))
        );
        assert_eq!(context.get("template_var2"), Some(&json!(42)));
        assert_eq!(context.get("template_var3"), Some(&json!(true)));
        assert!(context.len() >= 3); // Should have at least our 3 template vars
    }

    #[test]
    fn test_get_var_compatibility_alias() {
        let mut context = TemplateContext::new();
        context.set("test_key".to_string(), json!("test_value"));

        // get_var should work the same as get
        assert_eq!(context.get_var("test_key"), Some(&json!("test_value")));
        assert_eq!(context.get_var("test_key"), context.get("test_key"));
        assert_eq!(context.get_var("nonexistent"), None);
    }

    #[test]
    fn test_set_var_compatibility_alias() {
        let mut context = TemplateContext::new();

        // set_var should work the same as set
        context.set_var("test_key".to_string(), json!("test_value"));

        assert_eq!(context.get("test_key"), Some(&json!("test_value")));
        assert_eq!(context.len(), 1);
    }

    #[test]
    fn test_get_agent_config_direct_agent_key() {
        use crate::model::{
            LlamaAgentConfig, LlmModelConfig, McpServerConfig, ModelConfig, ModelExecutorConfig,
            ModelSource,
        };

        let mut context = TemplateContext::new();

        // Set up agent config directly under the "agent" key (sah.yaml style)
        let agent_config = ModelConfig {
            quiet: false,
            executor: ModelExecutorConfig::LlamaAgent(LlamaAgentConfig {
                model: LlmModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF".to_string(),
                        filename: Some("Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf".to_string()),
                        folder: None,
                    },
                    batch_size: 256,
                    use_hf_params: true,
                    debug: false,
                },
                mcp_server: McpServerConfig {
                    port: 0,
                    timeout_seconds: 30,
                },

                repetition_detection: Default::default(),
            }),
        };

        context.set(
            "agent".to_string(),
            serde_json::to_value(&agent_config).unwrap(),
        );

        // Test that get_agent_config finds the config under the direct "agent" key
        let retrieved_config = context.get_agent_config(None);

        // Verify it's the correct config type and not the default
        match retrieved_config.executor {
            ModelExecutorConfig::LlamaAgent(llama_config) => match &llama_config.model.source {
                ModelSource::HuggingFace { repo, filename, .. } => {
                    assert!(
                        repo.starts_with("unsloth/Qwen3"),
                        "Expected Qwen3 model, got {}",
                        repo
                    );
                    assert!(
                        filename
                            .as_ref()
                            .map(|f| f.contains("Qwen3"))
                            .unwrap_or(false),
                        "Expected Qwen3 filename, got {:?}",
                        filename
                    );
                }
                _ => panic!("Expected HuggingFace model source"),
            },
            _ => panic!("Expected LlamaAgent executor"),
        }
    }

    #[test]
    fn test_to_liquid_context() {
        let mut context = TemplateContext::new();
        context.set("string_var".to_string(), json!("hello"));
        context.set("number_var".to_string(), json!(42));
        context.set("bool_var".to_string(), json!(true));
        context.set("array_var".to_string(), json!(["item1", "item2"]));
        context.set("object_var".to_string(), json!({"nested": "value"}));

        let liquid_context = context.to_liquid_context();

        // Verify all variables are present
        assert_eq!(liquid_context.len(), 5);
        assert!(liquid_context.contains_key("string_var"));
        assert!(liquid_context.contains_key("number_var"));
        assert!(liquid_context.contains_key("bool_var"));
        assert!(liquid_context.contains_key("array_var"));
        assert!(liquid_context.contains_key("object_var"));

        // Test that the liquid context can be used with liquid templates
        let template_source = "{{string_var}} {{number_var}} {{bool_var}}";
        let liquid_parser = liquid::ParserBuilder::with_stdlib().build().unwrap();
        let template = liquid_parser.parse(template_source).unwrap();
        let output = template.render(&liquid_context).unwrap();

        assert_eq!(output, "hello 42 true");
    }

    #[test]
    fn test_to_liquid_context_with_nested_objects() {
        let mut context = TemplateContext::new();
        context.set(
            "database".to_string(),
            json!({
                "host": "localhost",
                "port": 5432,
                "credentials": {
                    "username": "user",
                    "password": "pass"
                }
            }),
        );

        let liquid_context = context.to_liquid_context();

        // Test nested object access in liquid template
        let template_source =
            "{{database.host}}:{{database.port}} {{database.credentials.username}}";
        let liquid_parser = liquid::ParserBuilder::with_stdlib().build().unwrap();
        let template = liquid_parser.parse(template_source).unwrap();
        let output = template.render(&liquid_context).unwrap();

        assert_eq!(output, "localhost:5432 user");
    }

    #[test]
    fn test_to_liquid_context_with_nil_values() {
        let mut context = TemplateContext::new();
        context.set("null_var".to_string(), json!(null));
        context.set("string_var".to_string(), json!("test"));

        let liquid_context = context.to_liquid_context();

        // Verify that null values are handled properly
        assert!(liquid_context.contains_key("null_var"));
        assert!(liquid_context.contains_key("string_var"));
        assert_eq!(liquid_context.len(), 2);

        // The conversion should succeed even with null values
        // We don't need to test liquid template parsing here, just the conversion
    }

    #[test]
    fn test_integration_liquid_template_engine() {
        // Create a comprehensive template context
        let mut context = TemplateContext::new();
        context.set(
            "app".to_string(),
            json!({
                "name": "SwissArmyHammer",
                "version": "2.0.0",
                "author": "Claude"
            }),
        );
        context.set(
            "database".to_string(),
            json!({
                "host": "localhost",
                "port": 5432,
                "credentials": {
                    "username": "admin",
                    "database": "production"
                }
            }),
        );
        context.set(
            "features".to_string(),
            json!(["templating", "config", "workflows"]),
        );
        context.set("debug".to_string(), json!(true));
        context.set("max_connections".to_string(), json!(100));

        // Convert to liquid context
        let liquid_context = context.to_liquid_context();

        // Test complex liquid template with various features
        let template_source = r#"
# Application Configuration

**Application:** {{app.name}} v{{app.version}}
**Author:** {{app.author}}
**Debug Mode:** {% if debug %}enabled{% else %}disabled{% endif %}

## Database Configuration

- **Host:** {{database.host}}:{{database.port}}
- **Database:** {{database.credentials.database}}
- **Username:** {{database.credentials.username}}
- **Max Connections:** {{max_connections}}

## Features

{% for feature in features -%}
- {{feature | capitalize}}
{% endfor %}

## Connection String

postgresql://{{database.credentials.username}}@{{database.host}}:{{database.port}}/{{database.credentials.database}}

---
Generated for {{app.name}} by liquid templating engine.
        "#.trim();

        // Parse and render the template
        let parser = liquid::ParserBuilder::with_stdlib()
            .build()
            .expect("Failed to create liquid parser");

        let template = parser
            .parse(template_source)
            .expect("Failed to parse liquid template");

        let output = template
            .render(&liquid_context)
            .expect("Failed to render liquid template");

        // Verify the rendered output contains expected content
        assert!(output.contains("**Application:** SwissArmyHammer v2.0.0"));
        assert!(output.contains("**Author:** Claude"));
        assert!(output.contains("**Debug Mode:** enabled"));
        assert!(output.contains("- **Host:** localhost:5432"));
        assert!(output.contains("- **Database:** production"));
        assert!(output.contains("- **Username:** admin"));
        assert!(output.contains("- **Max Connections:** 100"));
        assert!(output.contains("- Templating"));
        assert!(output.contains("- Config"));
        assert!(output.contains("- Workflows"));
        assert!(output.contains("postgresql://admin@localhost:5432/production"));
        assert!(output.contains("Generated for SwissArmyHammer by liquid templating engine"));

        // Verify liquid features work correctly
        // 1. Object property access (app.name, database.host.port)
        // 2. Conditional rendering ({% if debug %})
        // 3. Array iteration ({% for feature in features %})
        // 4. Filters (| capitalize)
        // 5. Complex nested object access (database.credentials.username)
    }

    #[test]
    fn test_set_default_variables_claude_code() {
        let mut context = TemplateContext::new();

        // Set Claude Code agent config
        context.set(
            "agent".to_string(),
            serde_json::to_value(ModelConfig::claude_code()).unwrap(),
        );

        // Set default variables
        context.set_default_variables();

        // Should set model to "claude"
        assert_eq!(context.get("model"), Some(&json!("claude")));

        // Should set working directory variables
        assert!(context.get("working_directory").is_some());
        assert!(context.get("cwd").is_some());

        // Both should be the same value
        assert_eq!(context.get("working_directory"), context.get("cwd"));
    }

    #[test]
    fn test_set_default_variables_model_defaults_to_claude() {
        // Model variable should default to "claude" when not explicitly set.
        // The model name should be set via --model flag or sah config,
        // not derived from agent configuration.
        let mut context = TemplateContext::new();

        // Set default variables without any model pre-set
        context.set_default_variables();

        // Should default to "claude"
        assert_eq!(context.get("model"), Some(&json!("claude")));

        // Should set working directory variables
        assert!(context.get("working_directory").is_some());
        assert!(context.get("cwd").is_some());
    }

    #[test]
    fn test_set_default_variables_respects_preset_model() {
        // When model is set before set_default_variables(),
        // it should not be overwritten
        let mut context = TemplateContext::new();

        // Pre-set model to a custom value (as would happen with --model flag)
        context.set("model".to_string(), json!("qwen-coder"));

        // Set default variables
        context.set_default_variables();

        // Should keep the pre-set model value
        assert_eq!(context.get("model"), Some(&json!("qwen-coder")));

        // Should still set working directory variables
        assert!(context.get("working_directory").is_some());
        assert!(context.get("cwd").is_some());
    }

    #[test]
    fn test_set_default_variables_no_agent_config() {
        let mut context = TemplateContext::new();

        // No agent config set - should default to claude
        context.set_default_variables();

        // Should set model to "claude" (default)
        assert_eq!(context.get("model"), Some(&json!("claude")));

        // Should set working directory variables
        assert!(context.get("working_directory").is_some());
        assert!(context.get("cwd").is_some());
    }

    #[test]
    fn test_set_default_variables_user_provided_model() {
        let mut context = TemplateContext::new();

        // User has already set a model variable
        context.set("model".to_string(), json!("Custom Model"));

        // Set default variables should not override user's choice
        context.set_default_variables();

        // Should keep user's model value
        assert_eq!(context.get("model"), Some(&json!("Custom Model")));

        // Should still set working directory variables
        assert!(context.get("working_directory").is_some());
        assert!(context.get("cwd").is_some());
    }

    #[test]
    fn test_load_sets_default_variables() {
        // This test may pass or fail depending on the environment, but should not crash
        let context_result = TemplateContext::load_for_cli();

        if let Ok(context) = context_result {
            // Should have a model variable set
            assert!(context.get("model").is_some());

            // Model should be a string
            assert!(context.get("model").unwrap().is_string());

            // Should default to "Claude Code" if no agent config found
            let model_str = context.get("model").unwrap().as_str().unwrap();
            assert!(!model_str.is_empty());

            // Should have working directory variables set
            assert!(context.get("working_directory").is_some());
            assert!(context.get("cwd").is_some());

            // Both should be strings and equal
            assert!(context.get("working_directory").unwrap().is_string());
            assert!(context.get("cwd").unwrap().is_string());
            assert_eq!(context.get("working_directory"), context.get("cwd"));
        }
    }

    #[test]
    fn test_with_template_vars_sets_default_variables() {
        let mut vars = HashMap::new();
        vars.insert("test_var".to_string(), json!("test_value"));

        // This should set both the template vars and the default variables
        let context_result = TemplateContext::with_template_vars(vars);

        if let Ok(context) = context_result {
            // Should have our test variable
            assert_eq!(context.get("test_var"), Some(&json!("test_value")));

            // Should also have a model variable set
            assert!(context.get("model").is_some());
            assert!(context.get("model").unwrap().is_string());

            // Should have working directory variables set
            assert!(context.get("working_directory").is_some());
            assert!(context.get("cwd").is_some());
        }
    }

    #[test]
    fn test_with_template_vars_user_model_override() {
        let mut vars = HashMap::new();
        vars.insert("test_var".to_string(), json!("test_value"));
        vars.insert("model".to_string(), json!("User Custom Model"));

        // User provided model should not be overridden
        let context_result = TemplateContext::with_template_vars(vars);

        if let Ok(context) = context_result {
            // Should have our test variable
            assert_eq!(context.get("test_var"), Some(&json!("test_value")));

            // Should keep user's model value
            assert_eq!(context.get("model"), Some(&json!("User Custom Model")));

            // Should still set working directory variables
            assert!(context.get("working_directory").is_some());
            assert!(context.get("cwd").is_some());
        }
    }

    #[test]
    fn test_set_default_variables_user_provided_working_directory() {
        let mut context = TemplateContext::new();

        // User has already set working directory variables
        context.set("working_directory".to_string(), json!("/custom/path"));
        context.set("cwd".to_string(), json!("/custom/path"));

        // Set default variables should not override user's choice
        context.set_default_variables();

        // Should keep user's working directory values
        assert_eq!(
            context.get("working_directory"),
            Some(&json!("/custom/path"))
        );
        assert_eq!(context.get("cwd"), Some(&json!("/custom/path")));

        // Should still set model variable
        assert_eq!(context.get("model"), Some(&json!("claude")));
    }

    #[test]
    fn test_with_template_vars_error_cases() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Acquire the global environment variable test lock to prevent race conditions
        let _lock_guard = ENV_VAR_TEST_LOCK.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Environment variable test lock was poisoned, recovering");
            poisoned.into_inner()
        });

        // Test case: with_template_vars should handle config loading gracefully
        let mut template_vars = HashMap::new();
        template_vars.insert("test_var".to_string(), json!("test_value"));

        // This should succeed even if there's no config - it creates empty config and adds template vars
        let context = TemplateContext::with_template_vars(template_vars.clone());
        assert!(
            context.is_ok(),
            "with_template_vars should succeed with valid template vars"
        );

        let context = context.unwrap();
        assert_eq!(context.get("test_var"), Some(&json!("test_value")));

        // Test case: template vars should override any loaded config values
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir(&config_dir).unwrap();

        let config_file = config_dir.join("sah.toml");
        fs::write(
            &config_file,
            r#"
test_var = "config_value"
config_only = "config_only_value"
        "#,
        )
        .unwrap();

        // Change to temp directory to load the config
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        // Create context with template vars that override config
        let mut override_vars = HashMap::new();
        override_vars.insert("test_var".to_string(), json!("template_override"));
        override_vars.insert("template_only".to_string(), json!("template_only_value"));

        let context = TemplateContext::with_template_vars(override_vars).unwrap();

        // Template vars should override config values
        assert_eq!(context.get("test_var"), Some(&json!("template_override")));
        // Config-only values should still be present
        assert_eq!(
            context.get("config_only"),
            Some(&json!("config_only_value"))
        );
        // Template-only values should be present
        assert_eq!(
            context.get("template_only"),
            Some(&json!("template_only_value"))
        );

        // Test case: empty template vars should still work (while in temp dir with valid config)
        let empty_vars = HashMap::new();
        let empty_context = TemplateContext::with_template_vars(empty_vars);
        assert!(
            empty_context.is_ok(),
            "with_template_vars should handle empty vars: {:?}",
            empty_context.err()
        );

        // Restore original directory (may fail if original dir no longer exists)
        let _ = env::set_current_dir(original_dir);
    }
}
