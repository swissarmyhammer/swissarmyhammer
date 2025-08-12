# Template System

SwissArmyHammer uses the Liquid template engine to create dynamic, data-driven prompts and workflows.

## Overview

The template system provides:
- **Variable Substitution**: Replace placeholders with dynamic values
- **Conditional Logic**: Show/hide content based on conditions  
- **Loops and Iteration**: Process lists and collections
- **Filters**: Transform and format data
- **Environment Integration**: Access environment variables and system info
- **Custom Extensions**: Add domain-specific functionality

## Basic Syntax

### Variables

Variables are enclosed in double curly braces:

```liquid
Hello {{name}}!
Welcome to {{project_name}}.
```

With arguments:
- `name: "Alice"`  
- `project_name: "SwissArmyHammer"`

Renders as:
```
Hello Alice!
Welcome to SwissArmyHammer.
```

### Comments

```liquid
{% comment %}
This is a comment that won't appear in output
{% endcomment %}

{# This is also a comment #}
```

## Control Structures

### Conditionals

#### If/Else

```liquid
{% if user.premium %}
  Welcome to Premium features!
{% else %}
  Consider upgrading to Premium.
{% endif %}
```

#### Multiple Conditions

```liquid
{% if language == "rust" %}
  Use `cargo build` to compile.
{% elsif language == "python" %}
  Use `python script.py` to run.
{% elsif language == "javascript" %}
  Use `node script.js` to run.
{% else %}
  Check your language documentation.
{% endif %}
```

#### Complex Conditions

```liquid
{% if user.premium and feature.enabled %}
  Premium feature available!
{% endif %}

{% if environment == "prod" or environment == "staging" %}
  ⚠️ Production environment detected!
{% endif %}
```

### Loops

#### For Loops

```liquid
## Requirements
{% for req in requirements %}
- {{req}}
{% endfor %}
```

#### Loop Variables

```liquid
{% for item in items %}
{{forloop.index}}. {{item.name}} 
   {% if forloop.first %}(First item){% endif %}
   {% if forloop.last %}(Last item){% endif %}
{% endfor %}
```

Available loop variables:
- `forloop.index` - Current iteration (1-based)
- `forloop.index0` - Current iteration (0-based)  
- `forloop.rindex` - Remaining iterations
- `forloop.rindex0` - Remaining iterations (0-based)
- `forloop.first` - True if first iteration
- `forloop.last` - True if last iteration
- `forloop.length` - Total loop length

#### Filtering in Loops

```liquid
{% for user in users %}
  {% if user.active %}
  - {{user.name}} ({{user.role}})
  {% endif %}
{% endfor %}
```

#### Limiting Loops

```liquid
{% for item in items limit:5 %}
- {{item}}
{% endfor %}

{% for item in items offset:2 limit:3 %}
- {{item}}
{% endfor %}
```

### Case Statements

```liquid
{% case language %}
  {% when "rust" %}
    Rust detected - using Cargo
  {% when "python" %}
    Python detected - using pip
  {% when "javascript", "typescript" %}
    Node.js detected - using npm
  {% else %}
    Unknown language
{% endcase %}
```

## Filters

Filters transform values using the pipe (`|`) operator:

```liquid
{{name | upcase}}
{{description | truncate: 50}}
{{items | size}}
```

### String Filters

| Filter | Description | Example |
|--------|-------------|---------|
| `capitalize` | Capitalize first letter | `{{text | capitalize}}` |
| `downcase` | Convert to lowercase | `{{text | downcase}}` |
| `upcase` | Convert to uppercase | `{{text | upcase}}` |
| `strip` | Remove whitespace | `{{text | strip}}` |
| `lstrip` | Remove left whitespace | `{{text | lstrip}}` |
| `rstrip` | Remove right whitespace | `{{text | rstrip}}` |
| `truncate` | Limit length | `{{text | truncate: 50}}` |
| `truncatewords` | Limit words | `{{text | truncatewords: 10}}` |
| `prepend` | Add prefix | `{{text | prepend: ">> "}}` |
| `append` | Add suffix | `{{text | append: " <<"}}` |
| `replace` | Replace text | `{{text | replace: "old", "new"}}` |
| `remove` | Remove text | `{{text | remove: "unwanted"}}` |
| `split` | Split into array | `{{text | split: ","}}` |
| `slice` | Extract substring | `{{text | slice: 0, 10}}` |

### Array Filters

| Filter | Description | Example |
|--------|-------------|---------|
| `join` | Join elements | `{{array | join: ", "}}` |
| `first` | Get first element | `{{array | first}}` |
| `last` | Get last element | `{{array | last}}` |
| `size` | Get length | `{{array | size}}` |
| `sort` | Sort elements | `{{array | sort}}` |
| `reverse` | Reverse order | `{{array | reverse}}` |
| `uniq` | Remove duplicates | `{{array | uniq}}` |
| `compact` | Remove nil values | `{{array | compact}}` |
| `map` | Extract property | `{{users | map: "name"}}` |
| `where` | Filter by property | `{{users | where: "active", true}}` |

### Numeric Filters

| Filter | Description | Example |
|--------|-------------|---------|
| `plus` | Addition | `{{num | plus: 5}}` |
| `minus` | Subtraction | `{{num | minus: 3}}` |
| `times` | Multiplication | `{{num | times: 2}}` |
| `divided_by` | Division | `{{num | divided_by: 4}}` |
| `modulo` | Remainder | `{{num | modulo: 3}}` |
| `round` | Round number | `{{num | round}}` |
| `ceil` | Round up | `{{num | ceil}}` |
| `floor` | Round down | `{{num | floor}}` |
| `abs` | Absolute value | `{{num | abs}}` |

### Date Filters

```liquid
{{date | date: "%Y-%m-%d"}}
{{date | date: "%B %d, %Y"}}
{{now | date: "%H:%M:%S"}}
```

Format strings:
- `%Y` - 4-digit year
- `%m` - Month (01-12)
- `%d` - Day (01-31)
- `%H` - Hour (00-23)
- `%M` - Minute (00-59)
- `%S` - Second (00-59)
- `%B` - Full month name
- `%A` - Full weekday name

### Utility Filters

| Filter | Description | Example |
|--------|-------------|---------|
| `default` | Default value | `{{value | default: "none"}}` |
| `escape` | HTML escape | `{{html | escape}}` |
| `escape_once` | Escape unescaped | `{{html | escape_once}}` |
| `url_encode` | URL encoding | `{{text | url_encode}}` |
| `strip_html` | Remove HTML tags | `{{html | strip_html}}` |
| `newline_to_br` | Convert \n to <br> | `{{text | newline_to_br}}` |

## Custom Filters

SwissArmyHammer includes programming-specific filters:

### Case Conversion Filters

```liquid
{{variable_name | snake_case}}     <!-- becomes snake_case -->
{{variable_name | kebab_case}}     <!-- becomes kebab-case -->
{{variable_name | pascal_case}}    <!-- becomes PascalCase -->
{{variable_name | camel_case}}     <!-- becomes camelCase -->
```

### Code Formatting Filters

```liquid
{{code | code_block: "rust"}}      <!-- Wraps in ```rust block -->
{{text | markdown_escape}}         <!-- Escapes markdown chars -->
{{number | pluralize: "item"}}     <!-- "1 item" or "2 items" -->
```

### Path Filters

```liquid
{{file_path | dirname}}            <!-- Get directory -->
{{file_path | basename}}           <!-- Get filename -->
{{file_path | extname}}            <!-- Get extension -->
```

## Advanced Features

### Variable Assignment

```liquid
{% assign full_name = first_name | append: " " | append: last_name %}
{% assign item_count = items | size %}
{% assign formatted_date = now | date: "%Y-%m-%d" %}

Hello {{full_name}}!
You have {{item_count}} items.
Today is {{formatted_date}}.
```

### Capture Blocks

```liquid
{% capture error_message %}
Error in {{file_name}} at line {{line_number}}: {{error_description}}
{% endcapture %}

{% if show_errors %}
**Error**: {{error_message}}
{% endif %}
```

### Include Templates

Create reusable template fragments:

**File**: `templates/header.liquid`
```liquid
# {{title | default: "Untitled"}}
**Generated**: {{now | date: "%Y-%m-%d %H:%M"}}
---
```

**Usage**:
```liquid
{% include "header" %}

Main content goes here...
```

### Template Inheritance

**Base template** (`base.liquid`):
```liquid
# {{title}}

{% block content %}
Default content
{% endblock %}

---
Generated by SwissArmyHammer
```

**Child template**:
```liquid
{% extends "base" %}

{% block content %}
Custom content for this template
{% endblock %}
```

## Environment Integration

### Environment Variables

```liquid
Project: {{PROJECT_NAME | default: "Unknown"}}
Environment: {{NODE_ENV | default: "development"}}  
User: {{USER}}
Home: {{HOME}}
Current Directory: {{PWD}}
```

### System Information

```liquid
OS: {{OSTYPE | default: "unknown"}}
Shell: {{SHELL | default: "unknown"}}
Path: {{PATH | truncate: 100}}
```

### Git Information

```liquid
Branch: {{GIT_BRANCH | default: "main"}}
Commit: {{GIT_COMMIT | slice: 0, 8}}
Repository: {{GIT_REMOTE_URL | replace: ".git", ""}}
```

## Context Variables

SwissArmyHammer provides built-in context variables:

### File Context

```liquid
Current file: {{file.path}}
File size: {{file.size}} bytes
Modified: {{file.modified | date: "%Y-%m-%d"}}
Extension: {{file.extension}}
```

### Workflow Context

```liquid
Workflow: {{workflow.name}}
State: {{workflow.current_state}}
Started: {{workflow.start_time | date: "%H:%M:%S"}}
Elapsed: {{workflow.elapsed_ms}}ms
```

### Issue Context

```liquid
Issue: {{issue.name}}
Status: {{issue.status}}
Branch: {{issue.branch}}
Created: {{issue.created | date: "%B %d, %Y"}}
```

## Error Handling

### Graceful Degradation

```liquid
{% if user.name %}
Hello {{user.name}}!
{% else %}
Hello there!
{% endif %}

Files: {{files | size | default: 0}}
```

### Debugging Templates

```liquid
{% comment %}Debug: {{variable | inspect}}{% endcomment %}

{% if debug_mode %}
**Debug Info**:
- Variable: {{variable}}  
- Type: {{variable | type}}
- Size: {{variable | size}}
{% endif %}
```

## Performance Considerations

### Efficient Loops

```liquid
{% comment %}Good: Filter before looping{% endcomment %}
{% assign active_users = users | where: "active", true %}
{% for user in active_users %}
- {{user.name}}
{% endfor %}

{% comment %}Avoid: Filtering inside loop{% endcomment %}
{% for user in users %}
  {% if user.active %}
  - {{user.name}}
  {% endif %}
{% endfor %}
```

### Variable Reuse

```liquid
{% assign processed_data = raw_data | sort | uniq | slice: 0, 10 %}

Count: {{processed_data | size}}
Items: {{processed_data | join: ", "}}
```

### Conditional Computation

```liquid
{% if expensive_operation_needed %}
  {% assign result = data | expensive_filter %}
  Result: {{result}}
{% endif %}
```

## Best Practices

### Template Organization

1. **Keep templates focused** - One purpose per template
2. **Use meaningful names** - Clear, descriptive template names
3. **Document complex logic** - Use comments for complex conditionals
4. **Validate inputs** - Check for required variables
5. **Provide defaults** - Use `default` filter for optional values

### Code Style

```liquid
{% comment %}Good: Clean, readable formatting{% endcomment %}
{% if environment == "production" %}
  {% assign base_url = "https://api.example.com" %}
{% else %}
  {% assign base_url = "http://localhost:3000" %}
{% endif %}

API Endpoint: {{base_url}}/{{endpoint | default: "status"}}

{% comment %}Avoid: Cramped, hard to read{% endcomment %}
API: {% if environment=="production"%}https://api.example.com{%else%}http://localhost:3000{%endif%}/{{endpoint|default:"status"}}
```

### Security

```liquid
{% comment %}Always escape user input{% endcomment %}
User: {{user_input | escape}}

{% comment %}Validate before use{% endcomment %}
{% if branch_name and branch_name != "" %}
Branch: {{branch_name | escape}}
{% endif %}

{% comment %}Use safe defaults{% endcomment %}
Environment: {{environment | default: "development" | escape}}
```

This template system provides powerful capabilities for creating dynamic, context-aware prompts and workflows while maintaining readability and maintainability.