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