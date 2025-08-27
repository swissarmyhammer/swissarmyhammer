//! Example showing custom template filters and advanced templating

use serde_json::json;
use std::collections::HashMap;
use swissarmyhammer::{
    common::{Parameter, ParameterType},
    Prompt, PromptLibrary, TemplateEngine,
};
use swissarmyhammer_config::TemplateContext;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a template engine
    let engine = TemplateEngine::new();

    // Example 1: Using built-in filters
    let template1 = r#"
Upper: {{ name | upcase }}
Lower: {{ name | downcase }}
Size: {{ content | size }}
"#;

    let mut args = HashMap::new();
    args.insert(
        "title".to_string(),
        "Hello World! This is a Test".to_string(),
    );
    args.insert("name".to_string(), "SwissArmyHammer".to_string());
    args.insert("content".to_string(), "Line 1\nLine 2\nLine 3".to_string());

    let result = engine.render(template1, &args)?;
    println!("Filters example:\n{result}");

    // Example 3: Complex template with conditionals and loops
    let _template3 = r#"
# Task List

{% for task in tasks %}
- [{% if task.done %}x{% else %} {% endif %}] {{ task.name }}
  {% if task.description %}Description: {{ task.description }}{% endif %}
{% endfor %}

Total tasks: {{ tasks | size }}
"#;

    // For this example, we'll use JSON to pass complex data
    let json_args = serde_json::json!({
        "tasks": [
            {"name": "Setup project", "done": true, "description": "Initialize repository"},
            {"name": "Write tests", "done": true},
            {"name": "Implement features", "done": false, "description": "Core functionality"},
            {"name": "Documentation", "done": false}
        ]
    });

    // Convert JSON to string map (simplified for this example)
    // In real usage, you might want to handle nested structures differently
    let tasks_json = serde_json::to_string(&json_args["tasks"])?;
    let mut args = HashMap::new();
    args.insert("tasks".to_string(), tasks_json);

    // Note: This is a simplified example. The actual template engine
    // would need proper array handling for the 'for' loop to work correctly.

    // Example 4: Using prompts with templates
    let mut library = PromptLibrary::new();

    let prompt = Prompt::new(
        "git-commit",
        r#"
{{ type }}: {{ description }}

{% if body %}
{{ body }}
{% endif %}

{% if breaking_change %}
BREAKING CHANGE: {{ breaking_change }}
{% endif %}

{% if issues %}
Fixes: {{ issues }}
{% endif %}
"#,
    )
    .with_description("Generate conventional commit messages")
    .add_parameter(
        Parameter::new(
            "type",
            "Commit type (feat, fix, docs, etc.)",
            ParameterType::String,
        )
        .required(true),
    )
    .add_parameter(
        Parameter::new("description", "Short description", ParameterType::String).required(true),
    )
    .add_parameter(
        Parameter::new("body", "Detailed explanation", ParameterType::String).required(false),
    )
    .add_parameter(
        Parameter::new(
            "breaking_change",
            "Breaking change description",
            ParameterType::String,
        )
        .required(false),
    )
    .add_parameter(
        Parameter::new("issues", "Related issue names", ParameterType::String).required(false),
    );

    library.add(prompt)?;

    // Use the commit message prompt
    let prompt = library.get("git-commit")?;

    let mut template_vars = HashMap::new();
    template_vars.insert("type".to_string(), json!("feat"));
    template_vars.insert(
        "description".to_string(),
        json!("add library API for prompt management"),
    );
    template_vars.insert("body".to_string(), json!("This commit refactors SwissArmyHammer to expose core functionality as a reusable Rust library. Developers can now integrate prompt management into their own applications."));
    template_vars.insert("issues".to_string(), json!("#123, #456"));

    let template_context = TemplateContext::with_template_vars(template_vars)?;
    let commit_msg = library.render(&prompt.name, &template_context)?;
    println!("\nGenerated commit message:\n{commit_msg}");

    Ok(())
}
