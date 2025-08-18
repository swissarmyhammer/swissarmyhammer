//! Basic usage example for SwissArmyHammer library

use std::collections::HashMap;
use swissarmyhammer::{Prompt, PromptLibrary, common::{Parameter, ParameterType}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new prompt library
    let mut library = PromptLibrary::new();

    // Create a prompt programmatically
    let code_review_prompt = Prompt::new(
        "code-review",
        r#"
Please review the following {{ language }} code:

```{{ language }}
{{ code }}
```

Focus on:
- Code quality and best practices
- Potential bugs or issues
- Performance considerations
- Suggestions for improvement
"#,
    )
    .with_description("A comprehensive code review prompt")
    .with_category("development")
    .with_tags(vec![
        "code".to_string(),
        "review".to_string(),
        "quality".to_string(),
    ])
    .add_parameter(
        Parameter::new("language", "The programming language", ParameterType::String)
            .required(true)
    )
    .add_parameter(
        Parameter::new("code", "The code to review", ParameterType::String)
            .required(true)
    );

    // Add the prompt to the library
    library.add(code_review_prompt)?;

    // Load prompts from a directory (if it exists)
    if std::path::Path::new("./.swissarmyhammer/prompts").exists() {
        let count = library.add_directory("./.swissarmyhammer/prompts")?;
        println!("Loaded {count} prompts from directory");
    }

    // List all available prompts
    println!("Available prompts:");
    for prompt in library.list()? {
        println!(
            "  - {} ({})",
            prompt.name,
            prompt.description.as_deref().unwrap_or("No description")
        );
    }

    // Use a prompt
    let prompt = library.get("code-review")?;

    let mut args = HashMap::new();
    args.insert("language".to_string(), "rust".to_string());
    args.insert(
        "code".to_string(),
        r#"
fn fibonacci(n: u32) -> u32 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}
"#
        .to_string(),
    );

    let rendered = prompt.render(&args)?;
    println!("\nRendered prompt:\n{rendered}");

    Ok(())
}
