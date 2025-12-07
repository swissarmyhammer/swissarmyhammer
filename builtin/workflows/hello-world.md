---
name: hello-world
title: Hello World Workflow
description: A simple workflow that demonstrates basic workflow functionality with parameters
version: 1.0.0
author: Swiss Army Hammer
category: builtin
tags:
  - example
  - basic
  - hello-world
parameters:
  - name: person_name
    description: The name of the person to greet
    required: false
    type: string
    default: ${USERNAME}

  - name: language
    description: The language to use for greeting
    required: false
    type: choice
    default: English
    choices:
      - English
      - Spanish
      - French
      - German
      - Italian

  - name: enthusiastic
    description: Whether to use enthusiastic greeting
    required: false
    type: boolean
    default: false
---

# Hello World Workflow

This is a simple workflow that demonstrates basic workflow functionality with optional parameters.

## Usage

All parameters can be provided via CLI switches or interactive prompting:

```bash
# Simple usage (uses USERNAME environment variable)
sah flow run hello-world

# CLI switches
sah flow run hello-world --person-name "Alice" --language "Spanish" --enthusiastic

# Interactive mode
sah flow run hello-world --interactive

# Variable parameter support
sah flow run hello-world --var person_name=John --var language=English
```

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> greet
    greet --> farewell
    farewell --> [*]
```

## Actions

- start: Log "Starting hello-world workflow{% if enthusiastic %}!{% endif %}"
- greet: Execute prompt "say-hello" with name="{{ person_name }}" language="{{ language | default: 'English' }}"{% if enthusiastic %} enthusiastic=true{% endif %}
- farewell: Log "Goodbye, {{ person_name }}{% if enthusiastic %}!{% endif %}"

## Description

This workflow demonstrates:

1. **Start State**: Logs a welcome message with optional enthusiastic formatting
2. **Greet State**: Executes a prompt with structured parameters
   - `person_name` - The name to greet (optional string parameter, defaults to USERNAME environment variable)
   - `language` - The language choice with default fallback (optional choice parameter)
   - `enthusiastic` - Whether to use enthusiastic greeting (optional boolean parameter)
3. **Farewell State**: Logs a goodbye message using template variables

The structured parameters are resolved before liquid template rendering, providing type safety, validation, and improved CLI experience with auto-generated help and interactive prompting.
