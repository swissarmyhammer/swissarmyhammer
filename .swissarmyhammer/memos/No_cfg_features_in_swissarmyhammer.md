# No cfg features in swissarmyhammer

## Policy

The main `swissarmyhammer` crate should not use `#[cfg(feature = "...")]` conditional compilation features.

## Rationale

### Simplicity
- Features add complexity to build and test configurations
- Users shouldn't need to understand feature combinations
- Reduces the number of possible build configurations to test
- Avoids feature interaction bugs

### User Experience
- All functionality should be available by default
- No need to research which features to enable
- Consistent behavior across different installations
- Reduces support burden for feature-specific issues

### Maintenance
- Fewer code paths to maintain and test
- No need to test feature combinations
- Simpler CI/CD pipeline configuration
- Reduces documentation complexity

## Alternative Patterns

### Separate Crates
- Use separate crates for optional functionality
- Allow users to depend only on what they need
- Keep the main crate focused and minimal
- Use workspace dependencies for shared code

### Runtime Configuration
- Use configuration files for optional behavior
- Enable/disable features through environment variables
- Use plugin architectures for extensibility
- Implement feature detection at runtime

### Dependency Management
- Use optional dependencies in Cargo.toml when appropriate
- Keep core dependencies minimal
- Use dev-dependencies for testing tools
- Document all optional dependencies clearly

## Exceptions

### Development Features
- Feature flags are acceptable for development aids
- Use features for debugging and profiling tools
- Keep development features clearly separated
- Document development-only features

### Platform-Specific Code
- Use `#[cfg(target_os = "...")]` for platform differences
- Handle platform-specific functionality gracefully
- Provide fallbacks for unsupported platforms
- Test on all supported platforms

### Integration Features
- Features may be acceptable for large optional integrations
- Must provide clear value and reduce binary size significantly
- Should have minimal impact on core functionality
- Require explicit justification and approval