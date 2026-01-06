# Model Management API and CLI Specification

## Overview

Add a new top-level `sah model` command to manage built-in model configurations. The command will discover model configurations from embedded resources and allow users to easily switch between them.

## Requirements

### CLI Interface

```bash
# List all available built-in models
sah model list

# Use a specific model configuration
sah model use <model_name>
```

### Built-in Model Compilation

Built-in models need to be compiled into the binary as embedded resources, similar to prompts and workflows:

#### Build Script Integration

Add to `swissarmyhammer-config/build.rs`:

```rust
fn generate_builtin_models(out_dir: &str) {
    let dest_path = Path::new(&out_dir).join("builtin_models.rs");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let builtin_dir = Path::new(&manifest_dir).join("../builtin/models");

    let mut code = String::new();
    code.push_str("// Auto-generated builtin models - do not edit manually\n");
    code.push_str("/// Get all built-in models as a vector of (name, content) tuples\n");
    code.push_str("pub fn get_builtin_models() -> Vec<(&'static str, &'static str)> {\n");
    code.push_str("    vec![\n");

    if builtin_dir.exists() {
        for entry in fs::read_dir(&builtin_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let name = path.file_stem().unwrap().to_str().unwrap();
                code.push_str(&format!(
                    "        (\"{}\", include_str!(\"{}\")),\n",
                    name,
                    path.display()
                ));
            }
        }
    }
    
    code.push_str("    ]\n");
    code.push_str("}\n");
    
    fs::write(&dest_path, code).unwrap();
}
```

### Model Discovery Hierarchy

Following the same pattern as prompts and workflows, models are discovered in this order:

1. **User models** (highest precedence) - `.swissarmyhammer/models/`
2. **Project models** - `models/` directory in project root
3. **Built-in models** (lowest precedence) - compiled into binary from `builtin/models/`

#### Built-in Model Discovery

- Models compiled into binary from `builtin/models/` directory at build time
- Use file stem (filename without extension) as model name
- Example: `builtin/models/qwen-coder.yaml` → model name `qwen-coder`
- Access via `include!(concat!(env!("OUT_DIR"), "/builtin_models.rs"));`

#### User Model Discovery

- Scan `.swissarmyhammer/models/` for `.yaml` files
- Allow users to override built-in models with same name
- Enable custom model configurations without modifying project files

### Configuration Management

- Check for existing config in order:
  1. `.swissarmyhammer/sah.yaml`
  2. `.swissarmyhammer/sah.toml`
- If neither exists, create `.swissarmyhammer/sah.yaml`
- Replace the `model:` section in the config file with the selected built-in model configuration

### API Design (swissarmyhammer-config)

```rust
// Include generated builtin models
include!(concat!(env!("OUT_DIR"), "/builtin_models.rs"));

// Model management operations
pub struct ModelManager;

impl ModelManager {
    /// List all available models from all sources
    pub fn list_models() -> Result<Vec<ModelInfo>, ModelError> {
        let mut models = Vec::new();
        
        // 1. Load built-in models (lowest precedence)
        for (name, content) in get_builtin_models() {
            let description = parse_model_description(content);
            models.push(Self::create_model_info(name, content, ModelSource::Builtin, description));
        }
        
        // 2. Load project models (medium precedence)
        if let Ok(project_models) = load_project_models() {
            Self::merge_models(&mut models, project_models);
        }
        
        // 3. Load user models (highest precedence)
        if let Ok(user_models) = load_user_models() {
            Self::merge_models(&mut models, user_models);
        }
        
        Ok(models)
    }
    
    /// Create a ModelInfo instance with consistent field mapping
    fn create_model_info(name: &str, content: &str, source: ModelSource, description: Option<String>) -> ModelInfo {
        ModelInfo {
            name: name.to_string(),
            content: content.to_string(),
            source,
            description,
        }
    }
    
    /// Merge new models into existing list, replacing by name if exists
    fn merge_models(existing: &mut Vec<ModelInfo>, new_models: Vec<ModelInfo>) {
        for model in new_models {
            if let Some(existing_model) = existing.iter_mut().find(|a| a.name == model.name) {
                *existing_model = model;
            } else {
                existing.push(model);
            }
        }
    }
    
    /// Apply a model configuration to the project config
    pub fn use_model(model_name: &str) -> Result<(), ModelError> {
        // Find the model by name from all sources
        let models = Self::list_models()?;
        let model = models
            .iter()
            .find(|a| a.name == model_name)
            .ok_or_else(|| ModelError::NotFound(model_name.to_string()))?;
        
        // Parse the model configuration
        let model_config: ModelConfig = serde_yaml::from_str(&model.content)?;
        
        // Find or create project config file
        // Replace model section
        // Write back to disk
    }
    
    /// Load models from .swissarmyhammer/models/
    fn load_user_models() -> Result<Vec<ModelInfo>, ModelError> {
        let models_dir = Path::new(".swissarmyhammer/models");
        Self::load_models_from_dir(models_dir, ModelSource::User)
    }
    
    /// Load models from models/ directory in project root
    fn load_project_models() -> Result<Vec<ModelInfo>, ModelError> {
        let models_dir = Path::new("models");
        Self::load_models_from_dir(models_dir, ModelSource::Project)
    }
    
    /// Load models from a specific directory
    fn load_models_from_dir(dir: &Path, source: ModelSource) -> Result<Vec<ModelInfo>, ModelError> {
        let mut models = Vec::new();
        
        if !dir.exists() {
            return Ok(models);
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| ModelError::InvalidPath(path.clone()))?;
                
                let content = fs::read_to_string(&path)?;
                let description = parse_model_description(&content);
                
                models.push(ModelInfo {
                    name: name.to_string(),
                    content,
                    source,
                    description,
                });
            }
        }
        
        Ok(models)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelSource {
    Builtin,
    Project,
    User,
}

pub struct ModelInfo {
    pub name: String,
    pub content: String,
    pub source: ModelSource,
    pub description: Option<String>,
}
```

### CLI Implementation (Thin Wrapper)

The CLI should be minimal, delegating all logic to the swissarmyhammer-config API and following the same output formatting pattern as other commands:

```rust
// In main CLI
match args.command {
    Command::Model { subcommand } => match subcommand {
        ModelSubcommand::List { format } => {
            let models = ModelManager::list_models()?;
            display_models(&models, format.unwrap_or(OutputFormat::Table))?;
        }
        ModelSubcommand::Use { model_name } => {
            ModelManager::use_model(&model_name)?;
            println!("Successfully switched to model: {}", model_name);
        }
    }
}

/// Display models using consistent formatting with other commands
fn display_models(models: &[ModelInfo], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(models)?;
            println!("{json}");
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(models)?;
            print!("{yaml}");
        }
        OutputFormat::Table => {
            display_models_table(models)?;
        }
    }
    Ok(())
}

/// Get color pair for source type (name_color, description_color)
fn get_source_color(source: &ModelSource) -> (Color, Color) {
    match source {
        ModelSource::Builtin => (Color::Green, Color::Green),
        ModelSource::User => (Color::Blue, Color::Blue),
        ModelSource::Project => (Color::Yellow, Color::Yellow),
    }
}

/// Display models in table format following the same pattern as prompts
fn display_models_table(models: &[ModelInfo]) -> Result<()> {
    if models.is_empty() {
        println!("No models found.");
        return Ok(());
    }

    let builtin_models: Vec<_> = models.iter().filter(|a| matches!(a.source, ModelSource::Builtin)).collect();
    let project_models: Vec<_> = models.iter().filter(|a| matches!(a.source, ModelSource::Project)).collect();
    let user_models: Vec<_> = models.iter().filter(|a| matches!(a.source, ModelSource::User)).collect();

    let total_models = models.len();
    let builtin_count = builtin_models.len();
    let project_count = project_models.len();
    let user_count = user_models.len();

    println!("🤖 Models: {total_models} total");
    println!("📦 Built-in: {builtin_count}");
    if project_count > 0 {
        println!("📁 Project: {project_count}");
    }
    if user_count > 0 {
        println!("👤 User: {user_count}");
    }
    println!();

    // Create a custom 2-line format like prompts
    let is_tty = atty::is(atty::Stream::Stdout);
    
    for model in models {
        let description = model.description.as_deref().unwrap_or("");

        // First line: Name | Description (colored by source)
        let first_line = if is_tty {
            let (name_color, desc_color) = get_source_color(&model.source);
            let name_colored = model.name.color(name_color).bold().to_string();
            let desc_colored = description.color(desc_color).to_string();
            if description.is_empty() {
                name_colored
            } else {
                format!("{} | {}", name_colored, desc_colored)
            }
        } else {
            if description.is_empty() {
                model.name.clone()
            } else {
                format!("{} | {}", model.name, description)
            }
        };

        // Second line: Source and executor info
        let executor_info = format!("source: {:?}", model.source).to_lowercase();
        let second_line = if is_tty {
            executor_info.dimmed().to_string()
        } else {
            executor_info
        };

        println!("{}", first_line);
        println!("  {}", second_line);
        println!(); // Blank line between entries
    }

    Ok(())
}
```

## Implementation Notes

1. **Error Handling**: Graceful handling of missing files, invalid configs, permission errors
2. **Validation**: Validate built-in model configs before applying
3. **Backup**: Consider backing up existing config before replacement
4. **Feedback**: Clear success/error messages for CLI users
5. **Discovery**: Recursive scanning if nested directories are needed later

## Future Enhancements

### Additional CLI Commands

```bash
# Display model configuration details
sah model show <model_name>

# Show currently active model
sah model current

# Validate model configuration
sah model validate <model_name>
```

### Enhanced API Design

```rust
impl ModelManager {
    /// Find a built-in model by name
    fn find_builtin_model(model_name: &str) -> Result<&'static str, ModelError> {
        get_builtin_models()
            .iter()
            .find(|(name, _)| *name == model_name)
            .map(|(_, content)| *content)
            .ok_or_else(|| ModelError::NotFound(model_name.to_string()))
    }
    
    /// Show details of a specific built-in model
    pub fn show_model(model_name: &str) -> Result<ModelDetails, ModelError> {
        let model_content = Self::find_builtin_model(model_name)?;
        let model_config: ModelConfig = serde_yaml::from_str(model_content)?;
        
        Ok(ModelDetails {
            name: model_name.to_string(),
            content: model_content.to_string(),
            config: model_config,
            executor_type: model_config.executor_type(),
            description: parse_model_description(model_content),
        })
    }
    
    /// Get the currently active model from project config
    pub fn get_current_model() -> Result<Option<CurrentModel>, ModelError> {
        // Read project config file
        // Parse model section
        // Return current model info
    }
    
    /// Validate a built-in model configuration
    pub fn validate_model(model_name: &str) -> Result<ValidationResult, ModelError> {
        let model_content = Self::find_builtin_model(model_name)?;
        
        // Parse and validate configuration
        let validation_result = validate_model_config(model_content)?;
        Ok(validation_result)
    }
}

pub struct ModelDetails {
    pub name: String,
    pub content: String,
    pub config: ModelConfig,
    pub executor_type: AgentExecutorType,
    pub description: Option<String>,
}

pub struct CurrentModel {
    pub name: Option<String>, // None if using custom config
    pub executor_type: AgentExecutorType,
    pub config_source: ConfigSource, // Builtin, Project, etc.
}

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
```

### Additional Features

- Model configuration validation and linting
- Model comparison functionality
- Export current config as new model template
- Copy built-in model to user directory for customization

### CLI Examples with User Models

```bash
# List all models (builtin, project, user)
sah model list

# Use a user-defined model
sah model use my-custom-model

# Copy built-in model to user directory for customization
sah model copy qwen-coder --to-user

# Show model with source information
sah model show qwen-coder
# Output: Model: qwen-coder (source: user, overrides builtin)
```

### Directory Structure

```
project/
├── .swissarmyhammer/
│   ├── sah.yaml                    # Project config
│   └── models/                     # User models (highest precedence)
│       ├── my-custom-model.yaml
│       └── qwen-coder.yaml         # Overrides builtin qwen-coder
├── models/                         # Project models (medium precedence)
│   └── team-model.yaml
└── builtin/models/                 # Built-in models (compiled in, lowest precedence)
    ├── claude-code.yaml
    ├── qwen-coder.yaml
    └── qwen-next.yaml
```
