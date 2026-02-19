---
name: implement
description: Implement a feature or fix directly from a description. Use when the user describes what they want built without a formal spec or plan.
allowed-tools: "*"
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Implement

Implement a feature or fix directly from a user description.

## How to Execute

1. Analyze the user's request in $ARGUMENTS
2. Search the codebase to understand the relevant architecture
3. Make the necessary code changes
4. Run tests to verify the implementation

## What Happens

1. Understands the requested change from the description
2. Explores the codebase to find relevant files and patterns
3. Implements the changes following existing code conventions
4. Runs the test suite to verify nothing is broken
5. Reports what was changed and the test results
