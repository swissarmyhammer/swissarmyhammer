//! Prompt new command - scaffold a new prompt from a template.

use std::path::PathBuf;

use anyhow::{bail, Result};

use super::cli::NewCommand;

/// Execute the new command: create a scaffold prompt file.
///
/// By default, calls the configured LLM to generate a rich scaffold.
/// Falls back to a static template if the model is unavailable or `--static` is set.
pub async fn execute_new_command(
    cmd: NewCommand,
    context: &crate::context::CliContext,
) -> Result<()> {
    // Validate name: kebab-case, alphanumeric + hyphens, 1-64 chars
    if !is_valid_prompt_name(&cmd.name) {
        bail!(
            "Invalid prompt name '{}'. Must be lowercase, alphanumeric with hyphens, 1-64 chars.",
            cmd.name
        );
    }

    // Determine target directory
    let target_dir = if cmd.user {
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".prompts")
    } else {
        // Use git root if available, otherwise current directory
        swissarmyhammer_common::utils::find_git_repository_root()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join(".prompts")
    };

    // Ensure directory exists
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create {}: {}", target_dir.display(), e))?;

    let file_path = target_dir.join(format!("{}.md", cmd.name));

    if file_path.exists() {
        bail!(
            "Prompt already exists: {}\nUse 'sah prompt edit {}' to modify it.",
            file_path.display(),
            cmd.name
        );
    }

    if cmd.r#static {
        let content = generate_static_scaffold(&cmd.name);
        std::fs::write(&file_path, &content)
            .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", file_path.display(), e))?;
    } else {
        match generate_ai_scaffold(&cmd.name, &file_path, context).await {
            Ok(()) => {}
            Err(e) => {
                eprintln!("AI generation failed ({}), using static template.", e);
                let content = generate_static_scaffold(&cmd.name);
                std::fs::write(&file_path, &content).map_err(|e| {
                    anyhow::anyhow!("Failed to write {}: {}", file_path.display(), e)
                })?;
            }
        }
    };

    let scope = if cmd.user { "user" } else { "project" };
    println!("Created {} prompt '{}':", scope, cmd.name);
    println!("  {}", file_path.display());
    println!();
    println!("Next steps:");
    println!("  sah prompt edit {}    # edit the prompt", cmd.name);
    println!("  sah prompt show {}    # view prompt details", cmd.name);
    println!("  sah prompt render {}  # render with variables", cmd.name);

    Ok(())
}

/// Generate a minimal static scaffold with just frontmatter.
fn generate_static_scaffold(name: &str) -> String {
    let title = kebab_to_title_case(name);
    let description = kebab_to_words(name);
    format!(
        "---\ntitle: {title}\ndescription: {description} prompt\n---\n\n",
        title = title,
        description = description,
    )
}

/// Call the configured LLM agent to generate a rich prompt scaffold.
///
/// Uses the builtin `scaffold-prompt` template as the system prompt,
/// sends it to an ephemeral agent that writes the file via its tools.
async fn generate_ai_scaffold(
    name: &str,
    file_path: &std::path::Path,
    context: &crate::context::CliContext,
) -> Result<()> {
    let title = kebab_to_title_case(name);

    // Render the scaffold-prompt builtin as the system prompt
    let mut render_context = context.template_context.clone();
    render_context.set_var("prompt_name".to_string(), name.into());
    render_context.set_var("prompt_title".to_string(), title.into());
    render_context.set_var(
        "file_path".to_string(),
        file_path.display().to_string().into(),
    );

    let system_prompt = context
        .prompt_library
        .render("scaffold-prompt", &render_context)?;

    let user_prompt = format!(
        "Create a prompt template named \"{}\" (title: \"{}\"). Write it to: {}",
        name,
        kebab_to_title_case(name),
        file_path.display(),
    );

    let config = context.template_context.get_agent_config(None);
    let options = swissarmyhammer_workflow::CreateAgentOptions { ephemeral: true };
    let mut agent =
        swissarmyhammer_workflow::create_agent_with_options(&config, None, options).await?;

    swissarmyhammer_workflow::execute_prompt(&mut agent, Some(system_prompt), None, user_prompt)
        .await?;

    if !file_path.exists() {
        bail!(
            "AI agent did not create the file at {}",
            file_path.display()
        );
    }

    Ok(())
}

/// Validate that a prompt name is valid (kebab-case, 1-64 chars).
fn is_valid_prompt_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// Convert kebab-case to Title Case: "code-review" -> "Code Review"
fn kebab_to_title_case(name: &str) -> String {
    name.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert kebab-case to space-separated words: "code-review" -> "code review"
fn kebab_to_words(name: &str) -> String {
    name.replace('-', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_prompt_names() {
        assert!(is_valid_prompt_name("code-review"));
        assert!(is_valid_prompt_name("a"));
        assert!(is_valid_prompt_name("my-prompt-123"));
        assert!(is_valid_prompt_name("hello"));
    }

    #[test]
    fn test_invalid_prompt_names() {
        assert!(!is_valid_prompt_name(""));
        assert!(!is_valid_prompt_name("-starts-with-hyphen"));
        assert!(!is_valid_prompt_name("ends-with-hyphen-"));
        assert!(!is_valid_prompt_name("HAS_UPPER"));
        assert!(!is_valid_prompt_name("has spaces"));
        assert!(!is_valid_prompt_name("has_underscores"));
        let long = "a".repeat(65);
        assert!(!is_valid_prompt_name(&long));
    }

    #[test]
    fn test_kebab_to_title_case() {
        assert_eq!(kebab_to_title_case("code-review"), "Code Review");
        assert_eq!(kebab_to_title_case("hello"), "Hello");
        assert_eq!(
            kebab_to_title_case("my-long-prompt-name"),
            "My Long Prompt Name"
        );
    }

    #[test]
    fn test_kebab_to_words() {
        assert_eq!(kebab_to_words("code-review"), "code review");
        assert_eq!(kebab_to_words("hello"), "hello");
    }

    #[test]
    fn test_generate_static_scaffold() {
        let content = generate_static_scaffold("code-review");
        assert!(content.starts_with("---\n"));
        assert!(content.contains("title: Code Review"));
        assert!(content.contains("description: code review prompt"));
        assert!(content.contains("---\n"));
    }
}
