# Review verification harness

A committed, repeatable harness that proves a real local-model (qwen) review
works from this repository alone — no ad-hoc external checkouts.

## Pieces

- `drive.py` — a minimal MCP stdio client. It spawns
  `sah serve --model qwen --cwd sample/`, performs the MCP handshake
  (`initialize`, `notifications/initialized`, `tools/list`), then calls the
  `review` tool with
  `{"op": "review file", "path": "<sample>/src/orders.rs", "backend": "local",
  "validators": ["duplication", "magic-numbers"]}`. Any server→client request
  is answered with a method-not-found error so nothing blocks.
- `sample/` — a tiny standalone Rust crate seeded with planted, obvious
  findings: duplicated function bodies (the same summation-plus-tax logic
  copy-pasted across `orders.rs` and `invoices.rs` with renamed identifiers)
  and repeated bare magic numbers (`0.0825`, `1.75`, ...). Its `Cargo.toml`
  carries an empty `[workspace]` table so it stays **out of the repository's
  cargo workspace** — never add it to the root members list, and never "fix"
  its findings.

## Running

1. Build the CLI: `just sah` (the harness invokes `sah` from `PATH`).
2. Make sure the qwen model is available. Model selection happens via the
   `--model qwen` flag drive.py passes to `sah serve`; alternatively run
   `sah init && sah model use qwen` inside `sample/` to persist it there.
3. From the repository root:

   ```sh
   python3 scripts/review-verify/drive.py
   ```

   A real local-model review takes minutes. `--timeout <seconds>` adjusts the
   wait (default 1800).

On first run drive.py `git init`s `sample/` — `sah serve` resolves its `.sah/`
data directory (logs, code-context index) at the git root of its cwd, and the
nested repo keeps that state inside `sample/` instead of polluting this
repository's `.sah/`. Paths under a `.git` component are invisible to the
parent repository.

## What success means

`drive.py` exits 0 only when **all** of these hold, and nonzero with a clear
message otherwise:

1. The review result markdown is non-empty and findings
   (blockers + warnings + nits) > 0 — the planted findings were reported.
2. `counts.attempted > 0` and `counts.failed == 0` (the serialized
   `tasks_attempted` / `tasks_failed` fan-out tallies) — every fan-out review
   task actually ran.
3. Zero `Queue is full` lines in `sample/.sah/mcp.<pid>.log` — the agent queue
   never silently dropped a task (the failure mode that turns a review into an
   empty "clean" pass).
4. At least one `AgentMessage (` line in that log — the local model actually
   produced a reply.

## Self-test (no model required)

```sh
python3 scripts/review-verify/drive.py --self-test
```

Runs the assertion logic against synthetic passing and failing fixtures
(empty review, nonzero failed tally, queue-full log, ...) and exits nonzero if
any fixture is misjudged. Cheap enough for CI.
