in the cli, validate.rs has validation logic in the cli. This is misplaced.

Workflows and Prompts need to be self validating with a Validatable trait that returns Vec<ValidationIssue
>

validate.rs in the cli just needs ot delegate to the trait and format results

there is a lot of dead code in validate.rs marked, remove it all.

There is goofy -- incorrect -- validation logic in validate.rs about alphanumeric characters. The parsers in swissarmyhammer should be the only opinion on parse and character validity.

## Proposed Solution

After examining the code structure, I will:

1. **Design Validatable trait** in the existing validation module
   - Add a `Validatable` trait that returns `Vec<ValidationIssue>`
   - This trait will be the standard interface for self-validating types

2. **Implement Validatable trait for core types**
   - Implement the trait for `Workflow` type in `swissarmyhammer/src/workflow/definition.rs`
   - Implement the trait for `Prompt` type in `swissarmyhammer/src/prompts.rs`
   - Move the validation logic from CLI into these implementations

3. **Refactor CLI validate.rs**
   - Remove workflow-specific validation logic (lines 342-556) - delegate to `Workflow::validate`
   - Remove prompt field validation logic (lines 231-289) - delegate to `Prompt::validate`
   - Remove incorrect alphanumeric validation logic (lines 372-386)
   - Remove all dead code marked with `#[allow(dead_code)]`
   - Keep only formatting and result aggregation logic

4. **Clean up and test**
   - Remove dead code
   - Ensure all existing tests still pass
   - Add tests for the new trait implementations

This approach follows the principle of making types self-validating while keeping the CLI as a thin layer that delegates to the core library types.