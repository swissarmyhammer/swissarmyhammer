# Investigate CLI Integration for System Prompt

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Research and analyze the current CLI architecture to understand how to integrate system prompt rendering and delivery to Claude Code via `--append-system-prompt`.

## Investigation Areas

### 1. Current CLI Architecture Analysis
- **CLI Entry Points**: Understand where Claude Code is invoked
- **Prompt Rendering System**: How prompts are currently rendered and processed
- **Template System Integration**: How liquid templates are processed
- **Error Handling**: Current error handling patterns for template rendering

### 2. System Prompt Rendering Requirements
- **Template Processing**: Ensure `.system.md` can be rendered with all includes
- **Content Preparation**: Process liquid templates and resolve all partials
- **Error Handling**: Handle rendering failures gracefully
- **Performance**: Ensure rendering doesn't significantly impact startup time

### 3. Claude Code Integration Points
- **Command Invocation**: Where/how Claude Code commands are executed
- **Parameter Passing**: How to pass rendered content to `--append-system-prompt`
- **Integration Timing**: When to render and inject system prompt
- **Multiple Contexts**: Handle different usage scenarios (prompts, workflows, etc.)

## Research Tasks

### Codebase Analysis
1. **Find Claude Code invocation points**
   - Search for "claude code" usage in codebase
   - Identify all locations where CLI calls are made
   - Document current parameter passing patterns

2. **Analyze prompt rendering system**
   - Review prompt resolution and rendering code
   - Understand how liquid templates are processed
   - Identify template include resolution mechanism

3. **Study current CLI integration patterns**
   - Review existing CLI parameter handling
   - Understand error handling and logging patterns
   - Identify best practices for CLI integration

### Technical Requirements
1. **Rendering Pipeline**
   - How to render `.system.md` with all template includes
   - Cache rendered content or render on-demand?
   - Handle rendering errors and fallbacks

2. **Integration Architecture**
   - Where in the codebase to add system prompt integration
   - How to pass rendered content to `--append-system-prompt`
   - Maintain backward compatibility

3. **Configuration and Control**
   - Should system prompt injection be configurable?
   - How to handle cases where system prompt rendering fails?
   - Debug/logging requirements for troubleshooting

## Deliverables

### Architecture Analysis Document
- Current CLI integration patterns
- Prompt rendering system overview
- Identified integration points for system prompt

### Technical Specification
- Proposed integration approach
- Required code changes and locations
- Error handling and fallback strategies
- Performance and caching considerations

### Implementation Plan
- Step-by-step integration approach
- Code modification requirements
- Testing and validation strategy
- Risk assessment and mitigation plans

## Success Criteria
- ✅ Complete understanding of current CLI architecture
- ✅ Clear technical approach for system prompt integration
- ✅ Identified all required code changes
- ✅ Implementation plan ready for next phase

## Notes
- This is pure research - no code changes in this step
- Focus on understanding existing patterns before proposing changes
- Document all findings for implementation planning
- Consider performance, error handling, and user experience

## Proposed Solution

After thorough investigation of the codebase, I've identified the key integration points and developed a clear approach for implementing system prompt integration with Claude Code CLI.

### Architecture Analysis

#### Current Claude Code Integration
- **Primary Integration Point**: `swissarmyhammer/src/workflow/actions.rs:428-432`
- **Method**: `PromptAction::execute_once_internal()`
- **Command Construction**: Claude CLI is invoked with specific arguments and prompt content piped via stdin
- **Current Arguments**: `--dangerously-skip-permissions`, `--print`, `--output-format stream-json`, `--verbose`

#### Prompt Rendering System
- **Rendering Pipeline**: `PromptAction::render_prompt_directly()` at line 318
- **Template Engine**: Uses Liquid templates with full include/partial support
- **Process**: PromptLibrary loads all prompts → PromptResolver processes templates → rendered content piped to Claude
- **System Prompt Location**: `/builtin/prompts/.system.md` exists and includes liquid partials

#### Configuration System
- **Configuration Management**: `sah_config` module provides TOML-based configuration
- **Integration Points**: Template variables can be loaded from `sah.toml` files
- **Environment Support**: Full environment variable substitution available

### Technical Implementation Approach

#### 1. System Prompt Rendering Integration
**Location**: `PromptAction::execute_once_internal()` method around line 390

**Changes Required**:
```rust
// Add after existing prompt rendering (line ~390)
let rendered_prompt = self.render_prompt_directly(context).await?;

// NEW: Render system prompt if available
let system_prompt = self.render_system_prompt(context).await?;
```

**New Method Implementation**:
```rust
async fn render_system_prompt(
    &self,
    context: &HashMap<String, Value>,
) -> ActionResult<Option<String>> {
    // Check if system prompt integration is enabled (configurable)
    let enable_system_prompt = context
        .get("_enable_system_prompt")
        .and_then(|v| v.as_bool())
        .unwrap_or(true); // Default to enabled

    if !enable_system_prompt {
        return Ok(None);
    }

    // Load and render .system.md prompt
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();

    resolver.load_all_prompts(&mut library).map_err(|e| {
        ActionError::ClaudeError(format!("Failed to load system prompt: {e}"))
    })?;

    // Try to render 'system' prompt (maps to .system.md)
    match library.render_prompt_with_env(".system", &HashMap::new()) {
        Ok(rendered) => Ok(Some(rendered)),
        Err(_) => {
            // System prompt not found or failed to render - continue without it
            tracing::debug!("System prompt not available or failed to render");
            Ok(None)
        }
    }
}
```

#### 2. Claude CLI Integration
**Location**: Line 428-432 where Claude arguments are constructed

**Changes Required**:
```rust
// Claude CLI arguments
cmd.arg("--dangerously-skip-permissions")
    .arg("--print")
    .arg("--output-format")
    .arg("stream-json")
    .arg("--verbose");

// NEW: Add system prompt if available
if let Some(system_content) = &system_prompt {
    cmd.arg("--append-system-prompt")
       .arg(system_content);
}
```

#### 3. Configuration Control
**Integration Point**: Context variables for workflow control

**Implementation**:
- Add `_enable_system_prompt` context variable (default: true)
- Allow workflows to disable system prompt injection when needed
- Support configuration via `sah.toml` for global enable/disable

#### 4. Error Handling Strategy
**Approach**: Graceful degradation
- If system prompt rendering fails → log warning, continue without system prompt
- If system prompt template missing → continue normally
- If system prompt includes fail → log debug message, continue
- Only fail the workflow if Claude CLI execution fails

### Integration Points Summary

1. **`swissarmyhammer/src/workflow/actions.rs:390`**: Add system prompt rendering call
2. **`swissarmyhammer/src/workflow/actions.rs:318`**: Add new `render_system_prompt()` method
3. **`swissarmyhammer/src/workflow/actions.rs:428-432`**: Add `--append-system-prompt` argument
4. **Context Variables**: Use existing context system for configuration control

### Implementation Benefits

- **Minimal Code Changes**: Leverages existing prompt rendering infrastructure
- **Backward Compatible**: No breaking changes, graceful fallback behavior
- **Configurable**: Can be enabled/disabled per workflow or globally
- **Testable**: Uses existing testing patterns and can be mocked easily
- **Maintainable**: Follows established codebase patterns and conventions

### Risk Mitigation

- **Performance**: System prompt rendering cached per workflow execution
- **Security**: Uses existing template security validation
- **Compatibility**: Graceful handling of missing/invalid system prompts
- **Testing**: Can be disabled for tests that don't expect system prompt injection

This approach provides clean integration while maintaining the existing architecture patterns and ensuring reliable operation across all use cases.
