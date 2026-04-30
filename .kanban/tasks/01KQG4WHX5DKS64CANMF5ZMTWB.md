---
assignees:
- claude-code
position_column: todo
position_ordinal: fe80
project: acp-upgrade
title: 'agent-client-protocol-extras: get_test_name_from_thread() picks wrong leaf for rstest cases'
---
## What

`agent_client_protocol_extras::get_test_name_from_thread()` uses
`name.rsplit("::").next()` to extract the leaf component of the current
thread name, then uses that as the fixture filename stem. Under
`rstest` parametric tests, the thread name shape is

  `<crate>::<module>::<test_fn>::case_<n>_<param_label>`

so the leaf is `case_<n>_<param_label>` (e.g. `case_1_llama`) — the
real test function name (`test_minimal_initialization`) is the
component *before* the case suffix.

Fixtures recorded with this helper end up at
`.fixtures/llama/case_1_llama.json` instead of
`.fixtures/llama/test_minimal_initialization.json`, and the fixture
verification step fails to find them.

## Where

- `agent-client-protocol-extras/src/fixture.rs::get_test_name_from_thread`

## Acceptance Criteria

- [ ] When the thread name ends in `::case_<digits>_<label>`, the
      helper returns the function-name component (the segment before
      the case suffix), not the case suffix itself.
- [ ] When the thread name has no case suffix, the existing leaf
      behaviour is preserved.
- [ ] Unit tests cover both shapes (with and without case suffix).

## Tests

- [ ] Add a unit test that spawns a thread named
      `integration::initialization::test_minimal_initialization::case_1_llama`
      and asserts the returned name is `test_minimal_initialization`.

## Discovered while

Adapting acp-conformance to ACP 0.11 (task 01KQ36AGXFCJF4PEEK2TDN6YQK).
The lib unit tests pass; the integration test failures are entirely
due to this helper picking the wrong leaf.