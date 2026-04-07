---
assignees:
- claude-code
depends_on:
- 01KND4A5F0ED791N2AF8RMMD0E
position_column: done
position_ordinal: ffffffffffffffffffed80
title: 'Self-test: cargo install and verify Stop validators fire with agent execution'
---
## What

After implementing the pre-content fix, install the updated AVP binary and verify the full end-to-end flow works in a real Claude Code session:

1. `cargo install --path avp-cli` to install the new binary
2. Create a source file (`.rs` or `.ts`) to trigger code-quality and test-integrity Stop validators
3. Edit the file to verify proper unified diffs (not new-file diffs)
4. Let the Stop hook fire and observe:
   - Sidecar diffs in `.avp/turn_diffs/<session_id>/`
   - Turn state in `.avp/turn_state/<session_id>.yaml`
   - AVP log showing Stop validators executing with the agent
   - Validator results (pass/fail) logged
5. Verify session isolation by checking subagent diffs don't clobber parent

### Verification checklist:
- [ ] `code-quality` Stop validator fires when `.rs` files are changed
- [ ] `test-integrity` Stop validator fires when test files are changed
- [ ] `security-rules` PostToolUse validator still fires immediately on Write/Edit
- [ ] `command-safety` PreToolUse validator still fires on shell commands
- [ ] Sidecar diffs contain proper unified diffs (after pre-content fix)
- [ ] Diffs are session-scoped (check directory structure)
- [ ] Diffs survive past Stop for debugging
- [ ] SessionStart cleans previous session's diffs

### This is a manual verification card — no automated tests to write.
Run `cargo install --path avp-cli`, then exercise the system by making edits and inspecting `.avp/` state.

## Acceptance Criteria
- [ ] AVP binary installed successfully
- [ ] Stop validators execute with real agent (not AVP_SKIP_AGENT)
- [ ] Log shows validator results for Stop hook
- [ ] Sidecar diffs are correct

## Tests
- [ ] Manual: create .rs file, edit it, observe Stop validators
- [ ] Manual: inspect .avp/turn_diffs/ and .avp/log"