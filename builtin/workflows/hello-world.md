---
name: hello-world
title: Hello World Workflow
description: A simple workflow that demonstrates basic workflow functionality
category: builtin
tags:
  - example
  - basic
  - hello-world
---

# Hello World Workflow

This is a simple workflow that demonstrates basic workflow functionality.
It starts, greets the user, and then completes.

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> Greeting
    Greeting --> Complete
    Complete --> [*]
```

## Actions

- Start: Log "Starting hello-world workflow"
- Greeting: Execute prompt "say-hello" with result="greeting_output"
- Complete: Log "Workflow completed! Greeting was: ${greeting_output.content}"

## Description

This workflow demonstrates:

- Basic state transitions
- Simple logging actions
- A complete workflow lifecycle from start to finish

## Usage

To run this workflow:

```bash
swissarmyhammer flow run hello-world
```