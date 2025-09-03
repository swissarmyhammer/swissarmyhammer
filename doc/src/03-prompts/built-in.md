# Built-in Prompts

SwissArmyHammer includes production-ready prompts for common development tasks. These prompts demonstrate best practices and can be used as-is or customized for your needs.

## Example: Say Hello

A simple greeting prompt demonstrating templating and conditional logic:

```markdown
{{#include ../../../builtin/prompts/say-hello.md}}
```

### Key Features Demonstrated

- **Conditional templating** with `{% if %}`/`{% else %}`/`{% endif %}`
- **Variable substitution** with `{{ name }}` and `{{ language }}`
- **Default values** in parameter definitions
- **Filters** using `{{ project_name | default: "Swiss Army Hammer" }}`
- **YAML front matter** with metadata and parameter definitions

### Usage Examples

```bash
# Basic usage
sah prompt test say-hello

# With custom name
sah prompt test say-hello --name "Alice"

# In different language
sah prompt test say-hello --name "Alice" --language "Spanish"
```

## Other Built-in Prompts

- `help` - General assistance prompt
- `plan` - Project planning and task breakdown
- `test` - Generate tests for code
- `commit` - Generate git commit messages
- `docs/readme` - Create README documentation
- `review/code` - Code review assistance
- `debug/error` - Debug error messages
- `issue/code` - Implement code for issues

All built-in prompts are located in the `builtin/prompts/` directory and can be viewed with:

```bash
sah prompt list --category builtin
```