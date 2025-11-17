# Rule Violation: code-quality/code-duplication

**File**: swissarmyhammer-cli/src/dynamic_cli.rs
**Severity**: ERROR

## Violation Message

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/dynamic_cli.rs
Line: 826-895
Severity: warning
Message: Near-identical command building patterns repeated for `build_prompt_command()` and `build_flow_command()`. Both methods follow the same pattern of creating a Command, setting about/long_about text, and adding subcommands with similar structures.
Suggestion: Extract a common command builder helper that accepts configuration data (name, about, long_about, subcommands) to reduce duplication. Consider creating a struct like `CommandConfig` with builder methods.

---

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/dynamic_cli.rs
Line: 471-541
Severity: warning
Message: The `build_cli()` method contains repeated patterns for adding arguments. The verbose, debug, quiet, and format argument definitions follow identical patterns with only the names/descriptions differing.
Suggestion: Create a helper method `add_standard_flags(cmd: Command) -> Command` that adds these common flags, reducing repetition and ensuring consistency across commands.

---

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/dynamic_cli.rs
Line: 826-1100
Severity: info
Message: The `build_prompt_command()`, `build_flow_command()`, `build_rule_command()`, and `build_agent_command()` methods all follow nearly identical structure: create Command, set about/long_about, add subcommands with detailed help text. The pattern repeats 4 times with only the content differing.
Suggestion: Consider a data-driven approach using a `CommandDefinition` struct that captures the common structure (name, about, long_about, subcommands). Then create a single `build_command_from_definition()` method that processes these definitions, eliminating the structural duplication.

---

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/dynamic_cli.rs
Line: 1174-1210
Severity: info
Message: The workflow shortcut building logic in `build_workflow_shortcuts()` has similar command construction patterns to other command builders, particularly the use of `intern_string()`, `about()`, and argument addition patterns that appear throughout the file.
Suggestion: Consolidate command building logic into shared helper methods that handle common patterns like setting about text, adding standard arguments, and interning strings.

---
*This issue was automatically created by `sah rule check --create-todos`*
