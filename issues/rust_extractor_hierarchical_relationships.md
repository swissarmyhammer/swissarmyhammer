# Rust Extractor TODO - Build Hierarchical Relationships

## Issue Location
File: `swissarmyhammer/src/outline/extractors/rust.rs:860`

## Description
There's a TODO comment in the Rust extractor about building proper hierarchical relationships for impl blocks, modules, etc.

## Current State
The `build_hierarchy` method currently returns symbols as-is without building relationships between:
- Impl blocks and their associated structs/enums
- Module hierarchies
- Nested functions and methods
- Associated types and their containers

## Expected Behavior
The Rust extractor should build proper hierarchical relationships to show the nested structure of Rust code, making the outline more useful for navigation and understanding code structure.

## Implementation Notes
This aligns with the hierarchical relationship work being done in other extractors (JavaScript, Python, Dart).