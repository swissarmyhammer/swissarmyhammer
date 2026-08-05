---
assignees:
- claude-code
depends_on:
- 01KZ934SNEJ1TXNS2G9Q4909TF
position_column: todo
position_ordinal: fe80
title: Tool-runner execution path in the review engine
---
Run tool rules in the review engine. No LLM reads the code for a tool rule.

The contract is `builtin/validators/README.md`.

Work:
- Pair matching stays the same — the existing `ValidatorMatch` path. When a matched rule has a `tool` block and the tool is healthy, run the `run` script instead of an LLM reviewer.
- Execute `run` with the shell (bash). `scope: files` — pass the changed files as script arguments (`"$@"`). `scope: workspace` — run once at the workspace root with no arguments, then keep only findings in changed files.
- Read findings from stdout, one per line: `path:line: message` or a JSON object `{file, line, message}`. Empty stdout = clean. Parsing these two line shapes is the ONLY parsing the engine does — no format/jq/regex config; the rule's pipe already did the mapping.
- Exit 0 = the script judged the code. Nonzero exit = tool error, not findings — raw stderr goes to the diagnosing agent, and no findings are read.
- `supersedes`: when a healthy tool rule matches a file, skip the named prompt rule for that file. When the tool is missing or unhealthy, run the named prompt rule as today.
- Stream findings on the existing channels. Skip adversarial verification for tool findings.
- When a tool needs a config file, write it to a temporary path and pass it with a flag. Never change the project's lint config.

Acceptance:
- A real-pipeline test: run `review file` on a fixture with the tool present, and see tool findings with zero LLM validator calls for that pair.
- A supersedes test: tool present skips the prompt rule; tool absent runs the prompt rule and the report notes the fallback.
- A nonzero-exit test: the run is reported as a tool error, not as clean and not as findings.

#tool-validators