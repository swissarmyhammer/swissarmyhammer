---
name: prompts-improve
title: Improve Existing Prompt
description: Analyze and enhance existing prompts for better effectiveness
parameters:
  - name: prompt_content
    description: The current prompt content (including YAML front matter)
    required: true
  - name: improvement_goals
    description: What aspects to improve (clarity, flexibility, effectiveness)
    required: false
    default: "overall enhancement"
  - name: user_feedback
    description: Any feedback or issues users have reported
    required: false
    default: ""
---

# Prompt Improvement Analysis

## Current Prompt
```
{{ prompt_content }}
```

## Improvement Goals
{{improvement_goals}}

{% if user_feedback %}
## User Feedback
{{user_feedback}}
{% endif %}

## Improvement Analysis

### 1. Current Prompt Assessment

#### Strengths
- Identify what works well
- Effective patterns used
- Clear instructions
- Good structure

#### Weaknesses
- Ambiguous instructions
- Missing edge cases
- Overly complex sections
- Insufficient examples

#### Opportunities
- Additional parameters
- Better defaults
- Enhanced flexibility
- Improved clarity

### 2. Prompt Engineering Principles

#### Clarity Enhancement
- **Specific Instructions**: Replace vague terms with concrete actions
- **Step-by-Step Guidance**: Break complex tasks into manageable steps
- **Clear Expectations**: Define what success looks like

#### Structure Optimization
- **Logical Flow**: Ensure natural progression
- **Section Organization**: Group related concepts
- **Visual Hierarchy**: Use formatting effectively

#### Flexibility Improvements
- **Parameterization**: Add useful variables
- **Conditional Logic**: Handle different scenarios
- **Sensible Defaults**: Reduce required configuration

### 3. Enhanced Prompt Design

#### Improved YAML Front Matter
- More descriptive title
- Comprehensive description
- Well-documented arguments
- Thoughtful defaults

#### Content Enhancements
- Clearer opening context
- Better variable usage
- Improved examples
- Enhanced output format

### 4. Specific Improvements

Based on the analysis, here are recommended changes:

1. **Argument Refinements**
   - Add missing optional parameters
   - Improve argument descriptions
   - Set better default values

2. **Content Structure**
   - Reorganize for clarity
   - Add missing sections
   - Improve transitions

3. **Template Usage**
   - Better variable placement
   - Add conditional sections
   - Use advanced features

4. **Examples and Guidance**
   - Include concrete examples
   - Add edge case handling
   - Provide troubleshooting tips

### 5. Improved Version

Here's the enhanced prompt:

```markdown
[Provide the complete improved prompt with all enhancements applied]
```

### 6. Testing Recommendations

Before deploying the improved prompt:
1. Test with various inputs
2. Verify edge case handling
3. Compare outputs with original
4. Gather user feedback
5. Iterate based on results

### 7. Migration Notes

If this prompt is already in use:
- Document breaking changes
- Provide migration guide
- Consider versioning strategy
- Communicate changes to users