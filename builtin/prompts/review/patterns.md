---
title: Pattern Code Review
description: Perform a comprehensive review of the code to improve pattern use.
---

## Code Under Review

Please review the all code in this project.

## Review Checklist

### 1. Pattern Consistency Analysis

- **Architectural Patterns**: Does the code follow established patterns in the codebase (MVC, Repository, Factory, etc.)?
- **Naming Conventions**: Are variables, functions, classes, and files named consistently with existing code?
- **Code Organization**: Does file structure and module organization match project conventions?
- **Error Handling**: Is error handling implemented consistently across similar functions?
- **API Design**: Do new endpoints/interfaces follow existing API patterns?

### 2. Code Duplication Detection

- **Exact Duplication**: Identify identical or near-identical code blocks
- **Logic Duplication**: Find similar algorithms or business logic that could be abstracted
- **Configuration Duplication**: Spot repeated constants, magic numbers, or config values
- **Test Duplication**: Identify repeated test setup or assertion patterns
- **Suggest Refactoring**: Recommend specific abstractions (functions, classes, utilities, constants)

### 3. Consistency Violations

- **Formatting & Style**: Flag deviations from project code style (indentation, spacing, etc.)
- **Import/Dependency Patterns**: Ensure consistent module importing and dependency usage
- **Comment Styles**: Check documentation comment consistency
- **Technology Stack**: Verify new dependencies align with existing tech choices

### 4. Quality Improvements

- **Extract Methods**: Suggest breaking down large functions
- **Shared Utilities**: Identify opportunities to create reusable utility functions
- **Constants & Enums**: Recommend extracting magic values to shared constants
- **Type Safety**: Suggest stronger typing where applicable

## Review Format

### Overview

  Brief summary of changes and overall assessment

### Pattern Adherence ✅/❌

- List specific pattern compliance or violations
- Reference existing code examples where applicable

### Duplication Analysis 🔍

- Exact duplications found (with line references)
- Logic similarities that could be abstracted
- Specific refactoring suggestions

### Improvement Opportunities 🚀

- Concrete suggestions for better abstractions
- Recommendations for utility functions/classes
- Proposals for constant extraction


## Process

- list all source files in the project and use the todo tool to keep track of files to review
- for each file in the todo list
  - perform the Review Checklist
  - use the issue_create tool to create issues for each finding

{% render "review_format" %}
