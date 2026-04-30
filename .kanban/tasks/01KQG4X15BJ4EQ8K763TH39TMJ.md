---
assignees:
- claude-code
position_column: todo
position_ordinal: ff80
project: acp-upgrade
title: 'agent-client-protocol-extras: get_fixture_path_for() resolves to workspace root, not crate-local .fixtures/'
---
## What

`agent_client_protocol_extras::fixture::workspace_root()` walks up
`CARGO_MANIFEST_DIR` looking for a `[workspace]` Cargo.toml, then
joins `.fixtures/<agent_type>/<test>.json` against that root. The
existing canonical fixtures live under each crate's own
`<crate>/.fixtures/` (`acp-conformance/.fixtures/`,
`avp-common/.fixtures/`, `agent-client-protocol-extras/.fixtures/`),
so the helper resolves to the **workspace** `.fixtures/` directory and
fails to find any of them.

The doc-comment explicitly says workspace-root layout is intended, so
the bug could be either:

- The helper should look at `CARGO_MANIFEST_DIR` first (= per-crate
  layout) and fall back to workspace root, mirroring the legacy 0.10
  layout.
- All callers should migrate their fixtures to the workspace root.

## Where

- `agent-client-protocol-extras/src/fixture.rs::workspace_root`,
  `get_fixture_path_for`

## Acceptance Criteria

- [ ] Decide which layout is canonical (per-crate vs workspace) and
      document it in the helper.
- [ ] Either the helper resolves to the canonical location, or the
      existing fixture trees are moved to match.
- [ ] `acp-conformance` integration tests can find the recorded
      fixtures without their fixture path being constructed manually.

## Discovered while

Adapting acp-conformance to ACP 0.11 (task 01KQ36AGXFCJF4PEEK2TDN6YQK).
Once 01KQG4WHX5DKS64CANMF5ZMTWB lands, this is the next thing
blocking the conformance integration tests from finding their
recorded fixtures.