# Remove Debug println! Statement in TreeSitter Parser

## Location
`swissarmyhammer/src/search/parser.rs:1907`

## Current State
There's a debug println! statement in the TreeSitter parser code that prints debugging information.

## Issue
Debug print statements should not be in production code. They should be replaced with proper logging using the `tracing` crate.

## Requirements
- Remove the println! statement at line 1907
- Remove the associated debug message at line 1908
- If the debug information is valuable, replace with appropriate tracing::debug! or tracing::trace! calls
- Ensure no console output pollution in production builds

## Implementation Approach
1. Review the debug statement to determine if the information is valuable
2. If valuable, replace with tracing::debug! or tracing::trace!
3. If not valuable for production, remove entirely
4. Test to ensure no regression in functionality