# PLAN_000003: Prompt System Updates

**Refer to ./specification/plan.md**

## Goal

Update the plan prompt in `builtin/prompts/plan.md` to accept a `plan_filename` parameter instead of hardcoded directory scanning, focusing planning on a specific file.

## Background

The current plan prompt is hardcoded to work with the `./specification` directory and scans all files within it. We need to modify it to work with a specific file passed as a parameter, making it more flexible and focused.

## Requirements

1. Add `plan_filename` argument to prompt YAML front matter
2. Replace hardcoded directory references with parameter usage
3. Remove directory scanning logic
4. Update process instructions to work with specific file
5. Maintain all existing planning logic and quality
6. Update reference instructions for issue creation

## Implementation Details

### Current Prompt Front Matter

```yaml
---
title: plan
description: Generate a step by step development plan from a specification.
---
```

### Updated Prompt Front Matter

```yaml
---
title: plan
description: Generate a step by step development plan from a specific specification file.
arguments:
  - name: plan_filename
    description: Path to the specific plan markdown file to process
    required: true
---
```

### Process Section Updates

Replace the current Process section with:

```markdown
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

### Reference Instruction Updates

Update the guideline about issue references:

```markdown
- Each issue you create that is a step in the plan should include the phrase "Refer to {{ plan_filename }}"
```

## Implementation Steps

1. Open `builtin/prompts/plan.md`
2. Update YAML front matter to include arguments section
3. Add liquid template parameter usage in Goal section
4. Update Process section to use parameter instead of directory scanning
5. Remove any hardcoded `./specification` references
6. Update reference instruction to use parameter
7. Ensure all liquid template syntax is correct

## Acceptance Criteria

- [ ] Prompt accepts `plan_filename` argument
- [ ] Argument is properly documented in YAML front matter
- [ ] All hardcoded directory references removed
- [ ] Process focuses on specific file instead of directory scanning
- [ ] Reference instructions updated to use parameter
- [ ] Liquid template syntax is correct throughout
- [ ] Maintains all existing planning quality and structure

## Testing

- Verify prompt can be called with plan_filename parameter
- Confirm file parameter is properly used in processing
- Check that liquid template rendering works correctly
- Ensure no hardcoded paths remain

## Dependencies

- Requires workflow updates from PLAN_000002
- Must integrate with existing prompt execution system

## Notes

- Use liquid template syntax: `{{ plan_filename }}`
- Remove all references to `./specification` directory
- Focus on single file processing instead of directory scanning
- Maintain all existing planning guidelines and quality standards
- The argument will be passed from the workflow execution

## Proposed Solution

After analyzing the current `builtin/prompts/plan.md` file, I will implement the following changes:

### 1. YAML Front Matter Updates
- Add `arguments` section with `plan_filename` parameter
- Update description to reflect single file processing instead of directory scanning

### 2. Process Section Overhaul
- Remove hardcoded `./specification` directory references
- Replace directory scanning logic with single file parameter usage  
- Update draft plan file creation to use parameter-based naming
- Modify git workflow to focus on single file changes rather than directory comparisons

### 3. Liquid Template Implementation
- Use `{{ plan_filename }}` throughout the prompt
- Replace line 28's hardcoded reference with parameter usage
- Ensure proper liquid syntax for file parameter

### 4. Specific Changes Required

**Lines to modify:**
- Line 3: Update description  
- Line 28: Replace hardcoded path with `{{ plan_filename }}`
- Line 33: Remove directory scanning, focus on single file
- Line 36: Remove git directory comparison
- Lines 44-45: Update example to use parameter

**Key implementation details:**
- Use liquid template syntax: `{{ plan_filename }}`
- Maintain all existing planning quality and structure
- Remove all references to directory scanning
- Focus processing on the specific file parameter
- Update issue creation to use file-based prefixes

This approach transforms the prompt from a directory-based scanner to a focused single-file processor while maintaining all existing planning capabilities and quality standards.

## Implementation Notes

Successfully implemented all required changes to `builtin/prompts/plan.md`:

### ✅ Changes Completed

1. **YAML Front Matter Updated**: 
   - Added `arguments` section with `plan_filename` parameter
   - Updated description to reflect single file processing
   - Parameter marked as required with proper description

2. **Liquid Template Parameter Usage**:
   - Line 32: Updated reference instruction to use `{{ plan_filename }}`
   - Line 37: Updated Process section to use `{{ plan_filename }}`
   - Line 47: Updated example to reference `FOO.md` instead of just `FOO`

3. **Process Section Overhaul**:
   - Removed hardcoded `./specification` directory references
   - Removed git directory comparison logic (line 36 in original)
   - Focused on single file analysis instead of directory scanning
   - Maintained all existing planning quality guidelines

4. **File Naming Convention Updates**:
   - Updated example in line 47 to use `.md` extension for clarity
   - Maintained numbered prefix pattern for issue ordering

### ✅ Acceptance Criteria Met

- [x] Prompt accepts `plan_filename` argument
- [x] Argument is properly documented in YAML front matter  
- [x] All hardcoded directory references removed
- [x] Process focuses on specific file instead of directory scanning
- [x] Reference instructions updated to use parameter
- [x] Liquid template syntax is correct throughout
- [x] Maintains all existing planning quality and structure

### Implementation Quality

- Used proper liquid template syntax: `{{ plan_filename }}`
- Removed all references to `./specification` directory
- Maintained existing planning guidelines and quality standards
- Preserved workflow integration patterns
- Updated examples to be more specific and clear

The prompt is now ready for integration with workflow parameter support from PLAN_000002.