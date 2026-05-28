---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc180
title: 'Flaky: swissarmyhammer-code-context lsp_communication::tests::test_send_request_accepts_mismatched_id_response'
---
## DONE (2026-05-28)

Reproduced on the FIRST stress run: `Err(LspError("write initialized failed: Broken pipe (os error 32)"))` (then passed 40×). So the card's "tight read timeout" hypothesis was wrong — the read path already uses a generous 30s deadline + poll loop.

Real root cause: the bug was in the **test's mock**, not the client. `LspJsonRpcClient::initialize()` sends the `initialize` request, reads the response, and THEN writes an `initialized` notification. But the python mock script sent its two messages and exited immediately — so its stdin read-end sometimes closed before the client's `initialized` write landed, giving a broken pipe. A pure ordering race between the client's post-response write and the mock's exit.

Fix (deterministic, no timing): make the mock behave like a real LSP server — after sending its response it calls `read_msg()` once to consume the `initialized` notification. That keeps its stdin open across the client's write (so the write can't break), then the script exits naturally and `child.wait()` returns. Applied to both raw-string scripts that call `initialize()` and share the race:
- `test_send_request_accepts_mismatched_id_response` (the card's named test)
- `test_send_request_skips_notifications_before_response` (identical latent race)

Verification: reproduced the broken pipe pre-fix; post-fix 50/50 + 40/40 stress runs of the `test_send_request*` family are green (5 tests each run).

Acceptance criteria:
- [x] Identified the actual failure (mock exit racing the client's `initialized` write — broken pipe), not the guessed read-timeout.
- [x] No sleep/timeout-for-synchronization; fix is a structural pipe-lifetime correction (mock stays open until it has consumed the client's notification).
- [x] Passes repeatedly (50/50) under back-to-back runs; deterministic.