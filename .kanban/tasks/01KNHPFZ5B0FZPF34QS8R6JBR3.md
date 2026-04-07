---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffe80
title: 'Coverage: daemon.rs read/send jsonrpc_message error paths'
---
## What

`swissarmyhammer-lsp/src/daemon.rs` — `read_jsonrpc_message()` and `send_jsonrpc_message()` have uncovered error-handling lines for: malformed Content-Length headers, EOF during body read, write failures, and JSON parse errors.

## Acceptance Criteria
- [ ] Test covers malformed Content-Length header (non-numeric, missing)
- [ ] Test covers EOF during body read (truncated message)
- [ ] Test covers write failure in `send_jsonrpc_message`
- [ ] Test covers JSON parse error on received body

## Tests
- [ ] Add tests in `swissarmyhammer-lsp/src/daemon.rs` (or `tests/`) using broken `Read`/`Write` implementations (e.g., `Cursor` with truncated data, a writer that returns `io::Error`)
- [ ] Each error path returns the expected error variant
- [ ] `cargo test -p swissarmyhammer-lsp jsonrpc_message` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #coverage-gap