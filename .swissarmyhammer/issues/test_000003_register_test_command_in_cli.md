Refer to ideas/test.md

Register the new test command in the main CLI application.

This is the third step in implementing the `sah test` command. We need to register the new command so it's available in the CLI.

Tasks:
- Update `swissarmyhammer-cli/src/commands/mod.rs` to include the test module
- Update `swissarmyhammer-cli/src/main.rs` to handle the new command
- Add the necessary command routing logic
- Ensure the command follows the same patterns as other commands

This step connects the new test command module to the main CLI application, making it accessible to users.