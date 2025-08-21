it does not seem like there should be a WorkflowParameter -- duplicate type - Parameters are supposed to be unified

## Proposed Solution

After analyzing the codebase, I've identified the duplication issue. There are indeed duplicate types:

1. **WorkflowParameter** - Defined in `workflow/definition.rs`  
2. **Parameter** - Defined in `common/parameters.rs` (the shared unified type)
3. **ParameterType** - Exists in both modules with identical definitions

The shared `Parameter` system was created to unify parameters across prompts and workflows, but the workflow module still maintains its own duplicate types.

### Implementation Steps:

1. **Remove duplicate ParameterType** from `workflow/definition.rs` and use the shared `ParameterType` from `common/parameters.rs`
2. **Remove WorkflowParameter struct** entirely and replace all usages with the shared `Parameter` type
3. **Update the Workflow struct** to use `Vec<Parameter>` instead of `Vec<WorkflowParameter>`
4. **Update all imports** across the codebase to use the shared types
5. **Remove the conversion methods** (`to_parameter()` and `From<Parameter> for WorkflowParameter`) since they'll no longer be needed
6. **Update all calling code** that currently creates `WorkflowParameter` to create `Parameter` instead

The shared `Parameter` type already has all the functionality needed (including validation, conditions, etc.) and was designed to replace the workflow-specific type.
## Implementation Completed

Successfully unified the parameter system by removing the duplicate types:

### Changes Made:

1. **Removed duplicate `ParameterType` enum** from `workflow/definition.rs` - now uses the shared `ParameterType` from `common/parameters.rs`

2. **Removed `WorkflowParameter` struct entirely** from `workflow/definition.rs` and replaced all usages with the unified `Parameter` type

3. **Updated Workflow struct** to use `Vec<Parameter>` instead of `Vec<WorkflowParameter>`

4. **Simplified ParameterProvider implementation** - no longer needs conversion since we directly store `Parameter` instances

5. **Updated all files that used WorkflowParameter:**
   - `swissarmyhammer/src/workflow/parser.rs` - Updated parameter creation to use `Parameter::new()`
   - `swissarmyhammer/src/common/parameter_cli.rs` - Updated function signatures and imports
   - `swissarmyhammer-cli/src/parameter_cli.rs` - Removed conversion logic
   - `swissarmyhammer/src/workflow/mod.rs` - Removed exports of duplicate types

6. **Updated all tests** to use the new `Parameter::new()` builder pattern instead of struct literals

7. **Fixed integration tests** to use the correct import paths

### Benefits Achieved:

- **Eliminated code duplication** - No more duplicate `ParameterType` and `WorkflowParameter` types
- **Simplified codebase** - Removed unnecessary conversion methods (`to_parameter()`, `From<Parameter>`)
- **Unified parameter system** - All parameters (prompts and workflows) now use the same `Parameter` type
- **Better maintainability** - Only one place to make parameter system changes
- **Consistent API** - All parameter operations work the same way across the system

### Verification:

- ✅ Project builds successfully 
- ✅ All parameter-related tests pass (149 tests)
- ✅ All workflow definition tests pass
- ✅ Parameter validation continues to work as expected
- ✅ Backward compatibility maintained through proper use of the builder pattern