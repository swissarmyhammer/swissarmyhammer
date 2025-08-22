# Update Documentation and Examples for Dynamic CLI

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Update all documentation, examples, and developer guidance to reflect the new dynamic CLI architecture and provide guidance for future MCP tool development.

## Technical Details

### Architecture Documentation
Create new documentation explaining the dynamic CLI system:

**Create `docs/dynamic-cli-architecture.md`:**
- Explain the MCP tool to CLI command mapping
- Document the schema-to-clap conversion process
- Provide guidelines for MCP tool CLI integration
- Include examples of CLI metadata implementation

### Developer Guide Updates
Update developer documentation for adding new tools:

**Update existing docs with new patterns:**
```markdown
# Adding New MCP Tools

To add a new MCP tool that appears in the CLI:

1. Implement the McpTool trait with CLI metadata:
```rust
impl McpTool for YourTool {
    fn name(&self) -> &'static str { "category_action" }
    fn cli_category(&self) -> Option<&'static str> { Some("category") }
    fn cli_name(&self) -> &'static str { "action" }
    fn cli_about(&self) -> Option<&'static str> { 
        Some("Brief description of what this command does") 
    }
    // ... other trait methods
}
```

2. Register the tool with the ToolRegistry
3. The command automatically appears in CLI as `sah category action`
```

### CLI Help Text Enhancement
Ensure dynamic CLI generation produces high-quality help:

**Category-Level Help:**
- Add category descriptions for major command groups
- Provide usage examples at category level
- Include common workflow guidance

**Tool-Level Help:**
- Ensure schema descriptions convert to clear help text
- Add examples to tool descriptions where helpful
- Verify parameter help is clear and actionable

### Migration Guide
Create migration guide for any external integrations:

**Create `docs/cli-migration-guide.md`:**
- Document any breaking changes (should be none for end users)
- Explain internal architecture changes for contributors
- Provide guidance for custom tool development

### README Updates
Update main README to reflect new architecture:

**Architecture Section Updates:**
- Replace static command documentation with dynamic generation explanation
- Update command examples to show current structure
- Add note about automatic CLI generation from MCP tools

**Development Section Updates:**
- Update contribution guidelines for MCP tools
- Remove references to static command enum maintenance
- Add guidance for CLI integration testing

### Code Examples
Update code examples throughout the codebase:

**Example Updates in Comments:**
```rust
// OLD: 
// To add a new command, update the Commands enum and add a handler

// NEW:
// To add a new CLI command, implement McpTool with CLI metadata
// The command will automatically appear in the CLI
```

**README Examples:**
- Update command usage examples
- Show new help output format
- Demonstrate category-based organization

### Testing Documentation
Document testing patterns for dynamic CLI:

**Update testing guidelines:**
- How to test MCP tools with CLI integration
- Schema conversion testing patterns  
- Integration testing for dynamic commands
- Validation testing approaches

### Error Message Documentation
Document the improved error handling:

**Error Guide:**
- Common schema validation errors and solutions
- Tool registration troubleshooting
- CLI generation debugging guidance
- User-friendly error interpretation

### Performance Documentation
Document any performance characteristics:

**Performance Notes:**
- CLI startup time with dynamic generation
- Tool registry initialization cost
- Schema conversion performance
- Recommendations for large tool collections

### Changelog
Update CHANGELOG.md with comprehensive notes:

**Major Changes Section:**
```markdown
## [Version X.Y.Z] - Date

### Changed
- 🔄 **BREAKING (Internal)**: Replaced static CLI command enums with dynamic generation from MCP tools
- ✨ CLI commands now automatically generated from MCP tool schemas
- 🗑️ Removed ~600+ lines of redundant command definition code
- 📚 Improved help text generation and consistency

### For Developers
- New MCP tools automatically appear in CLI when registered
- CLI metadata methods added to McpTool trait
- Simplified command addition process - no CLI code changes needed
- Enhanced schema validation and error handling

### For Users
- No functional changes - all commands work identically
- Improved help text consistency
- Better error messages for invalid commands
```

## Acceptance Criteria
- [ ] Architecture documentation explains dynamic CLI system
- [ ] Developer guide updated with new patterns
- [ ] Migration guide created for contributors
- [ ] README reflects new architecture
- [ ] Code comments updated throughout codebase
- [ ] Testing documentation updated
- [ ] Error handling documentation complete
- [ ] Performance characteristics documented
- [ ] Changelog updated with comprehensive notes
- [ ] All examples work with new system

## Implementation Notes
- Focus on developer experience - make it easy to add new tools
- Ensure documentation stays up-to-date with implementation
- Provide clear examples that can be copy-pasted
- Consider adding architectural diagrams
- This completes the user-facing aspects of the migration