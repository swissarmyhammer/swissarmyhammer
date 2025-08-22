# Environment Variable Substitution Enhancement

Refer to /Users/wballard/github/swissarmyhammer/ideas/config.md

## Objective

Port and enhance the environment variable substitution functionality from the existing `sah_config` system, ensuring compatibility with the new TemplateContext while improving performance and maintainability.

## Context

The current system in `src/sah_config/template_integration.rs` supports environment variable substitution with `${VAR}` and `${VAR:-default}` patterns. This functionality must be preserved exactly while integrating with the new TemplateContext system.

## Current Substitution Behavior

From existing code analysis:
- Pattern: `${VAR_NAME}` - Replace with env var value, empty string if not set
- Pattern: `${VAR_NAME:-default}` - Replace with env var value, or default if not set  
- Supports substitution in:
  - String values
  - Array elements (recursively)
  - Object/table values (recursively)
- Uses regex: `\$\{([^}:]+)(?::-([^}]*))?\}` with thread-local caching

## Architecture

```mermaid
graph TD
    A[TemplateContext] --> B[substitute_env_vars]
    B --> C[EnvVarProcessor] 
    C --> D[Regex Matcher]
    C --> E[Environment Lookup]
    D --> F[${VAR} Pattern]
    D --> G[${VAR:-default} Pattern]
    E --> H[std::env::var]
    C --> I[Value Processor]
    I --> J[String Values]
    I --> K[Array Values]  
    I --> L[Object Values]
```

## Tasks

### 1. Environment Variable Processor

Create dedicated processor in `src/env_substitution.rs`:

```rust
/// Environment variable substitution processor
pub struct EnvVarProcessor {
    // Compiled regex for performance
    var_regex: regex::Regex,
}

impl EnvVarProcessor {
    /// Create new processor with compiled regex
    pub fn new() -> Result<Self, ConfigError> { ... }
    
    /// Process environment variable substitution in a value
    pub fn substitute_value(&self, value: &mut serde_json::Value) { ... }
    
    /// Process environment variable substitution in a string
    pub fn substitute_string(&self, s: &str) -> String { ... }
    
    /// Check if string contains substitution patterns
    pub fn contains_patterns(&self, s: &str) -> bool { ... }
}

// Thread-local instance for performance
thread_local! {
    static ENV_PROCESSOR: EnvVarProcessor = EnvVarProcessor::new()
        .expect("Failed to initialize environment variable processor");
}
```

### 2. Regex Pattern Matching

Implement the exact regex pattern matching:

```rust
impl EnvVarProcessor {
    /// Regex pattern for environment variables
    /// Matches: ${VAR_NAME} and ${VAR_NAME:-default_value}
    const ENV_VAR_PATTERN: &'static str = r"\$\{([^}:]+)(?::-([^}]*))?\}";
    
    /// Process regex matches and perform substitution  
    fn process_matches(&self, text: &str) -> String {
        self.var_regex.replace_all(text, |caps: &regex::Captures| {
            let var_name = &caps[1];
            match std::env::var(var_name) {
                Ok(value) => value,
                Err(_) => {
                    // Check for default value pattern ${VAR:-default}
                    if let Some(default_match) = caps.get(2) {
                        default_match.as_str().to_string()
                    } else {
                        String::new() // No default, return empty string
                    }
                }
            }
        }).to_string()
    }
}
```

### 3. Value Type Processing

Handle all JSON value types recursively:

```rust
impl EnvVarProcessor {
    /// Substitute in any JSON value type
    fn substitute_value_recursive(&self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(s) => {
                *s = self.substitute_string(s);
            }
            serde_json::Value::Array(arr) => {
                for item in arr.iter_mut() {
                    self.substitute_value_recursive(item);
                }
            }
            serde_json::Value::Object(obj) => {
                for (_, val) in obj.iter_mut() {
                    self.substitute_value_recursive(val);
                }
            }
            // Numbers, booleans, null don't need substitution
            _ => {}
        }
    }
}
```

### 4. TemplateContext Integration

Add methods to TemplateContext:

```rust
impl TemplateContext {
    /// Substitute environment variables in all template variables
    pub fn substitute_env_vars(&mut self) {
        ENV_PROCESSOR.with(|processor| {
            for value in self.vars.values_mut() {
                processor.substitute_value(value);
            }
        });
    }
    
    /// Get context with environment variables substituted (non-mutating)
    pub fn with_env_substitution(&self) -> TemplateContext {
        let mut cloned = self.clone();
        cloned.substitute_env_vars();
        cloned
    }
    
    /// Substitute environment variables in specific variable
    pub fn substitute_var(&mut self, key: &str) {
        if let Some(value) = self.vars.get_mut(key) {
            ENV_PROCESSOR.with(|processor| {
                processor.substitute_value(value);
            });
        }
    }
}
```

### 5. ConfigProvider Integration

Integrate with configuration loading:

```rust
impl ConfigProvider {
    /// Load template context with environment variable substitution
    pub fn load_template_context(&self) -> Result<TemplateContext, ConfigError> {
        let mut context = self.load_raw_context()?;
        context.substitute_env_vars();
        Ok(context)
    }
    
    /// Load without environment substitution (for debugging)
    pub fn load_raw_context(&self) -> Result<TemplateContext, ConfigError> { ... }
}
```

### 6. Performance Optimization

Optimize for common use cases:
- Use thread-local regex compilation
- Skip substitution if no patterns detected
- Lazy evaluation where possible
- Benchmark against existing implementation

### 7. Error Handling

Proper error handling for:
- Regex compilation failures
- Invalid environment variable patterns
- Circular substitution detection (future enhancement)

### 8. Comprehensive Testing

Create tests in `src/tests/env_substitution_tests.rs`:
- Basic ${VAR} substitution
- ${VAR:-default} substitution with defaults
- Missing variables (empty string behavior)
- Complex nested structures (arrays, objects)  
- Multiple variables in single string
- Edge cases (empty vars, special characters)
- Performance benchmarks
- Thread safety testing

## Acceptance Criteria

- [ ] EnvVarProcessor with thread-local caching
- [ ] Exact regex pattern matching from existing system
- [ ] Recursive processing of all JSON value types
- [ ] TemplateContext integration methods
- [ ] ConfigProvider integration
- [ ] Performance equivalent or better than existing system
- [ ] Comprehensive test coverage including edge cases
- [ ] Thread safety validation
- [ ] All tests passing with `cargo nextest run`
- [ ] Clean `cargo clippy` output
- [ ] Benchmarks showing performance characteristics

## Implementation Notes

- Maintain exact behavioral compatibility with existing system
- Use same regex pattern and replacement logic  
- Preserve thread-local caching for performance
- Handle edge cases the same way (empty strings, missing defaults)
- Add debug logging for troubleshooting substitution issues
- Consider caching compiled regex in static/lazy_static if thread_local causes issues

## Files Changed

- `swissarmyhammer-config/src/lib.rs` (add env_substitution module)
- `swissarmyhammer-config/src/env_substitution.rs` (new)
- `swissarmyhammer-config/src/context.rs` (add substitution methods)
- `swissarmyhammer-config/src/provider.rs` (integrate substitution)
- `swissarmyhammer-config/src/tests/env_substitution_tests.rs` (new)
- `swissarmyhammer-config/Cargo.toml` (ensure regex dependency)