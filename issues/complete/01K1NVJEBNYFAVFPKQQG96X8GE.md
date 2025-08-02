
When I run 

```
 cargo run -- prompt test say-hello
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.18s
     Running `target/debug/swissarmyhammer prompt test say-hello`
2025-08-02T16:58:56.015939Z  INFO swissarmyhammer: Running prompt command
📝 Please provide values for the following arguments:

✔ name (optional): The name of the person to greet · Friend
✔ language (optional): The language to greet in · English

✨ Rendered Output:
──────────────────────────────────────────────────
DO NOT run any tools to perform this task:


Please respond with: "Hello, Friend! Greetings from Swiss Army Hammer! The workflow system is working correctly."

──────────────────────────────────────────────────
```

I actually expect to see SwissArmyHammer as is configured in `sah.toml`.

When I run

```
 cargo run validate
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.19s
     Running `target/debug/swissarmyhammer validate`
2025-08-02T16:59:59.729123Z  INFO swissarmyhammer: Running validate command

say-hello
  prompt:say-hello
  ERROR [-] Undefined template variable: 'project_name'
    💡 Add 'project_name' to the arguments list or remove the template variable

sah.toml
  sah.toml
  INFO [-] Configuration file validation passed

Summary:
  Files checked: 81
  Errors: 1

✗ Validation failed with errors.
```

I do not expect a validation failure, as the variable `project_name` *is* defined -- in `sah.toml`

Fix it.
## Proposed Solution

After analyzing the codebase, I found the root cause of the issue:

1. **Problem**: The template validation logic in `swissarmyhammer/src/prompts.rs` only checks if template variables are defined in the prompt's arguments, but it doesn't consider variables that might be defined in `sah.toml`.

2. **Current behavior**: When validating the `say-hello.md` prompt that uses `{{ project_name | default: "Swiss Army Hammer" }}`, the validator looks for `project_name` in the prompt's arguments list, but it's not there - it's defined in `sah.toml`.

3. **Solution**: Modify the template validation logic to also consider variables defined in `sah.toml` when checking for undefined template variables.

## Implementation Steps

1. Update the `Prompt::validate()` method to load and consider `sah.toml` variables
2. Modify the validation logic to check both prompt arguments AND sah.toml variables
3. Ensure the fix works for both validation and actual template rendering
4. Test the changes to ensure they work correctly

The key file to modify is `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/prompts.rs` around line 400-450 where the "Undefined template variable" error is generated.