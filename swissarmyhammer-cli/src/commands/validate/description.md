Catch configuration errors before they cause failures with comprehensive validation.

The validate command ensures quality and correctness across your entire
SwissArmyHammer configuration, detecting syntax errors, structural issues,
and best practice violations before they impact your workflows.

## Quality Assurance

Comprehensive Validation:
• Prompt files from all sources (builtin, user, project)
• Workflow definitions from standard locations
• MCP tool schemas and CLI integration (with --validate-tools)
• Template syntax and variable usage
• YAML frontmatter structure
• Required field presence and format
• Best practice compliance

Early Error Detection:
• Find syntax errors before execution
• Identify missing required fields
• Detect template variable mismatches
• Validate workflow state machine structure
• Check MCP tool schema correctness
• Verify CLI integration compatibility

CI/CD Integration:
• Automated quality checks in build pipelines
• Exit codes indicate validation results
• Quiet mode for clean CI output
• JSON output for tool integration
• Fast execution for rapid feedback

## What Gets Validated

Prompt Files:
• YAML frontmatter syntax correctness
• Required fields: title, description
• Template variable declarations match usage
• Liquid template syntax validity
• Parameter definitions and types
• Default value correctness
• Partial marker handling

Workflow Files:
• State machine structure integrity
• State connectivity and transitions
• Action and tool references
• Variable declarations and usage
• Conditional logic syntax
• Loop and iteration constructs
• Error handling configuration

MCP Tools (with --validate-tools):
• JSON schema correctness
• Parameter type definitions
• Required vs optional field specifications
• Tool description completeness
• CLI integration requirements
• Documentation quality
• Best practice adherence

## Validation Modes

Standard validation (prompts and workflows):
```bash
sah validate
```

Comprehensive validation (including MCP tools):
```bash
sah validate --validate-tools
```

CI/CD mode (errors only, no warnings):
```bash
sah validate --quiet
sah validate --validate-tools --quiet
```

Machine-readable output:
```bash
sah validate --format json
sah validate --validate-tools --format json
```

## Exit Codes

- `0` - All validation passed, no errors or warnings
- `1` - Warnings found but no errors
- `2` - Errors found that require fixes

Use exit codes in scripts and CI pipelines:
```bash
sah validate || exit 1
```

## Discovery and Sources

Prompts validated from:
• Built-in prompts (embedded in binary)
• User prompts (~/.swissarmyhammer/prompts/)
• Project prompts (./.swissarmyhammer/prompts/)

Workflows validated from:
• Built-in workflows (embedded in binary)
• User workflows (~/.swissarmyhammer/workflows/)
• Project workflows (./workflows/)

MCP tools validated from:
• SwissArmyHammer tool definitions
• CLI command integration points
• Tool parameter schemas

## Common Use Cases

Pre-commit validation:
```bash
sah validate --quiet && git commit
```

CI pipeline check:
```bash
sah validate --validate-tools --format json > validation-report.json
```

Development workflow validation:
```bash
sah validate --verbose
```

Quality gate in deployment:
```bash
sah validate --validate-tools --quiet || exit 1
```

## Validation Checks

YAML Frontmatter:
• Syntax correctness
• Required fields present
• Field types match expectations
• Valid enum values

Template Syntax:
• Liquid template parsing
• Variable references exist
• Filter syntax correctness
• Control flow validity
• Partial references resolve

Workflow Structure:
• All states are reachable
• Transitions are valid
• Actions reference existing tools
• Variables are declared before use
• Error handlers are properly configured

MCP Tool Schemas:
• JSON schema validity
• Parameter type correctness
• Required field specification
• Tool description quality
• CLI integration completeness

Best Practices:
• Descriptive titles and descriptions
• Proper parameter documentation
• Sensible default values
• Clear error messages
• Consistent naming conventions

## Examples

Basic validation:
```bash
sah validate
```

Full system validation:
```bash
sah validate --validate-tools
```

Quiet mode for CI:
```bash
sah validate --quiet
```

Detailed output:
```bash
sah --verbose validate
```

JSON output for tooling:
```bash
sah validate --format json | jq '.errors'
```

Validate after changes:
```bash
sah validate --validate-tools --verbose
```

## Output Formats

Table format (default):
• Human-readable tabular output
• Color-coded error/warning levels
• File paths and line numbers
• Clear error descriptions

JSON format:
• Machine-parseable structured output
• Complete error and warning details
• Suitable for CI integration
• Easy tool consumption

YAML format:
• Human-readable structured output
• Hierarchical error organization
• Good for documentation
• Easy diff comparison

## Troubleshooting

Validation errors in prompts:
• Check YAML frontmatter syntax
• Verify all required fields present
• Ensure template variables declared
• Test Liquid template syntax

Validation errors in workflows:
• Verify state machine structure
• Check all state transitions
• Ensure action references valid
• Validate variable declarations

Validation errors in tools:
• Review JSON schema correctness
• Check parameter type definitions
• Verify required fields specified
• Ensure documentation complete

## Integration with Development Workflow

Pre-commit hook:
```bash
#!/bin/bash
sah validate --quiet || {
  echo "Validation failed. Fix errors before committing."
  exit 1
}
```

Git hook (.git/hooks/pre-commit):
```bash
#!/bin/bash
sah validate --validate-tools --quiet
```

Make target:
```makefile
validate:
	sah validate --validate-tools --quiet

.PHONY: validate
```

CI pipeline (GitHub Actions):
```yaml
- name: Validate Configuration
  run: sah validate --validate-tools --format json
```

## Benefits

Catch Errors Early:
• Find problems before runtime
• Prevent workflow failures
• Avoid wasted execution time
• Reduce debugging effort

Ensure Quality:
• Enforce best practices
• Maintain consistent standards
• Improve documentation quality
• Promote good patterns

Enable Confidence:
• Deploy with certainty
• Refactor safely
• Share configuration reliably
• Integrate automatically

Support Automation:
• CI/CD quality gates
• Automated testing
• Pre-commit validation
• Continuous quality monitoring

The validate command is your quality assurance system for SwissArmyHammer
configuration, ensuring that prompts, workflows, and tools are correct,
complete, and ready for reliable operation.