# CONFIG_000240: CLI Integration - sah.toml Configuration

Refer to ./specification/config.md

## Goal

Add CLI commands to validate, inspect, and debug sah.toml configurations, providing users with tools to manage their project configurations effectively.

## Tasks

1. **Add Validate Command**
   - Extend existing `sah validate` command to check sah.toml files
   - Validate TOML syntax and structure
   - Check variable name compliance (valid Liquid identifiers)
   - Verify environment variable syntax
   - Report configuration loading errors with line numbers

2. **Create Config Inspection Commands**
   - Add `sah config show` to display current configuration
   - Add `sah config variables` to list all available variables
   - Add `sah config test` to test template rendering with configuration
   - Support JSON/YAML output formats for machine consumption

3. **Environment Variable Integration**
   - Add `sah config env` to show environment variable usage
   - Display which environment variables are referenced
   - Show current values and defaults for environment variables
   - Warn about missing required environment variables

4. **Configuration Debugging**
   - Add verbose mode to show configuration loading process
   - Display file discovery path and resolution
   - Show variable precedence and overrides
   - Report template integration status

5. **Error Reporting Enhancement**
   - Improve error messages for configuration problems
   - Add suggestions for common configuration mistakes
   - Provide examples of correct configuration syntax
   - Include context about which templates use which variables

## Acceptance Criteria

- [ ] `sah validate` checks sah.toml files and reports errors clearly
- [ ] `sah config show` displays current configuration properly
- [ ] `sah config variables` lists all available variables
- [ ] `sah config test` allows testing template rendering
- [ ] Environment variable usage properly displayed
- [ ] Error messages are helpful and actionable
- [ ] CLI follows existing patterns and conventions

## Files to Create

- `swissarmyhammer-cli/src/config.rs` - Configuration CLI commands
- `swissarmyhammer/src/config/cli_support.rs` - CLI helper functions

## Files to Modify

- `swissarmyhammer-cli/src/main.rs` - Add config subcommands
- `swissarmyhammer-cli/src/validate.rs` - Extend validation
- `swissarmyhammer/src/config/mod.rs` - Export CLI support

## Next Steps

After completion, proceed to CONFIG_000241_comprehensive-testing for implementing thorough test coverage of the configuration system.