---
assignees:
- wballard
depends_on:
- 01KQ7KYNBEHQGEMGND4AEG9EV6
- 01KQ35MHFJQPMEKQ08PZKBKFY0
position_column: done
position_ordinal: ffffffffffffffffffffffff8580
title: Sweep rule prompts for tool-name accuracy and remove claude-only assumptions
---
After the always-on validator MCP server lands (`01KQ35MHFJQPMEKQ08PZKBKFY0`) and the tool name mismatch is fixed (`01KQ7KYNBEHQGEMGND4AEG9EV6`), the rule prompts in `builtin/validators/**/*.md` will be slightly out of sync with the actual tool surface.

## What to fix

### 1. Tool names must match the registry

Each rule's "Available Tools" section currently reads something like:

> "You may have access to **files** (read file, glob, grep — read-only) and **code_context** (symbol lookup, call graphs, blast radius, code grep)."

After `01KQ7KYNBEHQGEMGND4AEG9EV6` lands with split tools (`read_file` / `glob_files` / `grep_files`), this wording is informal-ambiguous. Replace with explicit names that match the registry:

```markdown
## Available Tools

You may call these tools to inspect the code before deciding:

- `read_file` — read a file's contents (`{"path": "/abs/path"}`)
- `glob_files` — find files by pattern (`{"pattern": "**/*.rs", "path": "/dir"}`)
- `grep_files` — search file contents (`{"pattern": "regex", "path": "/dir"}`)
- `search_symbol`, `get_symbol`, `list_symbols` — symbol-level code intelligence
- `grep_code` — search code with semantic understanding
- `get_callgraph`, `get_blastradius` — relationship analysis

Do not guess about file contents — call the tools.
```

(Exact tool list comes from whatever `register_code_context_tools` actually exposes — verify against the runtime allowlist test in `01KQ7G1R9KRQ8RDBKYVSNEN9V4`.)

### 2. Remove leftover language assuming claude's built-in tools

Some rule prompts may reference operations the validator can't do. Look for:

- "use the bash tool to..."
- "edit the file to..."
- "write a corrected version..."
- "run the tests..."
- "check git history..."

These were never the validator's job — validators judge, they don't fix. The narrowed validator-only tool set makes them outright impossible. Sweep all prompts and replace any imperative-rewrite language with imperative-judge language.

### 3. The "## Hook Context" block in the rule template

`builtin/prompts/.system/rule.md` has a `{{ hook_context }}` interpolation that already renders. After `01KQ7EQFSMMYW80HC8PQZ2YATV` lands, that block will *also* contain a Files Changed section and unified diff blocks. Make sure the rule body's prose doesn't conflict (e.g., a rule that says "the user just edited this file" when the prompt has already shown the diff).

Read each rule's body in light of: "the prompt this is rendered into already contains a `## Files Changed This Turn` list and the diffs themselves." Trim redundancy. If a rule says "look at the changed files" — drop it, because the prompt already pointed the model at the changed files.

### 4. Verify against runtime tool list

Cross-reference the wording against what `tools/list` returns from `start_validator_mcp_server`. After `01KQ7G1R9KRQ8RDBKYVSNEN9V4` lands, that allowlist is the authority. The rule prompts must not advertise tools that aren't in the list and must not omit tools that are.

A simple way to enforce: a `cargo test` integration test that loads each rule prompt, extracts the "Available Tools" section, and asserts every tool name mentioned in that section also appears in the validator MCP server's `tools/list`. Catches drift.

## Scope

Files to touch (per `builtin/validators/`):

- `code-quality/rules/*.md` (10 rules)
- `security-rules/rules/*.md` (3 rules)
- `test-integrity/rules/*.md` (1 rule)
- Any other rule directories

That's about 14 files. Each edit is small (rewrite the "Available Tools" boilerplate, sweep for forbidden imperatives). Bulk of the work is reading carefully, not writing.

The shared partial in `_partials/` (if one exists for "Available Tools") would let this be a single edit instead of 14 — check whether one already exists. If yes, edit there. If no, this is a good time to introduce one.

## Tests

- Static check: `cargo test` integration test that asserts each rule prompt's tool-name mentions match the registry's `tools/list` output.
- Manual review: read one rule body before and after; confirm the prose makes sense given that the prompt now contains the full diff.

## Acceptance

- All rule prompts use the exact tool names registered in the validator MCP registry.
- No rule body advertises a tool the validator can't actually call.
- No rule body assumes claude built-in tools (bash, edit, write).
- No rule body redundantly tells the model to "look at the changed files" when the prompt already shows them.
- The static check passes — every tool name a prompt mentions is in `tools/list`.
- A boilerplate "Available Tools" partial in `_partials/` (new or existing) is the single source of truth for the tool list.

## Depends on

- `01KQ7KYNBEHQGEMGND4AEG9EV6` (tool name fix). Until that lands, the right names to use aren't determined.
- `01KQ35MHFJQPMEKQ08PZKBKFY0` (always-on tools). Until that lands, the registry isn't real.
- Soft order: do this after the tool surface settles, otherwise we'll edit prompts twice.

## Pairs with

- `01KQ7EQFSMMYW80HC8PQZ2YATV` (prompts include diffs). The diff section appears in every rule prompt, so this sweep also takes the prose change into account. #avp