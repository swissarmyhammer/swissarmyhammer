# Prompt System - Reusable AI Templates with Power and Flexibility

Create, manage, and test reusable AI prompt templates that bring consistency
and efficiency to your AI-powered workflows. The prompt system enables you to
define structured templates once and use them anywhere with different data.

## Template Power

Reusable Templates:
• Define prompts as Liquid templates with variables
• Use the same prompt with different parameters for each execution
• Share prompts across projects and team members
• Version control your AI interactions
• Build a library of proven, effective prompts

Structured Parameters:
• Define typed parameters with descriptions
• Interactive parameter collection or command-line arguments
• Validation and type checking for reliable execution
• Default values and optional parameters
• Clear documentation of prompt requirements

Template Features:
• Liquid template syntax for logic and formatting
• Conditional content based on parameters
• Loops for processing collections
• Filters for data transformation
• Partial templates for reusable components

## Discovery and Precedence

Prompts are loaded from multiple sources with hierarchical precedence:
• Built-in prompts (lowest) - Standard prompts included with the tool
• User prompts (medium) - ~/.swissarmyhammer/prompts/ for personal templates
• Project prompts (highest) - ./.swissarmyhammer/prompts/ for project-specific needs

Higher precedence prompts override lower ones by name, allowing customization
of built-in prompts or project-specific variants.

## Commands

The prompt system provides two main commands:

### list - Discover Available Prompts

Display all available prompts from all sources:
```bash
sah prompt list                      # Table view of all prompts
sah --verbose prompt list            # Include detailed descriptions
sah --format json prompt list        # Machine-readable output
sah --format yaml prompt list        # YAML format output
```

### test - Interactive Prompt Testing

Test prompts interactively with sample data before using in workflows:

```bash
# Interactive mode - prompts for all parameters
sah prompt test code-review

# Non-interactive - provide parameters directly
sah prompt test help --var topic=git --var format=markdown
sah prompt test code-review --var author=John --var version=1.0

# Debugging and troubleshooting
sah --verbose prompt test plan       # Detailed execution information
sah --debug prompt test help         # Full debug tracing
```

The test command validates your template syntax, parameter usage, and output
before integrating prompts into production workflows.

## Global Arguments

Control output and execution with global flags:

- `--verbose` - Show detailed information including parameter definitions
- `--format <FORMAT>` - Output format: table (default), json, yaml
- `--debug` - Enable comprehensive debug tracing
- `--quiet` - Suppress output except errors

## Why Use Prompts

Consistency:
• Same prompt produces consistent results across uses
• Team members use proven templates, not ad-hoc text
• Version controlled prompt evolution and improvements

Efficiency:
• Write prompt logic once, use many times
• Share effective prompts across projects
• Reduce time crafting one-off prompts

Quality:
• Test and refine prompts before production use
• Document prompt requirements and parameters
• Iterate on prompts based on results

Integration:
• Use prompts in workflows and automation
• Combine prompts for complex operations
• Build prompt libraries for common tasks

## Common Workflows

Discover available prompts:
```bash
sah prompt list
sah --verbose prompt list             # See parameter details
```

Test a prompt interactively:
```bash
sah prompt test code-review           # Interactive parameter entry
```

Test with specific parameters:
```bash
sah prompt test help --var topic=git --var format=markdown
sah prompt test code-review --var author=Jane --var severity=high
```

Export prompt list for documentation:
```bash
sah --format json prompt list > prompts.json
sah --format yaml prompt list > prompts.yaml
```

## Creating Custom Prompts

Create prompts by adding markdown files with YAML frontmatter to:
• ./swissarmyhammer/prompts/ - Project-specific prompts
• ~/.swissarmyhammer/prompts/ - Personal prompt library

Example prompt structure:
```markdown
---
title: Code Review
description: Analyze code for quality and issues
arguments:
  - name: file
    description: File path to review
    type: string
    required: true
  - name: severity
    description: Minimum severity to report
    type: string
    default: warning
---
Review the code in {{ file }} and identify issues at {{ severity }} level or higher.

{% if severity == "error" %}
Focus only on critical issues that must be fixed.
{% else %}
Include style suggestions and improvements.
{% endif %}
```

## Best Practices

Template Design:
• Keep prompts focused on single tasks
• Use clear, descriptive parameter names
• Provide default values when sensible
• Document parameter requirements
• Test with various parameter combinations

Organization:
• Group related prompts in directories
• Use consistent naming conventions
• Version control your prompt library
• Share effective prompts with team

Testing:
• Always test prompts before workflow integration
• Validate with edge cases and unusual inputs
• Use --verbose to understand parameter processing
• Iterate based on output quality

## Examples

List all available prompts:
```bash
sah prompt list
```

See detailed prompt information:
```bash
sah --verbose prompt list
```

Test a prompt interactively:
```bash
sah prompt test code-review
# Prompts for: file, severity, etc.
```

Test with provided parameters:
```bash
sah prompt test help --var topic=docker --var format=tutorial
```

Machine-readable prompt catalog:
```bash
sah --format json prompt list > available-prompts.json
```

Debug prompt rendering:
```bash
sah --debug prompt test custom-prompt --var param=value
```

The prompt system transforms ad-hoc AI interactions into structured, reusable,
version-controlled templates that improve consistency and efficiency across
your entire development workflow.