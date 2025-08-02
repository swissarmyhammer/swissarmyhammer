
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