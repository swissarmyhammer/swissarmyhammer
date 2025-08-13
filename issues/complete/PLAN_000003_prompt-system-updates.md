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

## Proposed Solution

I will implement this issue by systematically updating the `builtin/prompts/plan.md` file to:

1. **Update YAML Front Matter**: Add arguments section with plan_filename parameter
2. **Modify Goal Section**: Update description to reference specific file processing
3. **Refactor Process Section**: Replace directory scanning logic with parameter-based file processing
4. **Update Reference Instructions**: Change guideline to use liquid template parameter
5. **Remove Hardcoded Paths**: Eliminate all `./specification` directory references
6. **Validate Template Syntax**: Ensure all liquid template syntax is correct

The key changes will be:
- Add `arguments` section to YAML front matter defining plan_filename parameter
- Use `{{ plan_filename }}` liquid template syntax throughout
- Replace directory scanning steps with specific file analysis
- Update issue creation instructions to reference the parameter
- Maintain all existing planning logic and quality standards

## Notes

- Use liquid template syntax: `{{ plan_filename }}`
- Remove all references to `./specification` directory
- Focus on single file processing instead of directory scanning
- Maintain all existing planning guidelines and quality standards
- The argument will be passed from the workflow execution