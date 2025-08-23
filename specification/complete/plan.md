# Plan Subcommand Specification

## Overview

Add a new `plan` subcommand to the swissarmyhammer CLI that accepts a single plan file path parameter and runs the plan workflow with that specific file, removing the hardcoded dependency on the specification directory.

## Current State Analysis

### Existing Plan Workflow
- **Workflow**: `builtin/workflows/plan.md` - Simple workflow that executes the plan prompt
- **Prompt**: `builtin/prompts/plan.md` - Hardcoded to search `./specification` directory
- **CLI**: No dedicated plan subcommand exists currently

### Current Limitations
1. Plan prompt is hardcoded to work with `./specification` directory
2. No way to plan a specific file without running against all specs
3. Workflow doesn't accept parameters for targeting specific files

## Specification

### CLI Interface

Add a new top-level subcommand `plan` to the CLI:

```bash
swissarmyhammer plan <plan_filename>
```

#### Parameters
- `plan_filename` (required): Path to a specific markdown plan file to process

#### Examples
```bash
swissarmyhammer plan ./specification/new-feature.md
swissarmyhammer plan /path/to/custom-plan.md
swissarmyhammer plan plans/refactor.md
```

### Required Changes

#### 1. CLI Structure Updates

Add to `Commands` enum in `swissarmyhammer-cli/src/cli.rs`:

```rust
/// Plan a specific specification file
#[command(long_about = "
Execute planning workflow for a specific specification file.
Takes a path to a markdown specification file and generates implementation steps.

Basic usage:
  swissarmyhammer plan <plan_filename>    # Plan specific file

The planning workflow will:
- Read the specified plan file
- Generate step-by-step implementation issues
- Create numbered issue files in ./issues directory

Examples:
  swissarmyhammer plan ./specification/new-feature.md
  swissarmyhammer plan /path/to/custom-plan.md
  swissarmyhammer plan plans/database-migration.md
")]
Plan {
    /// Path to the plan file to process
    plan_filename: String,
},
```

#### 2. Workflow Parameter Support

Update `builtin/workflows/plan.md` to accept a `plan_filename` parameter:

```yaml
---
title: Plan
description: Create a plan from a specification
tags:
  - auto
---

## Parameters

- plan_filename: The path to the specific plan file to process

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> plan
    plan --> done
    done --> [*]
```

## Actions

- start: log "Making the plan for {{ plan_filename }}"
- plan: execute prompt "plan" with plan_filename="{{ plan_filename }}"
- done: log "Plan ready, look in ./issues"

## Description

This workflow creates a step-by-step plan from a specific specification file.
```

#### 3. Prompt System Updates

Update `builtin/prompts/plan.md` to:
1. Accept `plan_filename` parameter instead of hardcoded directory
2. Focus on the specific file rather than scanning all specifications

Key changes:
- Replace hardcoded `./specification` directory references with `{{ plan_filename }}` parameter
- Remove directory scanning logic
- Focus planning on the single specified file
- Update context instructions to work with a specific file path

```markdown
---
title: plan
description: Generate a step by step development plan from a specific specification file.
arguments:
  - name: plan_filename
    description: Path to the specific plan markdown file to process
    required: true
---

## Goal

Turn a specific specification file into a multiple step plan.

Generate a multiple step plan in the `./issues` folder of multiple `<nnnnnn>_step.md` markdown step files, one for each step in order.

**Note**: System prompt with coding standards and principals is now automatically injected via Claude Code integration.

## Guidelines

- DO Follow the Coding Standards
- DO NOT code at this step, we are just creating the plan
- DO make sure each step file is a single focused task
- DO create many, small step files. Ideally each step should result in less than 500 lines of code changed
- Any time you create a step file, it should use the next number larger than all other issues
- DO Use markdown
- DO Use Mermaid to diagram and make the step clearer
- DO provide context in the issues that will help when it is time to code
- Each step must be incremental progress, ensuring no big jumps in complexity at any stage
- DO make sure that each step builds on the previous prompts, and ends with wiring things together
- DO NOT leave hanging or orphaned code that isn't integrated into a previous step
- Each issue you create that is a step in the plan should include the phrase "Refer to {{ plan_filename }}"
- Iterate until you feel that the steps are right sized for this project.

## Process

- Read and analyze the specified plan file: {{ plan_filename }}
- Review the existing `./issues` directory and determine what has already been planned.
- Review the existing memos and think deeply about how they apply to the plan.
- Review the existing code to determine what parts of the specification might already be implemented.
- Draft a detailed, step-by-step plan to meet the specification, write this out to a temp file `.swissarmyhammer/tmp/DRAFT_PLAN.md`, refer to this draft plan to refresh your memory.
- Then, once you have a draft plan, break it down into small, iterative chunks that build on each other incrementally.
- Look at these chunks and then go another round to break it into small steps.
- From here you should have the foundation to provide an in order series of issue files that describes the work to do at each step
- Review the results and make sure that the steps are small enough to be implemented safely, but big enough to move the project forward
- When creating issue steps for the plan, make sure to prefix and number them padded with 0's so they run in order
  - Example, assuming your spec file is called `FOO.md`, make issue files called `FOO_<nnnnnn>_name.md`, so that your plan steps are in order
  - Use the issue_create tool, specifying the name, again making sure they are named so that they run in order
```

#### 4. CLI Command Handler

Add handler logic in `swissarmyhammer-cli/src/flow.rs` or appropriate module:

```rust
Commands::Plan { plan_filename } => {
    let vars = vec![
        ("plan_filename".to_string(), plan_filename.clone())
    ];
    
    // Execute plan workflow with the filename parameter
    execute_workflow("plan", vars, Vec::new(), false, false, false, None, false).await?;
}
```

### Implementation Steps

1. **CLI Structure**: Add `Plan` command to CLI enum and parsing
2. **Workflow Updates**: Modify plan workflow to accept and use plan_filename parameter
3. **Prompt Updates**: Update plan prompt to work with specific file instead of directory
4. **Command Handler**: Implement CLI command handler that passes filename to workflow
5. **Testing**: Test with various plan file paths and scenarios
6. **Documentation**: Update CLI help text and examples

### Benefits

1. **Flexibility**: Plan any markdown file, not just those in `./specification`
2. **Targeted Planning**: Focus on specific features without processing all specs
3. **No Special Directories**: Remove hardcoded dependency on specification directory
4. **Better Workflow**: More granular control over what gets planned
5. **Backward Compatibility**: Existing workflows continue to work

### Migration Notes

- The specification directory will no longer be "special" to swissarmyhammer
- Users can organize their plan files however they prefer
- Existing plan workflows will need to be updated to use the new parameter system
- The prompt system becomes more flexible and reusable