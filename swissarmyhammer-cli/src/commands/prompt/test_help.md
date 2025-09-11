Test prompts interactively to see how they render with different arguments.
Helps debug template errors and refine prompt content.

Usage modes:
  sah prompt test prompt-name                    # Interactive test
  sah prompt test -f path/to/prompt.md          # Test from file  
  sah prompt test prompt-name --var key=value   # Non-interactive

Examples:
  sah prompt test code-review
  sah prompt test help --var topic=git
  sah --verbose prompt test plan