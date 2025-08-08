# TypeScript Extractor TODO - Build Hierarchical Relationships

## Issue Location
File: `swissarmyhammer/src/outline/extractors/typescript.rs:1283`

## Description
There's a TODO comment in the TypeScript extractor about building proper hierarchical relationships for classes, interfaces, namespaces, etc.

## Current State
The `build_hierarchy` method currently returns symbols as-is without building relationships between:
- Classes and their methods/properties
- Interfaces and their members
- Namespaces and their contents
- Module hierarchies
- Nested types and their containers

## Expected Behavior
The TypeScript extractor should build proper hierarchical relationships to show the nested structure of TypeScript code, making the outline more useful for navigation and understanding code structure.

## Implementation Notes
This aligns with the hierarchical relationship work being done in other extractors (JavaScript, Python, Dart, Rust).