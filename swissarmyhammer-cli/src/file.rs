use crate::cli::FileCommands;
use crate::mcp_integration::{response_formatting, CliToolContext};
use serde_json::json;
use std::io::{self, Read};

/// Handle file-related CLI commands
pub async fn handle_file_command(
    command: FileCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = CliToolContext::new().await?;

    match command {
        FileCommands::Read { path, offset, limit } => {
            read_file(&context, &path, offset, limit).await?;
        }
        FileCommands::Write { path, content } => {
            write_file(&context, &path, &content).await?;
        }
        FileCommands::Edit { path, old_string, new_string, replace_all } => {
            edit_file(&context, &path, &old_string, &new_string, replace_all).await?;
        }
        FileCommands::Glob { pattern, path, case_sensitive, no_git_ignore } => {
            glob_files(&context, &pattern, path.as_deref(), case_sensitive, !no_git_ignore).await?;
        }
        FileCommands::Grep { pattern, path, glob, file_type, case_insensitive, context_lines, output_mode } => {
            grep_files(&context, &pattern, path.as_deref(), glob.as_deref(), file_type.as_deref(), case_insensitive, context_lines, output_mode.as_deref()).await?;
        }
    }

    Ok(())
}

/// Read file contents with optional offset and limit
async fn read_file(
    context: &CliToolContext,
    path: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec![("absolute_path", json!(path))];
    
    if let Some(offset_val) = offset {
        args.push(("offset", json!(offset_val)));
    }
    if let Some(limit_val) = limit {
        args.push(("limit", json!(limit_val)));
    }

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("files_read", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Write content to a file (creates or overwrites)
async fn write_file(
    context: &CliToolContext,
    path: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let final_content = if content == "-" {
        // Read from stdin
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else {
        content.to_string()
    };

    let arguments = context.create_arguments(vec![
        ("file_path", json!(path)),
        ("content", json!(final_content)),
    ]);

    let result = context.execute_tool("files_write", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Edit file with precise string replacement
async fn edit_file(
    context: &CliToolContext,
    path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let arguments = context.create_arguments(vec![
        ("file_path", json!(path)),
        ("old_string", json!(old_string)),
        ("new_string", json!(new_string)),
        ("replace_all", json!(replace_all)),
    ]);

    let result = context.execute_tool("files_edit", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Find files using glob patterns
async fn glob_files(
    context: &CliToolContext,
    pattern: &str,
    path: Option<&str>,
    case_sensitive: bool,
    respect_git_ignore: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec![("pattern", json!(pattern))];
    
    if let Some(search_path) = path {
        args.push(("path", json!(search_path)));
    }
    if case_sensitive {
        args.push(("case_sensitive", json!(true)));
    }
    if !respect_git_ignore {
        args.push(("respect_git_ignore", json!(false)));
    }

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("files_glob", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Search file contents using ripgrep
async fn grep_files(
    context: &CliToolContext,
    pattern: &str,
    path: Option<&str>,
    glob: Option<&str>,
    file_type: Option<&str>,
    case_insensitive: bool,
    context_lines: Option<usize>,
    output_mode: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec![("pattern", json!(pattern))];
    
    if let Some(search_path) = path {
        args.push(("path", json!(search_path)));
    }
    if let Some(glob_pattern) = glob {
        args.push(("glob", json!(glob_pattern)));
    }
    if let Some(file_type_val) = file_type {
        args.push(("type", json!(file_type_val)));
    }
    if case_insensitive {
        args.push(("-i", json!(true)));
    }
    if let Some(context_val) = context_lines {
        args.push(("-C", json!(context_val)));
    }
    if let Some(mode) = output_mode {
        args.push(("output_mode", json!(mode)));
    }

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("files_grep", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(subcommand)]
        command: FileCommands,
    }

    #[test]
    fn test_file_read_command_basic() {
        let result = TestCli::try_parse_from(["test", "read", "/path/to/file.txt"]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Read { path, offset, limit } = cli.command {
            assert_eq!(path, "/path/to/file.txt");
            assert_eq!(offset, None);
            assert_eq!(limit, None);
        } else {
            panic!("Expected Read command");
        }
    }

    #[test]
    fn test_file_read_command_with_options() {
        let result = TestCli::try_parse_from([
            "test", "read", "/path/to/file.txt", "--offset", "10", "--limit", "50"
        ]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Read { path, offset, limit } = cli.command {
            assert_eq!(path, "/path/to/file.txt");
            assert_eq!(offset, Some(10));
            assert_eq!(limit, Some(50));
        } else {
            panic!("Expected Read command");
        }
    }

    #[test]
    fn test_file_write_command() {
        let result = TestCli::try_parse_from(["test", "write", "/path/to/file.txt", "Hello World"]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Write { path, content } = cli.command {
            assert_eq!(path, "/path/to/file.txt");
            assert_eq!(content, "Hello World");
        } else {
            panic!("Expected Write command");
        }
    }

    #[test]
    fn test_file_write_command_stdin() {
        let result = TestCli::try_parse_from(["test", "write", "/path/to/file.txt", "-"]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Write { path, content } = cli.command {
            assert_eq!(path, "/path/to/file.txt");
            assert_eq!(content, "-");
        } else {
            panic!("Expected Write command");
        }
    }

    #[test]
    fn test_file_edit_command_basic() {
        let result = TestCli::try_parse_from([
            "test", "edit", "/path/to/file.rs", "old_code", "new_code"
        ]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Edit { path, old_string, new_string, replace_all } = cli.command {
            assert_eq!(path, "/path/to/file.rs");
            assert_eq!(old_string, "old_code");
            assert_eq!(new_string, "new_code");
            assert!(!replace_all);
        } else {
            panic!("Expected Edit command");
        }
    }

    #[test]
    fn test_file_edit_command_replace_all() {
        let result = TestCli::try_parse_from([
            "test", "edit", "/path/to/file.rs", "old_code", "new_code", "--replace-all"
        ]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Edit { path, old_string, new_string, replace_all } = cli.command {
            assert_eq!(path, "/path/to/file.rs");
            assert_eq!(old_string, "old_code");
            assert_eq!(new_string, "new_code");
            assert!(replace_all);
        } else {
            panic!("Expected Edit command");
        }
    }

    #[test]
    fn test_file_glob_command_basic() {
        let result = TestCli::try_parse_from(["test", "glob", "**/*.rs"]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Glob { pattern, path, case_sensitive, no_git_ignore } = cli.command {
            assert_eq!(pattern, "**/*.rs");
            assert_eq!(path, None);
            assert!(!case_sensitive);
            assert!(!no_git_ignore);
        } else {
            panic!("Expected Glob command");
        }
    }

    #[test]
    fn test_file_glob_command_with_options() {
        let result = TestCli::try_parse_from([
            "test", "glob", "*.json", "--path", "/search/dir", "--case-sensitive", "--no-git-ignore"
        ]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Glob { pattern, path, case_sensitive, no_git_ignore } = cli.command {
            assert_eq!(pattern, "*.json");
            assert_eq!(path, Some("/search/dir".to_string()));
            assert!(case_sensitive);
            assert!(no_git_ignore);
        } else {
            panic!("Expected Glob command");
        }
    }

    #[test]
    fn test_file_grep_command_basic() {
        let result = TestCli::try_parse_from(["test", "grep", "search_pattern"]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Grep { pattern, path, glob, file_type, case_insensitive, context_lines, output_mode } = cli.command {
            assert_eq!(pattern, "search_pattern");
            assert_eq!(path, None);
            assert_eq!(glob, None);
            assert_eq!(file_type, None);
            assert!(!case_insensitive);
            assert_eq!(context_lines, None);
            assert_eq!(output_mode, None);
        } else {
            panic!("Expected Grep command");
        }
    }

    #[test]
    fn test_file_grep_command_with_options() {
        let result = TestCli::try_parse_from([
            "test", "grep", "error.*handling", 
            "--path", "/src", 
            "--glob", "*.rs", 
            "--type", "rust",
            "-i",
            "-C", "3",
            "--output-mode", "content"
        ]);
        assert!(result.is_ok());
        
        let cli = result.unwrap();
        if let FileCommands::Grep { pattern, path, glob, file_type, case_insensitive, context_lines, output_mode } = cli.command {
            assert_eq!(pattern, "error.*handling");
            assert_eq!(path, Some("/src".to_string()));
            assert_eq!(glob, Some("*.rs".to_string()));
            assert_eq!(file_type, Some("rust".to_string()));
            assert!(case_insensitive);
            assert_eq!(context_lines, Some(3));
            assert_eq!(output_mode, Some("content".to_string()));
        } else {
            panic!("Expected Grep command");
        }
    }

    #[test]
    fn test_file_command_help() {
        let result = TestCli::try_parse_from(["test", "read", "--help"]);
        assert!(result.is_err()); // Help exits with error but that's expected

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_file_command_missing_arguments() {
        let result = TestCli::try_parse_from(["test", "read"]);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_file_edit_missing_arguments() {
        let result = TestCli::try_parse_from(["test", "edit", "/path/to/file", "old"]);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }
}