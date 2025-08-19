---
name: Example
title: Example Prompt Template
description: A comprehensive example showing how to create effective prompts with arguments and templates
category: examples
tags:
  - example
  - template
  - demo
  - tutorial
parameters:
  - name: task
    description: The task or operation to demonstrate
    required: true
    default: code review
  - name: language
    description: Programming language for examples
    required: false
    default: Python
  - name: complexity
    description: Complexity level (simple, moderate, advanced)
    required: false
    default: moderate
  - name: project_name
    description: Name of the project being worked on
    required: false
    default: Example Project
---

# Example Prompt: {{ task | capitalize }}

This is an example prompt demonstrating Swiss Army Hammer features.

## Task Details
**Task**: {{ task }}
**Language**: {{ language }}
**Complexity**: {{ complexity }}
**Project**: {{ project_name | default: "Example Project" }}

## Template Features Demonstrated

### 1. Arguments
This prompt uses several arguments:
- `task` (required): {{ task }}
- `language` (optional): {{ language }} 
- `complexity` (optional): {{ complexity }}

### 2. Conditional Logic
{% if complexity == "simple" %}
This is a simple {{ task }} task suitable for beginners.
{% elsif complexity == "advanced" %}
This is an advanced {{ task }} task requiring expertise.
{% else %}
This is a moderate {{ task }} task for intermediate users.
{% endif %}

### 3. Custom Filters (Available in TemplateEngine)
- Slugified task: `task-name-becomes-slug`
- Line count example: Shows number of lines in text

### 4. Environment Variables
- User: {{ env.USER | default: "unknown" }}
- Home directory: {{ env.HOME | default: "/home/user" }}

## Instructions for {{ task }}

{% if task contains "review" %}
Please perform a thorough code review focusing on:
1. Code quality and style
2. Performance considerations  
3. Security implications
4. Documentation completeness
{% elsif task contains "test" %}
Please create comprehensive tests including:
1. Unit tests for core functionality
2. Integration tests for workflows
3. Edge case validation
4. Performance benchmarks
{% else %}
Please complete the {{ task }} task with attention to:
1. Best practices for {{ language }}
2. Clear documentation
3. Error handling
4. Performance optimization
{% endif %}

## Expected Output Format
Provide your response in a structured format appropriate for {{ complexity }} level work in {{ language }}.

---
*This example demonstrates Swiss Army Hammer's template capabilities including arguments, conditionals, filters, and environment variable access.*