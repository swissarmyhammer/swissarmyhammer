---
name: review-accessibility
title: Accessibility Review
description: Review code for accessibility compliance and best practices
parameters:
  - name: wcag_level
    description: WCAG compliance level target (A, AA, AAA)
    required: false
    default: "AA"
  - name: target_users
    description: Specific user needs to consider
    required: false
    default: "all users"
  - name: component_type
    description: Type of component being reviewed (form, navigation, interactive, general)
    required: false
    default: "general"
---

## Code Under Review

Review all code in the current project.

{% render "principals" %}
{% render "coding_standards" %}

## Review Parameters

- **WCAG Level**: {{wcag_level}}
- **Component Type**: {{component_type}}
- **Target Users**: {{target_users}}

## Accessibility Audit

### 1. WCAG {{wcag_level}} Compliance

#### Perceivable

- **Text Alternatives**: Alt text for images, icons
- **Color Contrast**: Minimum ratios met
- **Text Resize**: Supports up to 200% zoom
- **Audio/Video**: Captions and transcripts

#### Operable

- **Keyboard Access**: All interactive elements reachable
- **Focus Indicators**: Visible focus states
- **Skip Links**: Navigation aids
- **Time Limits**: Adjustable or removable

#### Understandable

- **Labels**: Clear form labels and instructions
- **Error Messages**: Descriptive and helpful
- **Consistent Navigation**: Predictable UI
- **Language**: Proper language attributes

#### Robust

- **Valid HTML**: Semantic markup
- **ARIA Usage**: Correct implementation
- **Browser Support**: Cross-browser testing
- **Assistive Technology**: Screen reader compatible

### 2. Component-Specific Issues

{% if component_type == "form" %}
#### Form Accessibility

- Label association
- Error handling
- Required field indicators
- Fieldset/legend usage
- Autocomplete attributes
{% elsif component_type == "navigation" %}
#### Navigation Accessibility

- Landmark roles
- Menu structure
- Breadcrumbs
- Active state indication
- Mobile navigation
{% elsif component_type == "interactive" %}
#### Interactive Element Accessibility

- Button vs link usage
- State announcements
- Loading indicators
- Modal focus management
- Tooltip accessibility
{% endif %}

### 3. Assistive Technology Support

#### Screen Readers

- Proper reading order
- Meaningful announcements
- Context preservation
- Dynamic content updates

#### Keyboard Navigation

- Tab order logic
- Shortcut conflicts
- Focus trapping
- Escape key handling

### 4. Recommendations

#### Critical Fixes

- Must-fix accessibility barriers
- WCAG failures
- Broken functionality

#### Improvements

- Enhanced user experience
- Better semantics
- Performance optimizations
- Progressive enhancement

#### Best Practices

- Modern patterns
- Future-proofing
- Maintenance considerations
- Testing strategies

### 5. Implementation Guide

Provide specific code changes to:

- Fix accessibility issues
- Improve user experience
- Add proper ARIA attributes
- Enhance keyboard support

## Process

- list all source files in the project and create a markdown scratchpad file, this is your todo list
- create a CODE_REVIEW.md markdown file, this is your code review output
- for each file in the todo list
  - perform the Accessibility Audit
  - summarize your findings
  - write your findings to the code review output

{% render "review_format" %}
