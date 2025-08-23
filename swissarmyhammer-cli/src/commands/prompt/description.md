Manage prompts with support for listing, validating, testing, and searching.
Prompts are markdown files with YAML front matter that define reusable templates.

Basic usage:
  swissarmyhammer prompt list                    # List all prompts
  swissarmyhammer prompt validate                # Validate prompt files
  swissarmyhammer prompt test <name>             # Test a prompt
  swissarmyhammer prompt search <query>          # Search prompts

Examples:
  swissarmyhammer prompt list --source builtin
  swissarmyhammer prompt validate --quiet
  swissarmyhammer prompt test code-review --var file=main.rs
  swissarmyhammer prompt search "python code"