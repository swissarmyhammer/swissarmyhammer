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
## Proposed Solution

Based on analysis of the existing template system, I will implement the system prompt rendering infrastructure as follows:

### Architecture Overview
- Build on existing `template.rs` infrastructure with `PromptLibrary` and `PromptPartialSource`
- Create dedicated `system_prompt.rs` module in `swissarmyhammer/src/`
- Use existing Liquid template engine with partial support for includes like `{% render "principals" %}`

### Implementation Components

1. **System Prompt Renderer (`system_prompt.rs`)**
   ```rust
   pub struct SystemPromptRenderer {
       template_engine: TemplateEngine,
       prompt_library: Arc<PromptLibrary>,
       cache: Option<(String, SystemTime)>,
   }
   
   pub fn render_system_prompt() -> Result<String, SystemPromptError>
   ```

2. **Integration with Existing Template System**
   - Use `Template::with_partials()` for handling `{% render "principals" %}` includes
   - Leverage `PromptPartialSource` for resolving partial templates
   - Use `render_with_config()` for full variable substitution

3. **Caching Strategy**
   - In-memory cache with file modification time checking
   - Cache key based on `.system.md` and all referenced partials modification times
   - Cache invalidation when any template file changes

4. **Error Handling**
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum SystemPromptError {
       #[error("System prompt file not found: {0}")]
       FileNotFound(String),
       #[error("Template rendering failed: {0}")]
       RenderingFailed(String),
       #[error("Partial template not found: {0}")]
       PartialNotFound(String),
       #[error("IO error: {0}")]
       IoError(#[from] std::io::Error),
   }
   ```

### Implementation Steps

1. **Create system_prompt.rs module** with core types and renderer struct
2. **Implement render_system_prompt()** function using existing template infrastructure  
3. **Add caching logic** with modification time-based invalidation
4. **Comprehensive error handling** for all failure scenarios
5. **Unit tests** covering rendering, caching, and error cases

### Integration Points
- The rendered system prompt will be available via `SystemPromptRenderer::render_system_prompt()`
- Ready for CLI integration with `--append-system-prompt` parameter
- Maintains consistency with existing prompt rendering patterns
## Implementation Status: ✅ COMPLETED

The system prompt rendering infrastructure has been successfully implemented and tested. All requirements have been met:

### ✅ Completed Features

**1. System Prompt Renderer (`system_prompt.rs`)**
- ✅ Created dedicated module with comprehensive functionality
- ✅ `SystemPromptRenderer` struct with initialization from multiple prompt sources
- ✅ `render_system_prompt()` public function for easy access
- ✅ Integrated with existing `TemplateEngine` and `PromptLibrary` infrastructure

**2. Template Resolution**
- ✅ Full support for `{% render "partial_name" %}` includes
- ✅ Resolves `principals`, `coding_standards`, `tool_use` partials successfully
- ✅ Handles various file extensions (.md, .markdown, .liquid, .md.liquid)
- ✅ Flexible whitespace handling in template syntax
- ✅ Integration with configuration variables via `render_with_config()`

**3. Caching Strategy** 
- ✅ In-memory cache with file modification time tracking
- ✅ Automatic cache invalidation when template files change
- ✅ Tracks modification times of all referenced partials
- ✅ Thread-safe caching with mutex protection

**4. Comprehensive Error Handling**
- ✅ `SystemPromptError` enum with specific error types
- ✅ `FileNotFound`, `RenderingFailed`, `PartialNotFound`, `IoError` variants
- ✅ Conversion from `SwissArmyHammerError` for seamless integration
- ✅ Clear error messages with context information

**5. API Integration Ready**
- ✅ Public `render_system_prompt()` function returns `Result<String, SystemPromptError>`
- ✅ Exported in `lib.rs` main module and prelude for easy access
- ✅ `clear_cache()` utility function for testing and cache management
- ✅ Ready for CLI integration with `--append-system-prompt` parameter

**6. Comprehensive Testing**
- ✅ 8 unit tests covering all major functionality
- ✅ Regex pattern testing for partial extraction
- ✅ Error handling validation
- ✅ Cache management testing
- ✅ File modification time tracking
- ✅ All tests pass successfully

### 🏗️ Technical Implementation Details

**File Locations:**
- Main implementation: `swissarmyhammer/src/system_prompt.rs`
- Module integration: `swissarmyhammer/src/lib.rs` (lines 49, 148-149, 234-237)

**Key Components:**
```rust
// Public API
pub fn render_system_prompt() -> Result<String, SystemPromptError>
pub fn clear_cache()

// Core types
pub struct SystemPromptRenderer
pub enum SystemPromptError

// Cache management with file modification tracking
static SYSTEM_PROMPT_CACHE: Mutex<Option<CacheEntry>>
```

**Search Paths for `.system.md`:**
1. `builtin/prompts/.system.md` (✅ exists)
2. `.swissarmyhammer/prompts/.system.md` 
3. `prompts/.system.md`
4. `.system.md`

**Template Partials:**
- ✅ `principals` → `builtin/prompts/principals.md.liquid`
- ✅ `coding_standards` → `builtin/prompts/coding_standards.md.liquid`
- ✅ `tool_use` → `builtin/prompts/tool_use.md.liquid`

### 🎯 Ready for Next Step

The implementation is ready for integration with issue system_prompt_000009 (CLI integration). The rendered system prompt can be accessed via:

```rust
use swissarmyhammer::system_prompt::render_system_prompt;

let system_prompt = render_system_prompt()?;
// Use with --append-system-prompt flag
```

All success criteria have been achieved with comprehensive testing and robust error handling.