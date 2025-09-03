# Creating Prompts

Prompts are markdown files with YAML front matter that define reusable AI interactions. They support templating, parameters, and can be tested independently.

## Basic Structure

Every prompt file requires:

1. **YAML front matter** with metadata
2. **Markdown content** with optional Liquid templating

```markdown
---
name: prompt-name
title: Human Readable Title
description: Brief description of what this prompt does
parameters:
  - name: param1
    description: Description of parameter
    required: true
    default: optional_default
---

# Prompt Content

Your prompt content here with {{param1}} substitution.
```

## Required Front Matter

### Essential Fields
- `name`: Internal identifier (used in CLI and workflows)
- `title`: Display name for humans
- `description`: Brief explanation of prompt purpose

### Parameter Definition
```yaml
parameters:
  - name: language
    description: Programming language for the code
    required: false
    default: "Python"
  - name: task
    description: What task to help with
    required: true
```

## Templating with Liquid

### Variable Substitution
```liquid
Please help me with {{task}} in {{language}}.
```

### Conditional Logic
```liquid
{% if language == "Rust" %}
Focus on memory safety and performance.
{% else %}
Focus on readability and maintainability.
{% endif %}
```

### Loops
```liquid
Review these files:
{% for file in files %}
- {{file}}
{% endfor %}
```

### Filters
```liquid
Project: {{project_name | default: "Unknown Project"}}
Language: {{language | upcase}}
```

## Testing Prompts

### Command Line Testing
```bash
# Basic test
sah prompt test my-prompt

# With parameters
sah prompt test my-prompt --task "code review" --language "Rust"

# List all prompts
sah prompt list

# Validate syntax
sah validate prompts/
```

### Validation
SwissArmyHammer validates:
- YAML front matter syntax
- Required parameter usage
- Liquid template syntax
- File naming conventions

## File Organization

### Flat Structure
```
prompts/
├── code-review.md
├── commit-message.md
└── documentation.md
```

### Nested Structure
```
prompts/
├── review/
│   ├── security.md
│   ├── performance.md
│   └── accessibility.md
├── docs/
│   ├── readme.md
│   └── api.md
└── debug/
    ├── error.md
    └── logs.md
```

## Best Practices

- **Use descriptive names** - Clear, kebab-case filenames
- **Document parameters** - Explain what each parameter does
- **Set sensible defaults** - Reduce required parameters
- **Test thoroughly** - Validate with different parameter combinations
- **Keep focused** - One clear purpose per prompt
- **Use templates** - Leverage Liquid for dynamic content