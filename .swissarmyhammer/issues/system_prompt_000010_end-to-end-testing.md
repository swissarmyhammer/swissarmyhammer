# End-to-End System Prompt Testing and Validation

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Comprehensive end-to-end testing to validate that the system prompt infrastructure works correctly across all use cases and that the removal of template includes doesn't degrade functionality.

## Prerequisites
- All previous system_prompt issues completed
- `.system.md` file renamed and working
- Template includes removed from all prompts
- CLI integration implemented and deployed

## Testing Scope

### 1. System Prompt Functionality
- **Rendering**: Verify `.system.md` renders with all includes (principals, coding_standards, tool_use)
- **Content Validation**: Confirm rendered content matches expected standards
- **Template Resolution**: Verify all template includes resolve correctly
- **Error Handling**: Test behavior with broken templates or missing includes

### 2. Claude Code Integration
- **Parameter Injection**: Verify `--append-system-prompt` receives rendered content
- **Content Delivery**: Confirm Claude Code receives complete system prompt
- **CLI Compatibility**: Ensure all existing CLI functionality works
- **Error Scenarios**: Test behavior when system prompt rendering fails

### 3. Prompt Functionality After Include Removal
Test each category of prompts that had template includes removed:

#### Root Prompts (3 files)
- `test.md` - Verify testing guidance works without explicit includes
- `coverage.md` - Confirm coverage analysis maintains quality
- `plan.md` - Ensure planning prompts provide adequate guidance

#### Review Prompts (6 files)  
- `review/security.md` - Security review quality maintained
- `review/code.md` - Code review effectiveness preserved
- `review/patterns.md` - Pattern analysis still comprehensive
- `review/placeholders.md` - Placeholder detection working
- `review/accessibility.md` - Accessibility review thorough

#### Issue Prompts (3 files)
- `issue/review.md` - Issue review process effective
- `issue/code.md` - Issue coding guidance adequate
- `issue/code_review.md` - Issue code review comprehensive

#### Documentation Prompts (5 files)
- `docs/review.md` - Documentation review quality maintained
- `docs/readme.md` - README generation meets standards  
- `docs/correct.md` - Documentation correction effective
- `docs/project.md` - Project documentation comprehensive
- `docs/comments.md` - Code comment guidance adequate

### 4. Workflow Integration Testing
- **Complete Workflows**: Test full workflows that use multiple prompts
- **Standards Application**: Verify coding standards apply consistently
- **Quality Maintenance**: Confirm output quality matches pre-change levels
- **Error Handling**: Test workflow behavior with system prompt issues

## Testing Methodology

### 1. Baseline Comparison
- **Before/After Analysis**: Compare prompt outputs before and after changes
- **Quality Metrics**: Measure consistency and completeness of guidance
- **Standards Coverage**: Verify all standards topics are still covered
- **Functionality Preservation**: Confirm no feature regression

### 2. Real-World Scenarios
- **Actual Code Review**: Use prompts on real code for quality assessment
- **Documentation Generation**: Generate documentation with updated prompts
- **Issue Workflows**: Complete full issue development cycles
- **Standards Compliance**: Verify generated content meets established standards

### 3. Edge Case Testing
- **Template Failures**: Test with broken system prompt templates
- **Missing Includes**: Test behavior with missing template partials
- **Rendering Errors**: Verify graceful degradation with rendering failures
- **CLI Errors**: Test system prompt integration error scenarios

## Success Criteria

### Functional Requirements
- ✅ System prompt renders correctly with all template includes
- ✅ Claude Code receives system prompt via `--append-system-prompt`
- ✅ All prompts work without explicit template includes
- ✅ No degradation in prompt quality or guidance
- ✅ Standards are effectively applied through system prompt

### Quality Requirements  
- ✅ Coding standards guidance maintained across all prompts
- ✅ Review quality remains high without explicit standards includes
- ✅ Documentation generation meets established standards
- ✅ Issue workflows maintain development quality
- ✅ Error handling works gracefully in all scenarios

### Performance Requirements
- ✅ No significant performance degradation
- ✅ System prompt rendering doesn't slow CLI operations
- ✅ Caching works effectively for repeated operations
- ✅ Resource usage remains within acceptable limits

## Test Deliverables
- Comprehensive test report covering all scenarios
- Performance analysis and benchmarks
- Quality comparison before/after changes
- Error handling validation results
- Recommendations for any needed adjustments

## Risk Assessment and Mitigation
- **Quality Degradation**: Monitor output quality and adjust if needed
- **Performance Issues**: Profile and optimize system prompt rendering
- **Integration Failures**: Comprehensive error handling and fallbacks
- **User Experience**: Ensure changes are transparent to end users