---
assignees:
- wballard
position_column: done
position_ordinal: fffffffffffffffffffffff680
title: Reconsider blanket "test file" exclusion in validator rules
---
During an avp test run, `security-rules:input-validation` passed `swissarmyhammer-common/src/sample_avp_test.rs` purely because the filename contained `test`, even though the file lives in `src/` (production code) and contains real magic numbers, hardcoded IPs, and a hard-coded `return 42` for a known input. The "this is a test file → skip" heuristic is doing more harm than good.

Two problems to fix:

1. **The matching is by substring, not by path semantics.** A file named `sample_avp_test.rs` in `src/` is not a test. Today's exceptions trigger on `_test`, `test_`, `.spec.`, `.test.` substrings (see `builtin/validators/security-rules/rules/input-validation.md` and `no-secrets.md`). Need to scope the exception to actual test locations (e.g. `tests/` directory, `#[cfg(test)]` modules, `__tests__/`, `*.spec.ts` colocated with conventional patterns) — or drop the exception entirely.

2. **Whether to exclude tests at all is itself questionable.** Tests are exactly where hardcoded credentials, magic numbers, and hardcoded returns sneak in (think: real API keys checked into a fixture, "expected value" lookups that hide implementation gaps). Default to validating tests; let users opt out per-rule if they have a real reason.

**Why:** A validator that excuses violations based on filename gives false confidence — the user saw `passed` on a file that obviously violates the rules. The point of avp is to catch this stuff.

**How to apply:** Audit every rule prompt under `builtin/validators/` for "test file" / "tests are excluded" language. Replace blanket substring-match exclusions with either (a) no exclusion, or (b) precise path/AST-based test detection that the validator dispatcher passes in as context, not something the LLM has to infer from a filename. Re-run the qwen test (edit a Rust file in a crate, stop) and confirm the new behavior.

**Repro for verification:** see the test session ending 2026-04-25 — `sample_avp_test.rs` was written with magic numbers, hardcoded IP, and `return 42`, and `input-validation` passed it on filename alone.