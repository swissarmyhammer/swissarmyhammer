---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffb580
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

- [x] Decide which layout is canonical (per-crate vs workspace) and
      document it in the helper.
- [x] Either the helper resolves to the canonical location, or the
      existing fixture trees are moved to match.
- [x] `acp-conformance` integration tests can find the recorded
      fixtures without their fixture path being constructed manually.

## Resolution

Per-crate `<crate>/.fixtures/` is canonical (matches the existing
fixture trees in `acp-conformance/`, `avp-common/`, and
`agent-client-protocol-extras/`, and the `avp-common` test helpers
that already resolve via `env!("CARGO_MANIFEST_DIR")`).

Renamed `workspace_root` -> `fixture_root`. New resolution order:

1. `CARGO_MANIFEST_DIR` directly (calling crate's manifest dir, which
   cargo sets at test runtime to the test crate). This is the per-crate
   layout the existing fixtures live under.
2. Walk up to a `[workspace]` `Cargo.toml` from the manifest dir
   (defensive fallback when the manifest dir doesn't resolve).
3. Current working directory as a final fallback for non-cargo
   callers (e.g. release binaries).

Updated all docstrings (module-level, `get_fixture_path_for`, and
`fixture_root`). Replaced the old `workspace_root_resolves_to_repo_root`
unit test with a new `fixture_root_resolves_to_calling_crate_manifest_dir`
test plus `get_fixture_path_for_lands_under_per_crate_dot_fixtures`.

Removed the stale workspace-root `.fixtures/` directory and updated
the `.gitignore` comment (kept the `/.fixtures/` ignore line so any
historical or accidental drop stays quiet).

Verified: `cargo test -p acp-conformance case_2_claude` -> 52 passed,
0 failed. Conformance tests now resolve their per-crate fixtures.

## Discovered while

Adapting acp-conformance to ACP 0.11 (task 01KQ36AGXFCJF4PEEK2TDN6YQK).
Once 01KQG4WHX5DKS64CANMF5ZMTWB lands, this is the next thing
blocking the conformance integration tests from finding their
recorded fixtures.