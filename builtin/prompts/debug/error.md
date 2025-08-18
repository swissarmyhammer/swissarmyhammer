---
name: debug-error
title: Debug Error Messages
description: Analyze error messages and provide debugging guidance with potential solutions
parameters:
  - name: error_message
    description: The error message or stack trace to analyze
    required: true
  - name: language
    description: The programming language (e.g., rust, python, javascript)
    required: false
    default: "auto-detect"
  - name: context
    description: Additional context about when the error occurs
    required: false
    default: ""
---

# Debugging Error: {{error_message}}

I'll help you debug this error in {{language}}.

## Error Analysis

Let me analyze the error message: {{error_message}}

{% if context %}
### Additional Context
{{context}}
{% endif %}

## Steps to Debug

1. **Understand the Error**
   - Parse the error message carefully
   - Identify the error type and location
   - Look for line numbers and file references

2. **Common Causes**
   - Check for the most common causes of this error type
   - Verify assumptions about data types and values
   - Look for edge cases

3. **Debugging Approach**
   - Add logging/print statements near the error
   - Use a debugger to step through the code
   - Isolate the problem with minimal test cases

4. **Potential Solutions**
   - Provide specific fixes based on the error pattern
   - Suggest defensive programming techniques
   - Recommend best practices to prevent similar errors

## Example Usage

This prompt is most effective when you provide:
- The complete error message or stack trace
- The programming language (if not obvious from the error)
- Any relevant context about what you were trying to do

For example:
- `error_message`: "TypeError: cannot read property 'name' of undefined"
- `language`: "javascript"
- `context`: "Happens when processing user data from API"