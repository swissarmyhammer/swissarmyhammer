use crate::cli_builder::CliBuilder;
use anyhow::Result;
use clap_complete::{generate_to, Shell};
use std::io;
use std::path::Path;
use std::sync::Arc;


/// Generate shell completion scripts using dynamic CLI
pub async fn generate_completions<P: AsRef<Path>>(outdir: P) -> Result<()> {
    let outdir = outdir.as_ref();
    
    let mut cli = build_dynamic_cli().await?;

    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        generate_to(shell, &mut cli, "swissarmyhammer", outdir)?;
    }

    println!("Generated shell completions in: {}", outdir.display());

    Ok(())
}

/// Print shell completion script to stdout using dynamic CLI
pub async fn print_completion(shell: Shell) -> Result<()> {
    let mut cli = build_dynamic_cli().await?;
    
    clap_complete::generate(shell, &mut cli, "swissarmyhammer", &mut io::stdout());

    Ok(())
}

/// Build dynamic CLI with MCP tools
async fn build_dynamic_cli() -> Result<clap::Command> {
    use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    use swissarmyhammer_tools::*;
    
    // Create the tool registry and register tools
    let mut tool_registry = ToolRegistry::new();
    
    // Register all tools 
    register_file_tools(&mut tool_registry);
    register_issue_tools(&mut tool_registry);
    register_memo_tools(&mut tool_registry);
    register_notify_tools(&mut tool_registry);
    register_search_tools(&mut tool_registry);
    register_shell_tools(&mut tool_registry);
    register_todo_tools(&mut tool_registry);
    register_web_fetch_tools(&mut tool_registry);
    register_web_search_tools(&mut tool_registry);
    
    let tool_registry = Arc::new(tool_registry);
    
    // Build CLI using the CLI builder
    let cli_builder = CliBuilder::new(tool_registry);
    cli_builder.build_cli()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_completions_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = generate_completions(temp_dir.path()).await;

        assert!(result.is_ok(), "generate_completions should succeed");

        // Check that completion files were created for each shell
        let bash_completion = temp_dir.path().join("swissarmyhammer.bash");
        let zsh_completion = temp_dir.path().join("_swissarmyhammer");
        let fish_completion = temp_dir.path().join("swissarmyhammer.fish");
        let ps_completion = temp_dir.path().join("_swissarmyhammer.ps1");

        assert!(
            bash_completion.exists(),
            "Bash completion file should exist"
        );
        assert!(zsh_completion.exists(), "Zsh completion file should exist");
        assert!(
            fish_completion.exists(),
            "Fish completion file should exist"
        );
        assert!(
            ps_completion.exists(),
            "PowerShell completion file should exist"
        );

        // Verify files are not empty
        let bash_content = std::fs::read_to_string(&bash_completion).unwrap();
        assert!(
            !bash_content.is_empty(),
            "Bash completion should not be empty"
        );
        assert!(
            bash_content.contains("swissarmyhammer"),
            "Completion should contain program name"
        );
    }

    #[tokio::test]
    async fn test_bash_completion_generation() {
        let mut cli = build_dynamic_cli().await.unwrap();
        
        // Test that completion generation doesn't panic
        let mut output = Vec::new();
        clap_complete::generate(Shell::Bash, &mut cli, "swissarmyhammer", &mut output);
        
        let completion_script = String::from_utf8(output).unwrap();
        
        // Verify completion script contains expected commands
        assert!(completion_script.contains("issue"));
        assert!(completion_script.contains("memo"));
        assert!(completion_script.contains("file"));
        assert!(completion_script.contains("create"));
        assert!(completion_script.contains("list"));
    }
    
    #[tokio::test]
    async fn test_completion_for_all_shells() {
        let shells = vec![
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
        ];
        
        for shell in shells {
            let mut cli = build_dynamic_cli().await.unwrap();
            let mut output = Vec::new();
            clap_complete::generate(shell, &mut cli, "swissarmyhammer", &mut output);
            
            // Verify each shell generates non-empty completion
            assert!(!output.is_empty(), "Shell {:?} generated empty completion", shell);
        }
    }

    #[tokio::test]
    async fn test_print_completion_bash() {
        // Capture stdout
        let mut output = Vec::new();
        let mut cli = build_dynamic_cli().await.unwrap();
        clap_complete::generate(Shell::Bash, &mut cli, "swissarmyhammer", &mut output);

        let output_str = String::from_utf8(output).unwrap();
        assert!(
            !output_str.is_empty(),
            "Bash completion output should not be empty"
        );
        assert!(
            output_str.contains("swissarmyhammer"),
            "Output should contain program name"
        );
        assert!(
            output_str.contains("complete"),
            "Bash completion should contain 'complete' command"
        );
    }

    #[tokio::test]
    async fn test_print_completion_zsh() {
        // Capture stdout
        let mut output = Vec::new();
        let mut cli = build_dynamic_cli().await.unwrap();
        clap_complete::generate(Shell::Zsh, &mut cli, "swissarmyhammer", &mut output);

        let output_str = String::from_utf8(output).unwrap();
        assert!(
            !output_str.is_empty(),
            "Zsh completion output should not be empty"
        );
        assert!(
            output_str.contains("swissarmyhammer"),
            "Output should contain program name"
        );
        assert!(
            output_str.contains("#compdef"),
            "Zsh completion should contain compdef directive"
        );
    }

    #[tokio::test]
    async fn test_print_completion_fish() {
        // Capture stdout
        let mut output = Vec::new();
        let mut cli = build_dynamic_cli().await.unwrap();
        clap_complete::generate(Shell::Fish, &mut cli, "swissarmyhammer", &mut output);

        let output_str = String::from_utf8(output).unwrap();
        assert!(
            !output_str.is_empty(),
            "Fish completion output should not be empty"
        );
        assert!(
            output_str.contains("swissarmyhammer"),
            "Output should contain program name"
        );
        assert!(
            output_str.contains("complete -c swissarmyhammer"),
            "Fish completion should contain complete command"
        );
    }

    #[tokio::test]
    async fn test_completion_includes_flags() {
        // Test that completions include global flags
        let mut output = Vec::new();
        let mut cli = build_dynamic_cli().await.unwrap();
        clap_complete::generate(Shell::Bash, &mut cli, "swissarmyhammer", &mut output);

        let output_str = String::from_utf8(output).unwrap();

        // Check for global flags
        assert!(
            output_str.contains("--help") || output_str.contains("-h"),
            "Completion should include help flag"
        );
        assert!(
            output_str.contains("--verbose") || output_str.contains("-v"),
            "Completion should include verbose flag"
        );
        assert!(
            output_str.contains("--quiet") || output_str.contains("-q"),
            "Completion should include quiet flag"
        );
    }

    #[tokio::test]
    async fn test_print_completion_function() {
        // Test the actual print_completion function
        // We can't easily capture stdout in tests, so we just verify it doesn't panic
        assert!(print_completion(Shell::Bash).await.is_ok());
        assert!(print_completion(Shell::Zsh).await.is_ok());
        assert!(print_completion(Shell::Fish).await.is_ok());
        assert!(print_completion(Shell::PowerShell).await.is_ok());
    }

    #[tokio::test]
    async fn test_dynamic_commands_in_completions() {
        let mut cli = build_dynamic_cli().await.unwrap();
        let mut output = Vec::new();
        clap_complete::generate(Shell::Bash, &mut cli, "swissarmyhammer", &mut output);

        let output_str = String::from_utf8(output).unwrap();

        // Check for dynamic commands from MCP tools
        assert!(
            output_str.contains("issue"),
            "Completion should include issue command"
        );
        assert!(
            output_str.contains("memo"),
            "Completion should include memo command"
        );
        assert!(
            output_str.contains("file"),
            "Completion should include file command"
        );
        assert!(
            output_str.contains("search"),
            "Completion should include search command"
        );
    }
}
