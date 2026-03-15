---
position_column: done
position_ordinal: b6
title: 'SHELL-5: End-to-end test with user and project config overlays'
---
## What

Integration test that exercises the full stacking behavior with real YAML files on disk. Uses tempdir to simulate the three config layers.

**Test scenario 1 — Project permits what builtin denies**:
- Builtin denies `sed\s+.*`
- Project `.shell/config.yaml` permits `sed\s+-i\s+`
- Assert: `sed -i 's/foo/bar/' file.txt` is allowed
- Assert: `sed -e 's/foo/bar/'` is still denied (no permit match)

**Test scenario 2 — User adds custom deny**:
- Builtin has standard denies
- User `~/.shell/config.yaml` adds deny `docker\s+rm`
- Assert: `docker rm container` is denied
- Assert: `docker ps` is allowed

**Test scenario 3 — Project overrides settings**:
- Builtin has `max_command_length: 4096`
- Project `.shell/config.yaml` has `settings: {max_command_length: 8192}`
- Assert: 5000-char command passes validation

**Test scenario 4 — Hot reload**:
- Load config, validate command (denied)
- Write new permit to project config file
- Load config again, validate same command (now allowed)

**Affected files**:
- `swissarmyhammer-shell/tests/config_stacking_test.rs` (new)

## Acceptance Criteria
- [ ] All 4 test scenarios pass
- [ ] Tests use tempdir — no side effects on real `~/.shell/`
- [ ] Tests demonstrate the full builtin → user → project precedence chain
- [ ] Hot reload test proves no caching/singleton behavior

## Tests
- [ ] `cargo test -p swissarmyhammer-shell --test config_stacking_test` passes
- [ ] All scenarios described above are covered