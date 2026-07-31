---
assignees:
- claude-code
position_column: todo
position_ordinal: c380
title: sah tool prints YAML, so the ralph Stop hook cannot read the decision
---
The `sah tool ...` path always prints YAML. Claude Code Stop hooks read JSON from stdout. The hook therefore cannot see the ralph decision.

## Symptom

```
$ echo '{"session_id":"probe"}' | sah tool ralph ralph check --
decision: block
iteration: 1
max_iterations: 50
reason: keep going. Iteration 1 of 50.
```

The global `--format json` flag does not change this:

```
$ echo '{"session_id":"probe"}' | sah --format json tool ralph ralph check --
decision: block
...
```

## Cause

`response_formatting::format_success_response` in `apps/swissarmyhammer-cli/src/mcp_integration.rs` converts every tool result to YAML. Its own doc comment says it is "the ONE PLACE where we convert JSON output to YAML for display". The tool execution path never reads the global `--format` value.

## Required change

Make the `sah tool` output honor the global `--format` flag (`table` | `json` | `yaml`). Keep YAML as the default for humans. Then the Stop hook command can ask for JSON.

## Acceptance

- `sah --format json tool ralph ralph check --` prints a JSON object.
- `sah tool ralph ralph check --` still prints YAML.
- A test covers both formats through the real CLI path.

Found while implementing ^6xjxebg. That card made the command exist; this card makes its output readable by the hook. #bug #cli #ralph