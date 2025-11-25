# No cfg features in swissarmyhammer

## Policy

The main `swissarmyhammer` crate should not use `#[cfg(feature = "...")]` conditional compilation features.

## Exceptions

### Testing
- Feature flags are acceptable for `test`

### Platform-Specific Code
- Use `#[cfg(target_os = "...")]` for platform differences
- Handle platform-specific functionality gracefully
- Provide fallbacks for unsupported platforms
- Test on all supported platforms
