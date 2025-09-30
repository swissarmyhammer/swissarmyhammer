---
title: plan
description: Generate a step by step development plan from specification(s).
parameters:
  - name: plan_filename
    description: Path to the specific plan markdown file to process (optional)
    required: true
---

## Goal

Turn specification(s) into a multiple step plan.

Process the specific plan file: {{ plan_filename }}

Generate a multiple step plan with multiple issues folder of multiple `<nnnnnn>_step.md` markdown step files, one for each step in order. Use the issue_create tool to create an issue for each step.


## Guidelines

- DO NOT code at this step, we are just creating the plan
- DO make sure to review the existing codebase and architecture before creating the implementation plan
- DO make sure each step file is a single focused task
- DO create many, small, incremental step files. Ideally each step should result in less than 250 lines of code changed
- Any time you create a step file, it should use the next number larger than all other issues
- DO Use markdown in the step files
- DO Use Mermaid to diagram and make the step clearer
- DO provide context in the step files that will help when it is time to code
- DO make sure that each step builds on the previous prompts
- DO NOT leave hanging or orphaned code that isn't integrated into a previous step
- DO NOT plan security features unless specifically asked by the user
- DO NOT plan performance features unless specifically asked by the user
- DO NOT plan backward compatibility features unless specifically asked by the user
- Each issue you create that is a step in the plan should include the phrase "Refer to {{ plan_filename }}"
- Iterate until you feel that the steps are right sized for this project.

## Process

- Read and analyze the specified plan file: {{ plan_filename }}
- Review the existing memos and think deeply about how they apply to the plan.
- Review the existing code to determine what parts of the specification might already be implemented.  Unless explicitly instructed otherwise in the specification, do not add new systems/services when existing patterns and systems can be extended to achieve the goals.
- Draft a detailed, step-by-step plan to meet the specification, write this out to a temp file `.swissarmyhammer/tmp/DRAFT_PLAN.md`, refer to this draft plan to refresh your memory.
- Then, once you have a draft plan, break it down into small, iterative chunks that build on each other incrementally.
- Review the results and make sure that the steps are small enough to be implemented safely, but big enough to move the project forward
- When creating issue steps for the plan, make sure to prefix and number them padded with 0's so they run in order
  - Example, assuming your spec file is called `FOO.md`, make issue files called `FOO_<nnnnnn>_name.md`, so that your plan steps are in order
  - Use the issue_create tool, specifying the name, again making sure they are named so that they run in order
- The final output needs to be multiple issues created with the issue_create tool.
