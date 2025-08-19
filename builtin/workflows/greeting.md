---
title: Greeting Workflow
description: A workflow that generates personalized greetings in multiple languages
version: 1.0.0
author: Swiss Army Hammer
tags:
  - example
  - template
  - greeting
parameters:
  - name: person_name
    description: The name of the person to greet
    required: true
    type: string
    
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

# Greeting Workflow

This workflow generates personalized greetings in multiple languages.

## Usage

All parameters can be provided via CLI switches or interactive prompting:

```bash
# CLI switches
sah flow run greeting --person-name "Alice" --language "Spanish" --enthusiastic

# Interactive mode
sah flow run greeting --interactive

# Variable parameter support
sah flow run greeting --var person_name=John --var language=English
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

- start: Log "Starting greeting workflow{% if enthusiastic %}!{% endif %}"
- greet: Execute prompt "say-hello" with name="{{ person_name }}" language="{{ language | default: 'English' }}"{% if enthusiastic %} enthusiastic=true{% endif %}
- farewell: Log "Goodbye, {{ person_name }}{% if enthusiastic %}!{% endif %}"

## Description

This workflow showcases the use of structured parameters and liquid template variables in action strings:

1. **Start State**: Logs a welcome message with optional enthusiastic formatting
2. **Greet State**: Executes a prompt with structured parameters
   - `person_name` - The name to greet (required string parameter)
   - `language` - The language choice with default fallback (optional choice parameter)
   - `enthusiastic` - Whether to use enthusiastic greeting (optional boolean parameter)
3. **Farewell State**: Logs a goodbye message using template variables

The structured parameters are resolved before liquid template rendering, providing type safety, validation, and improved CLI experience with auto-generated help and interactive prompting.