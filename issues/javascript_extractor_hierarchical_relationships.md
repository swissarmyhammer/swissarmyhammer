# Build Proper Hierarchical Relationships for JavaScript Outline Extractor

## Location
`swissarmyhammer/src/outline/extractors/javascript.rs:658`

## Description
The JavaScript outline extractor needs proper hierarchical relationship building for classes and their members. Currently, symbols are extracted but not properly nested in a hierarchical structure.

## Current State
The code has a TODO comment indicating that hierarchical relationships need to be built for classes, etc.

## Requirements
- Implement proper parent-child relationships between classes and their methods
- Handle constructor functions and prototype-based inheritance
- Properly associate properties and methods with their containing classes
- Support ES6 classes and modern JavaScript patterns
- Handle nested functions and closures appropriately

## Acceptance Criteria
- [ ] Classes contain their methods and properties as children
- [ ] Constructor functions are properly identified
- [ ] Arrow functions and regular functions are correctly nested
- [ ] Module patterns are properly represented
- [ ] The outline accurately reflects JavaScript's flexible structure
- [ ] Unit tests verify the hierarchical structure