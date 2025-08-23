# Implement System Prompt Rendering Infrastructure

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Implement the core infrastructure to render the `.system.md` file with all template includes, preparing it for integration with Claude Code's `--append-system-prompt` parameter.

## Prerequisites
- Issue system_prompt_000007 (CLI integration investigation) completed
- `.system.md` file exists and renders correctly
- Understanding of current prompt rendering architecture

## Implementation Components

### 1. System Prompt Renderer
- **Function**: Render `.system.md` with all template includes resolved
- **Input**: `.system.md` file path
- **Output**: Fully rendered system prompt content as string
- **Error Handling**: Graceful handling of template rendering failures

### 2. Template Resolution
- **Partial Includes**: Ensure principals, coding_standards, tool_use render correctly
- **Variable Substitution**: Handle any liquid variables in the system prompt
- **Nested Includes**: Support any nested template includes
- **Error Context**: Provide clear error messages for template failures

### 3. Caching Strategy
- **Performance**: Cache rendered system prompt to avoid repeated processing
- **Cache Invalidation**: Invalidate cache when template files change
- **Memory Management**: Efficient caching that doesn't consume excessive memory
- **Debug Mode**: Option to bypass cache for debugging/development

### 4. Integration API
- **Public Interface**: Clean API for other components to get rendered system prompt
- **Error Handling**: Return Result<String> with detailed error information
- **Configuration**: Support for enabling/disabling system prompt injection
- **Logging**: Appropriate logging for debugging and troubleshooting

## Technical Specifications

### Function Signature (Rust)
```rust
pub fn render_system_prompt() -> Result<String, SystemPromptError> {
    // Implementation details
}

pub enum SystemPromptError {
    TemplateNotFound,
    RenderingFailed(String),
    PartialNotFound(String),
    // Other error variants
}
```

### Integration Points
- **CLI Commands**: Where system prompt rendering is called
- **Workflow Execution**: Integration with workflow systems
- **Prompt Resolution**: Integration with existing prompt systems
- **Configuration**: System-wide configuration for system prompt behavior

## Implementation Steps

1. **Create system prompt rendering module**
   - Define core functions and error types
   - Implement basic template rendering for .system.md
   - Add error handling and logging

2. **Template resolution enhancement**
   - Ensure all template includes resolve correctly
   - Handle nested includes and complex template structures
   - Add comprehensive error reporting

3. **Performance optimization**
   - Implement caching strategy
   - Add cache invalidation logic
   - Performance testing and optimization

4. **API integration**
   - Create clean public interface
   - Add configuration options
   - Integration with existing prompt systems

## Success Criteria
- ✅ System prompt renders completely with all includes
- ✅ Template includes (principals, coding_standards, tool_use) resolve correctly
- ✅ Comprehensive error handling for all failure scenarios
- ✅ Performance optimizations (caching) implemented
- ✅ Clean API ready for CLI integration
- ✅ Thorough unit tests covering all functionality

## Testing Requirements
- Unit tests for template rendering
- Integration tests with actual .system.md file
- Error handling tests for various failure scenarios
- Performance tests for rendering speed
- Cache behavior tests

## Technical Notes
- Build on existing prompt rendering infrastructure
- Maintain consistency with current template processing patterns
- Ensure no breaking changes to existing prompt functionality
- Consider future extensibility for system prompt customization