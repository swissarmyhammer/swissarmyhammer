Manage and interact with models in the SwissArmyHammer system.

Models provide specialized AI execution environments and configurations for specific
development workflows. They enable you to switch between different AI models, 
execution contexts, and toolchains based on your project's needs.

MODEL DISCOVERY AND PRECEDENCE

Models are loaded from multiple sources with hierarchical precedence:
• Built-in models (lowest precedence) - Embedded in the binary
• Project models (medium precedence) - ./models/*.yaml in your project
• User models (highest precedence) - ~/.swissarmyhammer/models/*.yaml

Higher precedence models override lower ones by name. This allows you to
customize built-in models or create project-specific variants.

BUILT-IN MODELS

The system includes these built-in models:
• claude-code    - Default Claude Code integration with shell execution
• qwen-coder     - Local Qwen3-Coder model with in-process execution

COMMANDS

The model system provides two main commands:
• list - Display all available models from all sources with descriptions
• use - Apply a model configuration to the current project

When you 'use' a model, it creates or updates .swissarmyhammer/sah.yaml in your
project with the model's configuration. This configures how SwissArmyHammer 
executes AI workflows in your project.

COMMON WORKFLOWS

1. Explore available models:
   sah model list

2. Apply a model to your project:
   sah model use claude-code

3. Switch to a different model:
   sah model use qwen-coder

4. View detailed model information:
   sah --verbose model list

Use global arguments to control output:
  --verbose         Show detailed information and descriptions
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode with comprehensive tracing
  --quiet           Suppress output except errors

Examples:
  sah model list                           # List all available models
  sah --verbose model list                 # Show detailed information and descriptions
  sah --format=json model list             # Output as structured JSON
  sah model use claude-code                # Apply Claude Code model to project
  sah model use qwen-coder                 # Switch to local Qwen3-Coder model
  sah --debug model use custom-model       # Apply model with debug output

CUSTOMIZATION

Create custom models by adding .yaml files to:
• ./models/ (project-specific models)
• ~/.swissarmyhammer/models/ (user-wide models)

Custom models can override built-in models by using the same name, or
provide entirely new configurations for specialized workflows.