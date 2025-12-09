# Implement semantic diagram validation

## Description
Add semantic validation beyond syntax parsing to ensure diagrams are logically correct and well-formed. This provides additional value beyond basic parsing.

## Requirements
1. Define validation rules for each diagram type
2. Implement validation trait/framework
3. Create comprehensive rule sets
4. Provide helpful validation messages
5. Support custom validation rules

## Validation Framework
```rust
pub trait DiagramValidator {
    type Diagram;
    type Error;
    
    fn validate(&self, diagram: &Self::Diagram) -> Result<(), Vec<Self::Error>>;
}

pub struct ValidationError {
    pub rule: &'static str,
    pub message: String,
    pub severity: Severity,
    pub location: Option<Location>,
}

pub enum Severity {
    Error,   // Must fix
    Warning, // Should fix
    Info,    // Consider fixing
}
```

## Validation Rules by Diagram Type

### Flowchart
- All nodes have at least one connection (except isolated)
- No circular dependencies in DAG mode
- Subgraph names are unique
- Style classes are defined

### Sequence
- All participants are declared
- Messages reference valid participants
- Activation blocks are balanced
- Loop/alt blocks are properly closed

### Class
- No circular inheritance
- Method signatures are valid
- Relationships reference existing classes
- No duplicate members

### State
- Start state exists
- Unreachable states warning
- Transition conditions are unique
- End states have no outgoing transitions

## Success Criteria
- Comprehensive validation rules
- Clear, actionable error messages
- Configurable severity levels
- Extensible for custom rules
- Performance impact minimal