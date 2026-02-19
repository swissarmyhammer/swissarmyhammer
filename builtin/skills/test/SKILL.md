---
name: test
description: Run tests and analyze results. Use when the user wants to run the test suite or test specific functionality.
allowed-tools: "*"
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Test

Run the project test suite and analyze results.

## How to Execute

Use the `shell_execute` tool to run tests:

    command: "$ARGUMENTS"

If no arguments provided, detect the project type and run the appropriate test command:
- Rust: `cargo test`
- Node.js: `npm test` or `yarn test`
- Python: `pytest`

## What Happens

1. Detects the project type if no specific test command given
2. Runs the appropriate test command
3. Analyzes test output for failures
4. Reports results with actionable information about failures
