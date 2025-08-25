Execute planning workflow for a specific specification file.
Takes a path to a markdown specification file and generates step-by-step implementation issues.

USAGE:
  swissarmyhammer plan <PLAN_FILENAME>

The planning workflow will:
• Read and analyze the specified plan file
• Review existing issues to avoid conflicts
• Generate numbered issue files in the ./issues directory  
• Create incremental, focused implementation steps
• Use existing memos and codebase context for better planning

FILE REQUIREMENTS:
The plan file should be:
• A valid markdown file (.md extension recommended)
• Readable and contain meaningful content
• Focused on a specific feature or component
• Well-structured with clear goals and requirements

OUTPUT:
Creates numbered issue files in ./issues/ directory with format:
• PLANNAME_000001_step-description.md
• PLANNAME_000002_step-description.md
• etc.

EXAMPLES:
  # Plan a new feature from specification directory
  swissarmyhammer plan ./specification/user-authentication.md
  
  # Plan using absolute path
  swissarmyhammer plan /home/user/projects/plans/feature-development.md
  
  # Plan a quick enhancement
  swissarmyhammer plan ./docs/bug-fixes.md
  
  # Plan with verbose output for debugging
  swissarmyhammer --verbose plan ./specification/api-redesign.md

TIPS:
• Keep plan files focused - break large features into multiple plans
• Review generated issues before implementation
• Use descriptive filenames that reflect the planned work
• Check existing issues directory to understand numbering
• Plan files work best when they include clear goals and acceptance criteria

TROUBLESHOOTING:
If planning fails:
• Verify file exists and is readable: ls -la <plan_file>
• Check issues directory permissions: ls -ld ./issues
• Ensure adequate disk space for issue file creation
• Try with --debug flag for detailed execution information
• Review file content for proper markdown formatting

For more information, see: swissarmyhammer --help