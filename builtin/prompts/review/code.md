---
name: code-review
title: Code Review
description: Review code for quality, bugs, and improvements
---

## Code Under Review

Please review the all code in this project.

## Review Checklist

1. **Code Quality**
   - Is the code readable and well-structured?
   - Are variable and function names descriptive?
   - Is the code properly documented?

2. **Potential Bugs**
   - Are there any logic errors?
   - Are edge cases handled?
   - Is error handling appropriate?

3. **Performance**
   - Are there any performance bottlenecks?
   - Can any algorithms be optimized?
   - Is memory usage efficient?

4. **Security**
   - Are there any security vulnerabilities?
   - Is input validation proper?
   - Are secrets handled correctly?

5. **Best Practices**
   - Does the code follow language idioms?
   - Is the code DRY (Don't Repeat Yourself)?
   - Are design patterns used appropriately?

Please provide specific feedback for each area of concern, including line numbers where applicable.

## Process

- list all source files in the project and create a markdown scratchpad file, this is your todo list
- create a CODE_REVIEW.md markdown file, this is your code review output
- for each file in the todo list
  - perform the Review Checklist
  - summarize your findings
  - write your findings to the code review output

{% render "review_format" %}
