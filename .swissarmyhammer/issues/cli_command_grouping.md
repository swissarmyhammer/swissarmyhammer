# Organize CLI Commands into Visual Groups in Help Text

## Objective

Organize the CLI help output to group commands into logical categories: static commands, workflows, and tools.

## Context

Currently, `sah --help` shows all commands in a flat list. As we add dynamic workflow shortcuts and tool commands, the help text becomes cluttered and hard to navigate.

**Desired Grouping**:
1. **Commands** (no heading) - Static built-in commands:
   - `serve`, `doctor`, `validate`, `agent`, `prompt`, `rule`
   
2. **Workflows** - Dynamic workflow shortcuts:
   - `implement`, `plan`, `code-review`, etc.
   - Generated from available workflows
   - May have `_` prefix for conflicts
   
3. **Tools** - MCP tool commands:
   - `flow`, `issue`, `memo`, `search`, etc.
   - Generated from MCP tool registry

## Research: Clap Grouping Options

Based on web search and Clap documentation:

### Option 1: `next_help_heading()`

Clap provides `Command::next_help_heading()` to set a heading for subsequent subcommands:

```rust
let cli = Command::new("sah")
    .about("SwissArmyHammer")
    // Static commands (no heading)
    .subcommand(Command::new("serve").about("Run MCP server"))
    .subcommand(Command::new("doctor").about("Diagnose setup"))
    
    // Set heading for workflows
    .next_help_heading("Workflows")
    .subcommand(Command::new("implement").about("..."))
    .subcommand(Command::new("plan").about("..."))
    
    // Set heading for tools
    .next_help_heading("Tools")
    .subcommand(Command::new("flow").about("..."))
    .subcommand(Command::new("issue").about("..."));
```

**Pros**: Simple, native Clap feature
**Cons**: All subcommands after heading are grouped until next heading

### Option 2: Custom Help Template

Override the help template to organize subcommands:

```rust
Command::new("sah")
    .help_template(
        "{about}\n\n\
         {usage-heading} {usage}\n\n\
         COMMANDS:\n\
         {static-commands}\n\n\
         WORKFLOWS:\n\
         {workflow-commands}\n\n\
         TOOLS:\n\
         {tool-commands}\n\n\
         {options}"
    )
```

**Pros**: Full control over help layout
**Cons**: Requires custom rendering logic, may break with Clap updates

### Option 3: Nested Subcommands

Create category subcommands:

```rust
Command::new("sah")
    .subcommand(Command::new("serve"))
    .subcommand(Command::new("doctor"))
    .subcommand(
        Command::new("workflow")
            .subcommand(Command::new("implement"))
            .subcommand(Command::new("plan"))
    )
    .subcommand(
        Command::new("tool")
            .subcommand(Command::new("flow"))
            .subcommand(Command::new("issue"))
    )
```

**Pros**: Clear organization, standard Clap pattern
**Cons**: Changes CLI syntax (breaks `sah implement` → `sah workflow implement`)

## Recommended Solution

**Use `next_help_heading()`** with careful ordering:

```rust
fn build_cli() -> Command {
    let mut cli = Command::new("sah")
        .version(VERSION)
        .about("SwissArmyHammer - The only coding assistant you'll ever need");
    
    // Static commands (no heading - default "Commands" used by Clap)
    cli = cli
        .subcommand(build_serve_command())
        .subcommand(build_doctor_command())
        .subcommand(build_validate_command())
        .subcommand(build_agent_command())
        .subcommand(build_prompt_command())
        .subcommand(build_rule_command());
    
    // Workflow shortcuts
    cli = cli.next_help_heading("Workflows");
    let workflow_shortcuts = generate_workflow_shortcuts(&workflow_storage);
    for shortcut in workflow_shortcuts {
        cli = cli.subcommand(shortcut);
    }
    
    // Tool commands (dynamic from MCP)
    cli = cli.next_help_heading("Tools");
    let tool_commands = generate_tool_commands(&tool_registry);
    for tool_cmd in tool_commands {
        cli = cli.subcommand(tool_cmd);
    }
    
    cli
}
```

## Proposed Solution

### Implementation Plan

After analyzing the codebase in `swissarmyhammer-cli/src/dynamic_cli.rs`, I propose the following changes:

1. **Modify `build_cli()` method** (lines 265-388):
   - Keep static commands under default heading
   - Add `next_help_heading("Workflows")` before workflow shortcuts
   - Add `next_help_heading("Tools")` before MCP tool commands

2. **Sort commands within each group** alphabetically for easier scanning

3. **Update `add_static_commands()` to return the CLI** without side effects

### Code Changes

**File: `swissarmyhammer-cli/src/dynamic_cli.rs`**

Modify the `build_cli()` method around line 265:

```rust
pub fn build_cli(&self, workflow_storage: Option<&WorkflowStorage>) -> Command {
    let mut cli = Command::new("swissarmyhammer")
        .version(env!("CARGO_PKG_VERSION"))
        .about("An MCP server for managing prompts as markdown files")
        .long_about(
            "
swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts as markdown files. It supports file watching, template substitution,
and seamless integration with Claude Code.

This CLI dynamically generates all MCP tool commands, eliminating code duplication
and ensuring perfect consistency between MCP and CLI interfaces.
",
        )
        // Add global flags
        .arg(/* verbose flag */)
        .arg(/* debug flag */)
        .arg(/* quiet flag */)
        .arg(/* validate-tools flag */)
        .arg(/* format flag */);

    // === STATIC COMMANDS (Default heading) ===
    // Add core serve command first
    cli = cli.subcommand(/* serve command */);
    
    // Add other static commands
    cli = Self::add_static_commands(cli);

    // === WORKFLOWS SECTION ===
    if let Some(storage) = workflow_storage {
        cli = cli.next_help_heading("Workflows");
        let mut shortcuts = Self::build_workflow_shortcuts(storage);
        // Sort alphabetically for easier scanning
        shortcuts.sort_by(|a, b| a.get_name().cmp(b.get_name()));
        for shortcut in shortcuts {
            cli = cli.subcommand(shortcut);
        }
    }

    // === TOOLS SECTION ===
    cli = cli.next_help_heading("Tools");
    
    // Get sorted category names for consistent ordering
    let mut category_names: Vec<String> = self.category_commands.keys().cloned().collect();
    category_names.sort();
    
    for category_name in category_names {
        if let Some(category_data) = self.category_commands.get(&category_name) {
            cli = cli.subcommand(self.build_category_command_from_data(&category_name, category_data));
        }
    }

    cli
}
```

### Testing Strategy

Create tests in a new file or extend existing tests:

```rust
#[test]
fn test_help_output_has_workflow_heading() {
    let cli = build_test_cli_with_workflows();
    let help = get_help_text(&cli);
    assert!(help.contains("Workflows"));
}

#[test]
fn test_help_output_has_tools_heading() {
    let cli = build_test_cli();
    let help = get_help_text(&cli);
    assert!(help.contains("Tools"));
}

#[test]
fn test_static_commands_appear_before_workflows() {
    let cli = build_test_cli_with_workflows();
    let help = get_help_text(&cli);
    let serve_pos = help.find("serve").unwrap();
    let workflows_pos = help.find("Workflows").unwrap();
    assert!(serve_pos < workflows_pos);
}

#[test]
fn test_workflows_appear_before_tools() {
    let cli = build_test_cli_with_workflows();
    let help = get_help_text(&cli);
    let workflows_pos = help.find("Workflows").unwrap();
    let tools_pos = help.find("Tools").unwrap();
    assert!(workflows_pos < tools_pos);
}
```

## Files to Modify

- `swissarmyhammer-cli/src/dynamic_cli.rs` - Main implementation
- `swissarmyhammer-cli/src/dynamic_cli_tests.rs` - Add new tests (or create if doesn't exist)

## Acceptance Criteria

- [ ] Help output shows three logical sections
- [ ] Static commands appear first (under default heading)
- [ ] Workflows appear under "Workflows" heading
- [ ] Tools appear under "Tools" heading
- [ ] Commands sorted alphabetically within each group
- [ ] `sah --help` is clear and easy to navigate
- [ ] All tests pass
- [ ] Code compiles without warnings

## Implementation Notes

1. **Minimal Changes**: This solution requires minimal code changes - just adding `next_help_heading()` calls and sorting
2. **Backward Compatible**: No changes to command names or behavior, only help text organization
3. **Native Clap**: Uses built-in Clap feature, no custom rendering needed
4. **Maintainable**: Simple and clear, easy to modify in the future

## Benefits

1. **Clarity**: Users can quickly find relevant commands
2. **Discoverability**: Clear distinction between static vs dynamic commands
3. **Scalability**: As workflows grow, they're grouped separately
4. **Maintainability**: Native Clap feature, no custom rendering

## Estimated Changes

~50 lines of code (primarily in `build_cli()` method)