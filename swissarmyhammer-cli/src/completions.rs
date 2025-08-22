use crate::cli::Cli;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
#[cfg(not(feature = "dynamic-cli"))]
use std::io;
use std::path::Path;

/// Generate shell completion scripts
#[allow(dead_code)]
pub fn generate_completions<P: AsRef<Path>>(outdir: P) -> Result<()> {
    let outdir = outdir.as_ref();

    let mut cmd = Cli::command();

    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        generate_to(shell, &mut cmd, "swissarmyhammer", outdir)?;
    }

    println!("Generated shell completions in: {}", outdir.display());

    Ok(())
}

/// Print shell completion script to stdout
#[cfg(not(feature = "dynamic-cli"))]
pub fn print_completion(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();

    clap_complete::generate(shell, &mut cmd, "swissarmyhammer", &mut io::stdout());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_completions_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = generate_completions(temp_dir.path());

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

    #[test]
    fn test_print_completion_bash() {
        // Capture stdout
        let mut output = Vec::new();
        {
            let mut cmd = Cli::command();
            clap_complete::generate(Shell::Bash, &mut cmd, "swissarmyhammer", &mut output);
        }

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

    #[test]
    fn test_print_completion_zsh() {
        // Capture stdout
        let mut output = Vec::new();
        {
            let mut cmd = Cli::command();
            clap_complete::generate(Shell::Zsh, &mut cmd, "swissarmyhammer", &mut output);
        }

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

    #[test]
    fn test_print_completion_fish() {
        // Capture stdout
        let mut output = Vec::new();
        {
            let mut cmd = Cli::command();
            clap_complete::generate(Shell::Fish, &mut cmd, "swissarmyhammer", &mut output);
        }

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

    #[test]
    fn test_completion_includes_subcommands() {
        // Test that completions include our subcommands
        let mut output = Vec::new();
        {
            let mut cmd = Cli::command();
            clap_complete::generate(Shell::Bash, &mut cmd, "swissarmyhammer", &mut output);
        }

        let output_str = String::from_utf8(output).unwrap();

        // Check for main commands
        assert!(
            output_str.contains("prompt"),
            "Completion should include 'prompt' command"
        );
        assert!(
            output_str.contains("serve"),
            "Completion should include 'serve' command"
        );
        assert!(
            output_str.contains("doctor"),
            "Completion should include 'doctor' command"
        );
        assert!(
            output_str.contains("completion"),
            "Completion should include 'completion' command"
        );
        // Note: memo commands are only available with dynamic-cli feature
        // assert!(
        //     output_str.contains("memo"),
        //     "Completion should include 'memo' command"
        // );
        // Note: issue commands are only available with dynamic-cli feature
        // assert!(
        //     output_str.contains("issue"),
        //     "Completion should include 'issue' command"
        // );

        // Check for prompt subcommands
        assert!(
            output_str.contains("list"),
            "Completion should include 'list' subcommand"
        );
        assert!(
            output_str.contains("search"),
            "Completion should include 'search' subcommand"
        );
        assert!(
            output_str.contains("validate"),
            "Completion should include 'validate' subcommand"
        );
        assert!(
            output_str.contains("test"),
            "Completion should include 'test' subcommand"
        );
    }

    #[test]
    fn test_completion_includes_flags() {
        // Test that completions include global flags
        let mut output = Vec::new();
        {
            let mut cmd = Cli::command();
            clap_complete::generate(Shell::Bash, &mut cmd, "swissarmyhammer", &mut output);
        }

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

    #[test]
    fn test_print_completion_function() {
        // Test the actual print_completion function
        // We can't easily capture stdout in tests, so we just verify it doesn't panic
        assert!(print_completion(Shell::Bash).is_ok());
        assert!(print_completion(Shell::Zsh).is_ok());
        assert!(print_completion(Shell::Fish).is_ok());
        assert!(print_completion(Shell::PowerShell).is_ok());
    }
}
