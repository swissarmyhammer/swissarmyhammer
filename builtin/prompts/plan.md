---
title: plan
description: Generate a step by step development plan from a specification.
---

## Goal

Turn a specification into a multiple step plan.

Generate a multiple step plan in the `./issues` folder of multiple `<nnnnnn>_step.md` markdown step files, one for each step in order.

{% render "principals" %}
{% render "coding_standards" %}

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
- Each issue you create that is a step in the plan should include the phrase "Refer to ./specification/<specific plan file read>"

## Process

- Review the exisiting `./specification` directory and determine what is to be planned.
- Review the existing `./issues` directory and determine what has already been planned.
- Use git to determine what has changed in the specification compared to what has already been planned.
- Review the existing code to determine what parts of the specification might already be implemented.
- Draft a detailed, step-by-step plan to meet the specification.
- If anything is ambiguous, STOP and ask the user clarifying questions.
- Then, once you have a solid plan, break it down into small, iterative chunks that build on each other incrementally.
- Look at these chunks and then go another round to break it into small steps.
- From here you should have the foundation to provide an in order series of issue files that describes the work to do at each step
- When creating issue steps for the plan, make sure to prefix and number them padded with 0's so they run in order
  - Example, assuming your spec is called `FOO`, make issue files called `FOO_<nnnnnn>_name.md`
- Review the results and make sure that the steps are small enough to be implemented safely, but big enough to move the project forward
- Iterate until you feel that the steps are right sized for this project.
