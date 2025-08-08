# Build Proper Hierarchical Relationships for Python Outline Extractor

## Location
`swissarmyhammer/src/outline/extractors/python.rs:840`

## Description
The Python outline extractor needs proper hierarchical relationship building for classes and their methods. Currently, the symbols are extracted but not properly nested in a hierarchical structure.

## Current State
The code has a TODO comment indicating that hierarchical relationships need to be built for classes and their methods.

## Requirements
- Implement proper parent-child relationships between classes and their methods
- Handle nested classes correctly
- Properly associate properties and methods with their containing classes
- Maintain correct indentation and scope information

## Acceptance Criteria
- [ ] Classes contain their methods as children in the outline
- [ ] Nested classes are properly represented in the hierarchy
- [ ] Properties and attributes are correctly associated with their classes
- [ ] The outline accurately reflects the code structure
- [ ] Unit tests verify the hierarchical structure