# Final Validation and Cleanup

Refer to /Users/wballard/github/swissarmyhammer/ideas/config.md

## Objective

Perform final validation that the new figment-based configuration system fully meets the specification requirements, validate performance characteristics, and complete any remaining cleanup tasks.

## Context

This is the final step to ensure the configuration system migration is complete and successful. All previous steps should be implemented and tested before this validation phase.

## Specification Compliance Check

### Requirements Checklist

Verify each specification requirement is met:

- [ ] **Figment Integration**: System uses `figment` instead of custom parsing
- [ ] **Multiple File Formats**: Support for TOML, YAML, JSON formats  
- [ ] **File Discovery**: Finds both `sah.*` and `swissarmyhammer.*` files
- [ ] **Search Locations**: Checks `./.swissarmyhammer/` and `~/.swissarmyhammer/`
- [ ] **Precedence Order**: Defaults → global → project → env vars → CLI args
- [ ] **Environment Variables**: Support for `SAH_` and `SWISSARMYHAMMER_` prefixes
- [ ] **New Crate**: Configuration logic in `swissarmyhammer-config` crate
- [ ] **TemplateContext**: Uses proper context object instead of HashMap
- [ ] **No Caching**: Reads config fresh each time for live editing
- [ ] **Template Integration**: Works with prompts, workflows, and actions
- [ ] **Module Removal**: `sah_config` and `toml_config` modules eliminated
- [ ] **CLI Command Removal**: `sah config test` command removed

## Validation Tests

### 1. End-to-End Functionality Test

Create comprehensive end-to-end test in `tests/final_validation.rs`:

```rust
#[test]
fn test_complete_config_system() {
    let env = TestEnvironment::new().unwrap();
    
    // Set up complex configuration scenario
    env.write_global_config(r#"
        project_name = "GlobalProject"
        environment = "production"  
        timeout = 30
        database = { host = "global.db", port = 5432 }
    "#, ConfigFormat::Toml);
    
    env.write_project_config(r#"
        project_name = "ProjectOverride"
        debug = true
        database = { host = "project.db" }
        api_key = "${API_SECRET:-fallback_key}"
    "#, ConfigFormat::Yaml);
    
    env.set_env_var("SAH_ENVIRONMENT", "development");
    env.set_env_var("API_SECRET", "env_secret_123");
    
    // Test configuration loading
    let provider = ConfigProvider::new().unwrap();
    let context = provider.load_template_context().unwrap();
    
    // Verify precedence rules
    assert_eq!(context.get_string("project_name").unwrap(), "ProjectOverride");
    assert_eq!(context.get_string("environment").unwrap(), "development");
    assert_eq!(context.get_number("timeout").unwrap(), 30.0);
    assert_eq!(context.get_bool("debug").unwrap(), true);
    assert_eq!(context.get_string("api_key").unwrap(), "env_secret_123");
    
    // Test template rendering
    let template = "{{project_name}} in {{environment}} mode (debug: {{debug}})";
    let result = provider.render_template(template, None).unwrap();
    assert_eq!(result, "ProjectOverride in development mode (debug: true)");
    
    // Test workflow variable precedence  
    let mut workflow_vars = HashMap::new();
    workflow_vars.insert("environment".to_string(), json!("workflow_env"));
    
    let workflow_result = provider.render_template(template, Some(workflow_vars)).unwrap();
    assert_eq!(workflow_result, "ProjectOverride in workflow_env mode (debug: true)");
}
```

### 2. Performance Validation

```rust
#[test]
fn test_performance_meets_requirements() {
    let env = TestEnvironment::new().unwrap();
    env.write_project_config(&create_realistic_config(), ConfigFormat::Toml);
    
    // Test loading performance
    let start = Instant::now();
    let provider = ConfigProvider::new().unwrap();
    let context = provider.load_template_context().unwrap();
    let load_duration = start.elapsed();
    
    // Should load reasonably quickly (adjust threshold as needed)
    assert!(load_duration < Duration::from_millis(100));
    
    // Test rendering performance
    let template = create_complex_template();
    let start = Instant::now();
    for _ in 0..100 {
        let _result = provider.render_template(&template, None).unwrap();
    }
    let render_duration = start.elapsed();
    
    // Should render at acceptable speed
    assert!(render_duration < Duration::from_millis(500));
}
```

### 3. File Discovery Validation

```rust
#[test]
fn test_file_discovery_specification() {
    let env = TestEnvironment::new().unwrap();
    
    // Test all supported file name and location combinations
    let test_cases = [
        (".swissarmyhammer/sah.toml", ConfigScope::Project),
        (".swissarmyhammer/sah.yaml", ConfigScope::Project),
        (".swissarmyhammer/sah.yml", ConfigScope::Project),
        (".swissarmyhammer/sah.json", ConfigScope::Project),
        (".swissarmyhammer/swissarmyhammer.toml", ConfigScope::Project),
        ("~/.swissarmyhammer/sah.toml", ConfigScope::Global),
        // ... more combinations
    ];
    
    for (path, expected_scope) in test_cases {
        // Test that each file location is properly discovered
        env.write_config_at_path(path, "test_value = true");
        let discovery = FileDiscovery::new().unwrap();
        let files = discovery.discover_all().unwrap();
        
        assert!(files.iter().any(|f| f.scope == expected_scope));
        env.cleanup_config_at_path(path);
    }
}
```

## Compatibility Validation

### 4. Backward Compatibility Test

```rust
#[test]
fn test_existing_config_files_still_work() {
    // Test with real-world config files from the existing system
    let existing_configs = collect_existing_config_examples();
    
    for config_content in existing_configs {
        let env = TestEnvironment::new().unwrap();
        env.write_project_config(&config_content, ConfigFormat::Toml);
        
        let provider = ConfigProvider::new().unwrap();
        let result = provider.load_template_context();
        
        // Should load without errors
        assert!(result.is_ok(), "Failed to load existing config: {}", config_content);
    }
}
```

### 5. Template Rendering Compatibility

```rust
#[test]
fn test_template_rendering_identical_results() {
    // If possible, compare output with old system to ensure identical results
    let test_cases = load_template_test_cases();
    
    for (config, template, expected_output) in test_cases {
        let env = TestEnvironment::new().unwrap();
        env.write_project_config(&config, ConfigFormat::Toml);
        
        let provider = ConfigProvider::new().unwrap();
        let result = provider.render_template(&template, None).unwrap();
        
        assert_eq!(result, expected_output);
    }
}
```

## Code Quality Validation

### 6. Code Quality Checks

```bash
# Run all quality checks and ensure they pass
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings  
cargo nextest run --all-features
cargo doc --no-deps --all-features
```

### 7. Documentation Validation

- [ ] All public APIs have documentation
- [ ] Examples in documentation work correctly
- [ ] README files are updated
- [ ] Migration guide is complete and accurate

## Security Validation

### 8. Security Review

- [ ] No secrets or sensitive data in configuration files
- [ ] Path traversal protection in file discovery
- [ ] Environment variable handling is secure
- [ ] No unsafe code introduced
- [ ] Dependencies are from trusted sources

## Migration Validation

### 9. Migration Completeness

```bash
# Verify no old system references remain
rg "sah_config::" --type rust | wc -l  # Should be 0
rg "toml_config::" --type rust | wc -l  # Should be 0
rg "merge_config_into_context" --type rust | wc -l  # Should be 0 or only in compatibility layer

# Verify new system is used
rg "swissarmyhammer_config::" --type rust | wc -l  # Should be > 0
rg "TemplateContext" --type rust | wc -l  # Should be > 0
rg "ConfigProvider" --type rust | wc -l  # Should be > 0
```

### 10. Dependency Cleanup Validation

```bash
# Check that unused dependencies were removed
cargo machete  # If available, finds unused dependencies
cargo audit    # Check for security issues in dependencies
```

## Final Checklist

### Build and Test Validation
- [ ] `cargo build --all-features` succeeds
- [ ] `cargo nextest run --all-features` passes 100%
- [ ] `cargo clippy --all-targets --all-features` has no warnings
- [ ] `cargo fmt --all -- --check` succeeds
- [ ] `cargo doc --no-deps --all-features` builds documentation

### Functionality Validation  
- [ ] All configuration file formats work
- [ ] All precedence rules work correctly
- [ ] Environment variable substitution works
- [ ] Template rendering produces correct output
- [ ] File discovery finds all expected locations
- [ ] Error messages are helpful and clear

### Performance Validation
- [ ] Configuration loading is fast enough for interactive use
- [ ] Template rendering performance is acceptable
- [ ] Memory usage is reasonable
- [ ] No performance regression from old system

### Specification Compliance
- [ ] Every requirement in the specification is met
- [ ] No backward compatibility breaking changes (unless intended)
- [ ] All old modules and commands removed as specified

## Acceptance Criteria

- [ ] End-to-end functionality test passes
- [ ] Performance validation shows acceptable characteristics
- [ ] All specification requirements are verified as implemented
- [ ] Backward compatibility with existing config files maintained
- [ ] Code quality checks all pass
- [ ] Documentation is complete and accurate
- [ ] Security review shows no concerns
- [ ] Migration is completely finished
- [ ] All tests pass consistently
- [ ] Final cleanup is complete

## Files Changed

- `swissarmyhammer-config/tests/final_validation.rs` (new)
- Update any documentation that needs final corrections
- Final cleanup of any remaining unused imports or code

## Success Criteria

The configuration system migration is considered successful when:
1. All acceptance criteria above are met
2. The system works seamlessly for end users  
3. Performance is equal or better than the old system
4. All existing configuration files continue to work
5. The new system provides all the benefits outlined in the specification