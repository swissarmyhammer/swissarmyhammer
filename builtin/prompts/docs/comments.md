---
name: docs-comments
title: Generate Code Comments
description: Add comprehensive comments and documentation to code
---

## Goal

Create high quality documentation comments in source.


{% render "documentation" %}

{% render code %}

## Documentation Strategy

### 1. Comment Types

#### File/Module Level

- Purpose and responsibility
- Author and maintenance info
- Dependencies and requirements
- Usage examples

#### Class/Interface Level

- Design decisions
- Invariants and contracts
- Relationships to other components
- Thread safety considerations

#### Method/Function Level

- Purpose and behavior
- Parameters and return values
- Side effects and exceptions
- Usage examples
- Complexity notes

#### Implementation Comments

- Non-obvious logic explanation
- Algorithm choices
- Performance considerations
- Bug workarounds

### 2. Documentation Standards

#### JSDoc Format

```javascript
/**
 * Brief description of the function.
 * 
 * @param {Type} paramName - Parameter description
 * @returns {Type} Return value description
 * @throws {ErrorType} When this error occurs
 * @example
 * // Example usage
 * functionName(args);
 */
```

#### Python Docstring Format

```python
"""Brief description of the function.

Longer description if needed.

Args:
    param_name (Type): Parameter description
    
Returns:
    Type: Return value description
    
Raises:
    ErrorType: When this error occurs
    
Examples:
    >>> function_name(args)
    expected_output
"""
```

#### Rust Documentation Format

```rust
/// Brief description of the function.
/// 
/// Longer description if needed.
/// 
/// # Arguments
/// 
/// * `param_name` - Parameter description
/// 
/// # Returns
/// 
/// Return value description
/// 
/// # Examples
/// 
/// ```
/// let result = function_name(args);
/// ```
```

### 3. Best Practices

#### What to Document

- Public APIs thoroughly
- Complex algorithms
- Non-obvious decisions
- Workarounds and hacks
- Performance considerations

#### What NOT to Document

- Obvious code
- Language features
- Redundant information

#### Writing Style

- Clear and concise
- Active voice
- Present tense
- Consistent terminology

### 4. Detail Level Documentation

Include:

- Detailed parameter descriptions
- Multiple examples
- Edge cases
- Performance notes
- Related references

### 5. Generated Documentation

Provide the code with appropriate comments added according to the specified style and detail level.
