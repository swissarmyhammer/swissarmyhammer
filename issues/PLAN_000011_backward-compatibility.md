# PLAN_000011: Backward Compatibility

**Refer to ./specification/plan.md**

## Goal

Ensure that the new plan command implementation maintains full backward compatibility with existing workflows, commands, and usage patterns in swissarmyhammer, while not breaking any existing functionality.

## Background

The plan command implementation introduces changes to the plan workflow and prompt system. We need to ensure that any existing usage of these components continues to work exactly as before, while adding the new parameterized functionality.

## Requirements

1. Existing plan workflow continues to work without parameters
2. Plan prompt maintains compatibility with existing callers
3. No breaking changes to public APIs
4. Existing issue numbering and creation patterns preserved
5. File system interactions remain consistent
6. Error handling doesn't change existing behavior
7. Workflow execution system remains compatible
8. Command line interface maintains existing functionality

## Compatibility Areas

### 1. Workflow Compatibility

The plan workflow must work both ways:

```yaml
# New parameterized usage (from CLI)
execute prompt "plan" with plan_filename="{{ plan_filename }}"

# Legacy usage (if called without parameter)
execute prompt "plan"
```

Implementation approach:
- Make plan_filename parameter optional with default behavior
- Maintain existing hardcoded directory scanning as fallback
- Ensure workflow metadata remains compatible

### 2. Prompt Compatibility

The plan prompt must handle both scenarios:

```yaml
# Updated front matter
arguments:
  - name: plan_filename
    description: Path to the specific plan markdown file to process
    required: false  # Made optional for compatibility
    default: "./specification"  # Fallback to existing behavior
```

### 3. CLI Compatibility

Ensure no existing commands are affected:

```bash
# All existing commands continue to work
swissarmyhammer serve
swissarmyhammer doctor
swissarmyhammer prompt test plan
swissarmyhammer flow run plan  # This should still work!

# New command added without conflicts
swissarmyhammer plan ./specification/feature.md
```

### 4. Issue Creation Compatibility

Maintain existing issue numbering and file naming:
- Existing numbering sequences preserved
- File naming patterns remain consistent
- No conflicts with existing issue management

## Implementation Details

### Workflow Backward Compatibility

```yaml
---
title: Plan
description: Create a plan from a specification
tags:
  - auto
---

## Parameters

- plan_filename: The path to the specific plan file to process (optional, defaults to scanning ./specification directory)

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> plan
    plan --> done
    done --> [*]
```

## Actions

- start: log "Making the plan{% if plan_filename %} for {{ plan_filename }}{% endif %}"
- plan: execute prompt "plan"{% if plan_filename %} with plan_filename="{{ plan_filename }}"{% endif %}
- done: log "Plan ready, look in ./issues"

## Description

This workflow creates a step-by-step plan from specification files.
When plan_filename is provided, plans the specific file.
When no parameter is given, scans the ./specification directory (legacy behavior).
```

### Prompt Backward Compatibility

```yaml
---
title: plan
description: Generate a step by step development plan from specification(s).
arguments:
  - name: plan_filename
    description: Path to the specific plan markdown file to process (optional)
    required: false
---

## Goal

Turn specification(s) into a multiple step plan.

{% if plan_filename %}
Process the specific plan file: {{ plan_filename }}
{% else %}
Process all specifications in the ./specification directory.
{% endif %}

## Process

{% if plan_filename %}
- Read and analyze the specified plan file: {{ plan_filename }}
{% else %}
- Review the existing `./specification` directory and determine what is to be planned.
{% endif %}
- Use git to determine what has changed in the specification compared to what has already been planned.
- Review the existing `./issues` directory and determine what has already been planned.
- Review the existing memos and think deeply about how they apply to the plan.
- Review the existing code to determine what parts of the specification might already be implemented.
- [Rest of process remains the same...]
```

### Flow System Compatibility

Ensure existing flow commands continue to work:

```rust
// In command handler, ensure flow run still works
Commands::Flow { subcommand } => {
    match subcommand {
        FlowSubcommand::Run { workflow, vars, .. } => {
            if workflow == "plan" {
                // Legacy flow run plan should work without parameters
                // This maintains existing behavior
                execute_workflow("plan", vars, /* ... */).await?;
            }
            // ... handle other workflows
        }
        // ... other flow subcommands
    }
}

// New plan command is separate and doesn't interfere
Commands::Plan { plan_filename } => {
    let vars = vec![
        ("plan_filename".to_string(), plan_filename.clone())
    ];
    execute_workflow("plan", vars, Vec::new(), false, false, false, None, false).await?;
}
```

## Testing Strategy

### 1. Regression Testing

Test all existing usage patterns:

```bash
# Existing workflow execution should work
swissarmyhammer flow run plan

# Existing prompt testing should work
swissarmyhammer prompt test plan

# All other commands unaffected
swissarmyhammer serve
swissarmyhammer doctor
swissarmyhammer issue list
```

### 2. Compatibility Testing

Create tests that verify both old and new behavior:

```rust
#[tokio::test]
async fn test_plan_workflow_legacy_compatibility() {
    // Test that plan workflow works without parameters (legacy mode)
    let result = execute_workflow("plan", Vec::new(), Vec::new(), false, false, false, None, false).await;
    assert!(result.is_ok(), "Legacy plan workflow should still work");
}

#[tokio::test]
async fn test_plan_workflow_with_parameters() {
    // Test new parameterized functionality
    let vars = vec![("plan_filename".to_string(), "test.md".to_string())];
    let result = execute_workflow("plan", vars, Vec::new(), false, false, false, None, false).await;
    assert!(result.is_ok(), "New parameterized workflow should work");
}

#[tokio::test]
async fn test_flow_run_plan_still_works() {
    // Test that 'flow run plan' command still works
    // This tests CLI backward compatibility
}
```

### 3. Integration Testing

Verify no breaking changes in complete workflows:

```rust
#[tokio::test]
async fn test_existing_user_workflows() {
    // Test typical user workflows that might use plan
    // Ensure they continue working exactly as before
}
```

## Migration Strategy

### For Existing Users

1. **No Action Required**: Existing workflows continue to work
2. **Optional Migration**: Users can gradually adopt the new command
3. **Clear Documentation**: Document both old and new approaches
4. **Deprecation Timeline**: No deprecation - both approaches remain valid

### Communication

- Document new functionality without implying old approach is deprecated
- Provide examples showing both approaches are valid
- Explain when to use each approach

## Implementation Steps

1. Implement parameter as optional in workflow and prompt
2. Add conditional logic to handle both parameterized and non-parameterized calls
3. Ensure default behavior matches exactly the current behavior
4. Test all existing usage patterns thoroughly
5. Add regression tests for backward compatibility
6. Verify no performance impact on existing functionality
7. Test integration with all other commands
8. Document compatibility guarantees

## Acceptance Criteria

- [ ] Existing `swissarmyhammer flow run plan` continues to work
- [ ] Plan prompt works both with and without parameters
- [ ] Plan workflow maintains existing behavior when called without parameters
- [ ] No existing commands are affected by the changes
- [ ] All existing tests continue to pass
- [ ] Issue creation patterns remain consistent
- [ ] Performance of existing functionality is not degraded
- [ ] Comprehensive regression test coverage added

## Validation Commands

```bash
# Test all existing functionality
swissarmyhammer flow run plan
swissarmyhammer prompt test plan
swissarmyhammer flow list | grep plan

# Test new functionality
swissarmyhammer plan ./specification/test.md

# Verify help system
swissarmyhammer --help | grep plan
swissarmyhammer plan --help
```

## Dependencies

- Must be implemented after all core functionality (PLAN_000001-000010)
- Requires thorough testing of existing system behavior
- Should be final step before release

## Notes

- Backward compatibility is critical for user trust
- No existing functionality should be deprecated or removed
- Both approaches should be documented as equally valid
- Consider this a feature addition, not a replacement
- Test thoroughly with real-world usage scenarios
- Monitor for any performance regressions