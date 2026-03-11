# Rule Checker Progress Message Formats

## Investigation Summary

This document describes the current progress message formats used in the rule checking system.

## Files Investigated

1. `swissarmyhammer-rules/src/checker.rs` - Core rule checking engine
2. `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs` - MCP tool wrapper

## Current Progress Message Formats

### 1. Individual File Checks (checker.rs:772)

**Location**: `swissarmyhammer-rules/src/checker.rs` line 772

**Format**:
```
"Checking {} against {} [{}/{}] - {} remaining, ETA: {:.1}s"
```

**Example**:
```
Checking src/main.rs against no-unwrap [5/20] - 15 remaining, ETA: 45.3s
```

**Components**:
- `target.display()` - File path being checked
- `rule.name` - Rule name
- `completed_so_far + 1` - Current item number
- `total_items` - Total items to check
- `remaining` - Number of items remaining
- `estimated_remaining_secs` - ETA in seconds (1 decimal place)

### 2. Filtered Rules Check (check/mod.rs:681-682)

**Location**: `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs` lines 681-682

**Format**:
```
"Checking rule {} of {} ({}) - ETA: {}s"
```

**Example**:
```
Checking rule 3 of 10 (code-quality/no-todo-comments) - ETA: 120s
```

**Components**:
- `current` - Current rule number
- `total_rules` - Total number of rules
- `rule.name` - Rule name
- `eta_secs` - ETA in seconds (no decimals)

### 3. Progress Notifications (check/mod.rs)

The MCP tool also sends progress notifications with metadata:

**Milestones**:
- `PROGRESS_START = 0` - "Starting rules check"
- `PROGRESS_INITIALIZED = 10` - "Rule checker initialized"
- `PROGRESS_CHECKING = 20` - "Checking files against unfiltered rules"
- `PROGRESS_COMPLETE = 100` - "Rules check complete"

**Metadata Fields**:
- `rule_names` - Filter for rule names
- `file_paths` - Patterns being checked
- `category` - Category filter
- `severity` - Severity filter
- `current` - Current rule number (filtered rules)
- `total` - Total rules count (filtered rules)
- `rule_name` - Current rule being checked
- `eta_seconds` - Estimated time remaining
- `violations_found` - Final count of violations
- `files_with_violations` - Number of affected files
- `violation_count_by_severity` - Breakdown by severity
- `duration_ms` - Total execution time
- `todos_created` - Number of todos created (if applicable)

## Format Consistency Observations

### Similarities
- Both use ETA in seconds
- Both show current/total progress indicators
- Both include the rule name

### Differences
- File checks use decimal precision for ETA (`.1f`), filtered rules use integers
- File checks show "remaining" count explicitly, filtered rules don't
- File checks show file path, filtered rules don't (they check per-rule)
- Bracket style differs: `[5/20]` vs `3 of 10`

## Context

The two different formats exist because:

1. **Individual file checks** (checker.rs) - Used when streaming checks across multiple files for unfiltered rules. Each message represents checking one file against one rule.

2. **Filtered rules** (check/mod.rs) - Used when checking rules that have tool filters. These require creating a proxy server per rule, so progress is tracked at the rule level rather than file level.

## Recommendations for Future Work

If standardization is desired:

1. Consider using consistent bracket style: either `[X/Y]` or `X of Y` everywhere
2. Consider consistent ETA precision (either always 1 decimal or always integer)
3. Consider whether "remaining" count adds value vs just showing `[X/Y]`
4. Document the semantic difference between the two contexts in user-facing docs
