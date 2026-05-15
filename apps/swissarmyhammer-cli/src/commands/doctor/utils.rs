//! Utility functions for the doctor module

/// Get the Claude add command
pub fn get_claude_add_command() -> String {
    r#"Initialize SwissArmyHammer in your project:

sah init

Or add manually to Claude Code:
claude mcp add --scope user sah sah serve
"#
    .to_string()
}
