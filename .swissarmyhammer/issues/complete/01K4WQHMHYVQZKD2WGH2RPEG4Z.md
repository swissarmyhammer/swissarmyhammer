better to derive_builder on CliContext than always have to change the constructor

## Proposed Solution

After analyzing the CliContext struct in `swissarmyhammer-cli/src/context.rs`, I can see it has a complex constructor with 7 parameters:

```rust
pub async fn new(
    template_context: swissarmyhammer_config::TemplateContext,
    format: OutputFormat,
    format_option: Option<OutputFormat>,
    verbose: bool,
    debug: bool,
    quiet: bool,
    matches: clap::ArgMatches,
) -> Result<Self>
```

The current implementation requires callers to pass all 7 parameters in the correct order every time, making it error-prone and difficult to maintain when new fields are added.

### Implementation Steps:

1. **Add derive_builder dependency** to `swissarmyhammer-cli/Cargo.toml`
2. **Refactor CliContext struct** to use `#[derive(Builder)]` with appropriate builder configuration:
   - Use `#[builder(setter(into))]` for string-like fields
   - Use `#[builder(default)]` for optional fields with sensible defaults
   - Keep the async `new` method logic in a separate `build_async()` method
3. **Update all call sites** to use the builder pattern instead of direct constructor calls
4. **Test the changes** to ensure everything compiles and works correctly

This will make the code more maintainable and allow adding new fields to CliContext without breaking existing code.