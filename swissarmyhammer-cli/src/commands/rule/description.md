Manage and check code quality rules in the SwissArmyHammer system.

Rules are code quality and style enforcement patterns stored as markdown files
with YAML frontmatter. They define checks that can be validated for correctness
and run against source code files to identify violations.

RULE DISCOVERY AND PRECEDENCE

Rules are loaded from multiple sources with hierarchical precedence:
• Built-in rules (lowest precedence) - Embedded in the binary
• User rules (medium precedence) - ~/.swissarmyhammer/rules/*.md
• Project rules (highest precedence) - ./rules/*.md in your project

Higher precedence rules override lower ones by name. This allows you to
customize built-in rules or create project-specific checks.

RULE STRUCTURE

Each rule is a markdown file with YAML frontmatter:
```yaml
---
title: Rule Title
description: Rule description
severity: error|warning|info
category: security|style|performance|correctness
tags: [tag1, tag2]
---
Rule template content using Liquid template syntax.
Use {{ file_path }} and {{ file_content }} variables.
```

PARTIALS

Partial templates (marked with `{% partial %}`) are reusable template fragments
that can be included in other rules. They are filtered out from list output
and cannot be checked directly.

COMMANDS

The rule system provides three main commands:

• list - Display all available rules from all sources with metadata
• validate - Check rule files for syntax errors and structural issues
• check - Run rules against source code files to find violations

SUBCOMMAND: list

Lists all available rules with their metadata. Partials are automatically filtered.

Examples:
  sah rule list                      # List all rules in table format
  sah --verbose rule list            # Show detailed information
  sah --format json rule list        # Output as structured JSON
  sah --format yaml rule list        # Output as YAML

SUBCOMMAND: validate

Validates rule files for syntax and structural correctness.

Options:
  --rule NAME    Validate a specific rule by name
  --file PATH    Validate a specific rule file

Examples:
  sah rule validate                  # Validate all rules
  sah rule validate --rule no-unwrap # Validate specific rule
  sah rule validate --file custom.md # Validate specific file

SUBCOMMAND: check

Checks source code files against rules to find violations. Uses AI-powered
analysis to evaluate code against rule templates.

Options:
  <PATTERNS>...          Glob patterns or file paths to check
  -r, --rule NAME        Check only specific rules (can be repeated)
  -s, --severity LEVEL   Filter by severity: error, warning, info
  -c, --category CAT     Filter by category: security, style, performance, correctness

Examples:
  sah rule check "**/*.rs"                    # Check all Rust files
  sah rule check --severity error "src/**/*.rs" # Only error-level rules
  sah rule check --rule no-unwrap "*.rs"      # Check specific rule
  sah rule check --category security "**/*.rs" # Only security rules

CHECK BEHAVIOR

The check command:
1. Loads and validates all rules
2. Applies specified filters (--rule, --severity, --category)
3. Expands glob patterns to target files (respects .gitignore)
4. Creates AI agent for rule evaluation
5. Runs checks with fail-fast behavior on first violation
6. Returns exit code 1 on violations, 0 on success

COMMON WORKFLOWS

1. Explore available rules:
   sah rule list

2. Validate your custom rules:
   sah rule validate

3. Check code quality:
   sah rule check "**/*.rs"

4. Check specific concerns:
   sah rule check --category security --severity error "**/*.rs"

Use global arguments to control output:
  --verbose         Show detailed information and descriptions
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode with comprehensive tracing
  --quiet           Suppress output except errors

AGENT CONFIGURATION

Rule checking uses AI agents configured in .swissarmyhammer/sah.yaml.
The default is Claude Code if no configuration is present.

To configure an agent for your project:
  sah agent use qwen-coder
  sah rule check "**/*.rs"

Or edit .swissarmyhammer/sah.yaml directly:
  agent:
    executor:
      type: llama-agent
      config:
        model:
          source: !HuggingFace
            repo: unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF