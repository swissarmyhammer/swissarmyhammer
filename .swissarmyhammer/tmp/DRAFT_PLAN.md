# DRAFT PLAN: Remove issue_current and issue_next Tools

## Overview
This plan removes two redundant MCP tools (`issue_current` and `issue_next`) and consolidates their functionality into the existing `issue_show` tool with special parameter handling.

## Analysis
- Current state: Two separate tools for getting current/next issues
- Desired state: Single `issue_show` tool with `"current"` and `"next"` special parameters
- Impact: 4 builtin prompt files need updating, 2 tool implementations need removal
- Benefits: Reduced API surface, consistent interface, less maintenance burden

## High-Level Steps

### Phase 1: Preparation and Analysis
1. **Audit Current Usage**: Search codebase for all references to `issue_current` and `issue_next`
2. **Understand Current Implementation**: Read and analyze the logic in both tools to be removed
3. **Identify Test Coverage**: Find all tests that need updating

### Phase 2: Core Implementation
4. **Enhance issue_show Tool**: Add special parameter handling for "current" and "next"
5. **Update Tool Description**: Document new functionality in description.md
6. **Add Comprehensive Tests**: Test new functionality and edge cases

### Phase 3: Migration and Cleanup  
7. **Update Builtin Prompts**: Replace tool calls in 4 prompt files
8. **Remove Old Tools**: Delete tool implementations and registry entries
9. **Update Tests**: Fix any broken tests from removed tools

### Phase 4: Verification
10. **Integration Testing**: Verify workflows still function correctly
11. **Final Cleanup**: Remove any remaining dead code or references

## Risk Analysis
- Low risk: Functionality is being consolidated, not removed
- Main risk: Breaking existing workflows that use the prompts
- Mitigation: Thorough testing of updated prompts before cleanup

## Size Estimation
- Small to medium sized changes
- Most work is in consolidation rather than new feature development
- Should be implementable in incremental steps