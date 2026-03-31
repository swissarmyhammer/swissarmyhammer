---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff9580
title: Add tests for handle_merge dispatch (kanban-cli merge.rs)
---
kanban-cli/src/merge.rs:48-76\n\n`pub fn handle_merge(matches: &ArgMatches) -> i32`\n\nThe dispatch function is only tested indirectly via the E2E tests. The individual `run_*` functions have unit tests, but `handle_merge` itself — including the unknown subcommand fallback (line 72, returns 2) — is never tested directly.\n\nAdditionally, `extract_paths` (line 81) is tested implicitly but the error path (missing args returning Err(2)) is never exercised since clap enforces required args. Low priority. #coverage-gap