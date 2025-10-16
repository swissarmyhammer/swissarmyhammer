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


## Analysis of Current Implementation

### Code Status
The code in `swissarmyhammer-cli/src/dynamic_cli.rs` already contains the grouping logic:

**Lines 314-400:**
- Line 314-372: Static commands added first (under default heading)
- Line 377: `cli = cli.next_help_heading(Some("Workflows"));` - Sets "Workflows" heading
- Line 388: `cli = cli.next_help_heading(Some("Tools"));` - Sets "Tools" heading

### Problem Identified
Despite the code having `next_help_heading()` calls, the help output does NOT show section headings. All commands appear in a flat list under "Commands:".

**Current Output:**
```
Commands:
  serve            Run as MCP server (default when invoked via stdio)
  doctor           Diagnose configuration and setup issues
  prompt           Manage and test prompts
  ...
  _implement       Autonomously run until all issues are resolved (shortcut for 'flow implement')
  ...
  file             FILE management commands (MCP Tool)
  ...
```

**Expected Output:**
```
Commands:
  serve            Run as MCP server
  doctor           Diagnose configuration and setup issues
  ...

Workflows:
  _implement       Autonomously run until all issues are resolved
  _plan            Turn specifications into multiple step plans
  ...

Tools:
  file             FILE management commands
  issue            ISSUE management commands
  ...
```

### Root Cause Investigation

The issue is that `next_help_heading()` needs to be called BEFORE adding static commands, not after. Looking at the Clap documentation and the code flow:

1. Lines 266-312: CLI base created with global args
2. Lines 314-372: Static commands added (serve, doctor, etc.) 
3. Line 377: `next_help_heading("Workflows")` called
4. Line 382: Workflow shortcuts added
5. Line 388: `next_help_heading("Tools")` called  
6. Line 397: Tool commands added

The problem is that when no heading is set initially, Clap uses "Commands" as the default heading for ALL subcommands added before the first `next_help_heading()` call. Once you call `next_help_heading()`, it only affects commands added AFTER that call.

### Solution

We need to explicitly set the heading for static commands. There are two approaches:

**Option A: Set "Commands" heading explicitly**
```rust
// Line ~314: Before adding static commands
cli = cli.next_help_heading(Some("Commands"));

// Add static commands
cli = cli.subcommand(serve_command);
cli = Self::add_static_commands(cli);

// Line ~377: Set Workflows heading
cli = cli.next_help_heading(Some("Workflows"));
// Add workflow shortcuts

// Line ~388: Set Tools heading  
cli = cli.next_help_heading(Some("Tools"));
// Add tool commands
```

**Option B: Clear heading for static commands**
Since "Commands" is Clap's default, we could also use `None` to keep the default behavior:
```rust
// Line ~314: Explicitly use default heading
cli = cli.next_help_heading(None);  // or just omit this line

// Add static commands...

// Line ~377: Set Workflows heading
cli = cli.next_help_heading(Some("Workflows"));
```

**Recommendation: Option A** - Explicitly set all headings for clarity and maintainability.




## Critical Discovery: `next_help_heading()` Does Not Work for Subcommands

### The Real Problem

After deep research into Clap's GitHub issues (#5828 and #1553), I discovered that **`next_help_heading()` only works for ARGUMENTS, not SUBCOMMANDS**. This is a known limitation in Clap v4.x.

From the Clap maintainers:
> "The usage text in clap has one section for subcommands, and therefore one heading. It shows all the available subcommands."

This explains why our code at lines 377 and 388 has no effect - Clap simply ignores `next_help_heading()` calls when adding subcommands.

### Current Code Analysis

**Lines 377 & 388:**
```rust
cli = cli.next_help_heading(Some("Workflows"));  // ❌ IGNORED for subcommands
cli = cli.next_help_heading(Some("Tools"));      // ❌ IGNORED for subcommands
```

### Alternative Solutions

Since Clap doesn't support multiple subcommand headings, we have several options:

#### Option 1: Custom Help Template (Most Flexible)
Override Clap's help template to manually organize subcommands into sections. This requires:
- Collecting subcommands into categories
- Writing a custom help formatter
- More maintenance overhead

**Pros:** Full control over help output
**Cons:** Breaks with Clap updates, more code to maintain

#### Option 2: Visual Separators in Command Descriptions (Simplest)
Add visual markers to command descriptions to create pseudo-sections:

```rust
// For first workflow command:
.about("─── WORKFLOWS ─── | Autonomously run until all issues are resolved")

// For first tool command:
.about("─── TOOLS ─── | FILE management commands (MCP Tool)")
```

**Pros:** Works with current Clap, minimal code changes
**Cons:** Not as clean as real sections, relies on description formatting

#### Option 3: Single Heading per Command Category (Nested Structure)
Instead of flat subcommands, nest them:

```rust
// Instead of: sah implement, sah file, etc.
// Use: sah workflows implement, sah tools file
```

**Pros:** Clean organization, native Clap support
**Cons:** **BREAKS USER EXPERIENCE** - changes all command names

#### Option 4: Document Limitation & Use Naming Conventions
Accept Clap's limitation and use clear naming conventions:
- Prefix workflow shortcuts: `flow-implement`, `flow-plan`
- Prefix tool commands: `tool-file`, `tool-issue`

**Pros:** Works with Clap, backwards compatible
**Cons:** Uglier command names, doesn't solve the visual grouping issue

### Recommended Solution: Option 2 (Visual Separators)

Given the constraint that this is a known Clap limitation scheduled for v5.0 (not yet released), I recommend **Option 2** as the pragmatic solution:

1. Add visual separator to the FIRST workflow command's description
2. Add visual separator to the FIRST tool command's description
3. Document this limitation in code comments

This provides visual organization without breaking functionality or requiring major refactoring.




## Implementation: Visual Separators Solution

Since Clap doesn't support multiple subcommand headings, I'm implementing visual separators in command descriptions to create pseudo-sections.

### Changes Required

1. **Remove ineffective `next_help_heading()` calls** (lines 377, 388)
2. **Add visual separator to first workflow command**
3. **Add visual separator to first tool command**
4. **Add documentation comments explaining the limitation**

### Implementation Details

The first workflow command in alphabetical order will get a separator like:
```
──────── WORKFLOWS ──────── | <original description>
```

The first tool command in alphabetical order will get a separator like:
```
──────── TOOLS ──────── | <original description>
```

This creates visual grouping in the help output without requiring Clap features that don't exist yet.




## Implementation Complete

Successfully implemented visual separators to group CLI commands, working around Clap v4.x's limitation that `next_help_heading()` doesn't work with subcommands.

### Changes Made

**File:** `swissarmyhammer-cli/src/dynamic_cli.rs`

1. **Lines 374-395:** Modified workflow shortcuts section
   - Removed ineffective `cli.next_help_heading(Some("Workflows"))` call
   - Added documentation comment explaining Clap v4.x limitation with link to GitHub issue
   - Added visual separator `──────── WORKFLOWS ────────` to first workflow command's description

2. **Lines 397-420:** Modified MCP tools section
   - Removed ineffective `cli.next_help_heading(Some("Tools"))` call
   - Added documentation comment explaining the limitation
   - Added visual separator `──────── TOOLS ────────` to first tool command's description
   - Used enumeration to identify first command for separator injection

### Result

Help output now displays:
```
Commands:
  serve            Run as MCP server (default when invoked via stdio)
  doctor           Diagnose configuration and setup issues
  prompt           Manage and test prompts
  flow             Execute and manage workflows
  validate         Validate prompt files and workflows for syntax and best practices
  plan             Plan a specific specification file
  implement        Execute the implement workflow for autonomous issue resolution
  agent            Manage and interact with agents
  rule             Manage and test code quality rules
  _implement       ──────── WORKFLOWS ──────── | Autonomously run until all issues are resolved (shortcut for 'flow implement')
  _plan            Turn specifications into multiple step plans (shortcut for 'flow plan')
  ...
  file             ──────── TOOLS ──────── | FILE management commands (MCP Tool)
  issue            ISSUE management commands (MCP Tool)
  ...
```

The visual separators clearly delineate:
1. Static commands (serve, doctor, prompt, etc.)
2. Workflow shortcuts (starting with `──────── WORKFLOWS ────────`)
3. MCP tool commands (starting with `──────── TOOLS ────────`)

### Technical Notes

- Visual separators use Unicode box-drawing characters (`─`) for clear visual distinction
- Separators are injected at command description generation time
- Solution is maintainable and doesn't rely on unstable Clap features
- Code includes links to Clap GitHub issues for future reference
- This approach will work until Clap v5.0 provides native multiple heading support

