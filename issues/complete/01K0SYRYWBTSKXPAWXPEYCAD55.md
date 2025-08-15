eliminate the ./builtin/prompts/workflows and any references to them including in documentation. use actual workflows in ./builtin/workflows for documentation samples

## Proposed Solution

Based on my analysis of the codebase, I have identified the following items that need to be addressed:

### Current State
- `./builtin/prompts/workflows/` contains 5 example workflow files that are **prompt templates**, not actual workflows
- `./builtin/workflows/` contains 3 actual workflow definitions with proper YAML frontmatter and state machines
- `doc/src/workflow-examples.md` references both directories and uses examples from the prompts/workflows directory

### Files to be affected
1. **Remove directory**: `./builtin/prompts/workflows/` and all its contents:
   - code-review.md
   - data-processing-pipeline.md
   - database-migration.md
   - deployment-pipeline.md
   - multi-step-refactoring.md

2. **Update documentation**: `doc/src/workflow-examples.md`
   - Remove references to `builtin/prompts/workflows/`
   - Update all workflow examples to use actual workflows from `./builtin/workflows/`
   - Ensure documentation examples are based on real, executable workflows

3. **Check code references**: `swissarmyhammer/src/file_loader.rs` has comments mentioning prompts/workflows - will verify if updates are needed

### Implementation Steps
1. Remove the `./builtin/prompts/workflows/` directory entirely
2. Update `doc/src/workflow-examples.md` to only reference `./builtin/workflows/`
3. Update all example references in documentation to use actual workflows
4. Check and update any code references if needed
5. Run tests to ensure nothing breaks

This will clean up the confusion between prompt templates and actual workflows, making the documentation consistent with the actual workflow system.