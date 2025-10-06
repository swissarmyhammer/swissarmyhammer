# Implement full conditional parameter validation

## Locations
- `swissarmyhammer-common/src/parameters.rs:983` - Condition references parameters we don't have yet
- `swissarmyhammer-common/src/parameters.rs:1000` - Using original prompting system for now
- `swissarmyhammer-common/src/parameters.rs:1080` - Assume not required if condition can't be evaluated

## Current State
Multiple locations skip or make assumptions about conditional parameters when conditions cannot be fully evaluated.

## Description
Conditional parameter validation has several incomplete areas where conditions are skipped or assumptions are made. This should be properly implemented to handle all conditional parameter scenarios.

## Requirements
- Design proper condition evaluation order
- Handle forward references to parameters not yet collected
- Implement multi-pass validation when needed
- Provide clear errors for circular dependencies
- Support partial evaluation when appropriate
- Add comprehensive tests for conditional scenarios
- Document conditional parameter behavior

## Use Cases
- Parameters that depend on other parameter values
- Dynamic required/optional based on selections
- Conditional defaults and validation rules

## Impact
Conditional parameters may not validate correctly, leading to runtime errors.