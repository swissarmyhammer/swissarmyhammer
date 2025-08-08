# Remove Dart Extractor TODO - Build Hierarchical Relationships

## Location
`swissarmyhammer/src/outline/extractors/dart.rs:1067`

## Current State
There's a TODO comment in the Dart extractor about building proper hierarchical relationships for classes, mixins, etc.

## Issue
The Dart extractor currently returns symbols as-is without building the proper parent-child relationships between classes and their members.

## Requirements
- Implement the `build_hierarchy` method to properly structure:
  - Classes and their members
  - Mixins and their contents
  - Extensions and their methods
  - Abstract classes
  - Factory constructors
  - Named constructors

## Implementation Approach
1. Track Dart-specific constructs (mixins, extensions)
2. Associate constructors (including factory and named) with their classes
3. Handle getter/setter pairs
4. Group extension methods under their extensions
5. Support mixin applications
6. Handle library-level declarations