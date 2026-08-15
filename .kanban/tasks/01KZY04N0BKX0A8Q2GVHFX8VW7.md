---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m00ctm6wp8sgdw12h3sfqa79
  text: |-
    Picked up. Research done.

    Two sources today:
    - `crates/mirdan/src/strategy/mod.rs` — `const PERMISSIONS_DENY_POINTER: &str = "/permissions/deny"`, read at `deny_tool` and `allow_tool`.
    - `crates/mirdan/src/install/profile.rs` — private `POINTER_KEY_PERMISSIONS`, `POINTER_KEY_DENY`, `permissions_deny_pointer()`, read at `desired_edit_redirect_fragment` and `apply_edit_redirect_at`.

    DECISION — the single source goes in a small module under the strategy layer, `crate::strategy::claude_settings`, not in `settings.rs`. The evidence is what each module says it owns:

    - `settings.rs` opens with "They are intentionally agent-agnostic: the caller supplies the JSON pointer/key and the desired value." A Claude Code schema key put there breaks that declared contract.
    - `strategy/mod.rs` opens with "All agent-specific configuration knowledge ... lives here", and `ClaudeCodeStrategy` is documented as owning "the `permissions.deny` array in the scope's settings file ... This is the single place those Claude specifics live." The pointer IS Claude Code schema knowledge, so the strategy layer is its home.
    - The dependency edge already runs install -> strategy (`install/applier.rs` has `use crate::strategy::{self, AgentConfigStrategy}`), so `install::profile` reading the shared module adds no new edge and no cycle.

    A separate small file, not free items in `strategy/mod.rs`, because both `strategy::ClaudeCodeStrategy` and `install::profile` read it and a named module states what the three items are.
  timestamp: 2026-08-14T15:04:10.716382+00:00
- actor: claude-code
  id: 01m00dfjetd7beaqqt0g4bptzk
  text: |-
    Implementation landed.

    New file `crates/mirdan/src/strategy/claude_settings.rs` holds the ONE declaration of `POINTER_KEY_PERMISSIONS`, `POINTER_KEY_DENY` and `permissions_deny_pointer()`. Three call sites read it: `strategy/mod.rs` `deny_tool` and `allow_tool`, `install/profile.rs`, and `status.rs`.

    MUTATION PROOF, not an assertion. The module was first written with the pointer as the literal `"/permissions/deny"` — the exact defect the card names — and the test was run against a changed key.

    - Round 1: `POINTER_KEY_DENY = "denyList"`, pointer still the literal. 4 tests FAIL: `strategy::claude_settings::tests::pointer_resolves_the_array_the_keys_name`, `strategy::tests::claude_deny_and_allow_tool_project`, `strategy::tests::claude_deny_tool_user_writes_global_settings`, `strategy::tests::claude_deny_tool_local_writes_settings_local_json`.
    - Round 2: same changed key, pointer now `format!("/{POINTER_KEY_PERMISSIONS}/{POINTER_KEY_DENY}")`. The same 4 tests PASS. Deriving the pointer is what saves them.
    - Round 3: the other key, `POINTER_KEY_PERMISSIONS = "permissionRules"` with the literal pointer back. The same 4 tests FAIL. Both keys are covered, not only one.
    - Restored to `"permissions"` / `"deny"` and the derived pointer.

    The three `strategy::tests` failures are what tie the guard to the two call sites the card names. They read the written settings file through a new `denied_tools` helper that addresses the array by the two keys, so a hardcoded pointer at either call site goes red the moment a key moves.

    SWEEP — every `const` VALUE against every string literal, substring, not whole-literal.

    Scanned: 29 string consts, 4107 string literals across `crates/mirdan/**/*.rs`. Raw substring matching gives 612 hits, nearly all coincidental English-word overlap (a const `"agent"` inside the unrelated word `"agent-config"`, a const `"skill"` inside the test name `"test-skill"`). Restricting to the shape the card describes — the const value standing as a DELIMITED SEGMENT of a longer literal, which is how `"permissions"` sits inside `"/permissions/deny"` — leaves 209 hits: 28 in production code and 181 in tests.

    The 28 production hits, every one read:

    - 22 x `VALIDATOR_RULES_DIR = "rules"` and 1 x `USER_EDITED_FIXTURE`, all in `retired_validators.rs`. Every one is an `include_str!` argument or the `relative_path` beside it. `include_str!` takes a literal and cannot take a `const`, and the table records a historical retired layout, not the live directory. Not fixable and not the same contract.
    - 3 x `STORE_README_FILE_NAME = "README.md"` in `install/profile.rs`, again `include_str!` paths. They name a repository source file; the const names the file mirdan writes into the store. Two files, one common name.
    - 1 x `AGENT_ITEM_LABEL = "agent"` inside `"agent-config"` — a component name, not composed from the label.
    - 1 x `UNKNOWN_VERSION = "latest"` (a `list.rs` placeholder for a package with no version) inside `"{}/api/packages/{}/latest"` (the registry HTTP endpoint in `registry/client.rs`). Two contracts, one spelling.

    Nine hits of the `permissions.deny` contract itself remain, and they are all tests: 8 in `settings.rs` tests using `"/permissions/deny"` as the example pointer for the agent-agnostic primitives, and 1 panic message `"permissions.deny must be an array"` in `edit_redirect_tests.rs`. The `settings.rs` eight are self-consistent fixtures — each test passes the pointer AND builds the expected document from the same literal — so a Claude Code rename cannot make them lie about the primitive they test, and `settings.rs` declares itself agent-agnostic, so importing the Claude keys there would break its own stated contract. Left alone, and named here.

    PRODUCTION RESTATEMENTS OF THE `permissions.deny` CONTRACT: three before this change, zero after. The card named one (`strategy/mod.rs`). The sweep found a second production site the card did not name: `status.rs` `permissions_present` read the array with `.get("permissions").and_then(|p| p.get("deny"))` while `profile.rs` documents that installer and detector "derive from one source and cannot drift" — true of the tool list, false of the keys. Both now read `claude_settings`.

    ARCHITECTURE.md needs no update. It carries no internal module map for mirdan, and the change adds a submodule inside an existing module along the install -> strategy edge that already exists.
  timestamp: 2026-08-14T15:15:37.050247+00:00
- actor: claude-code
  id: 01m00dfr725jc80shtzyjs0b8d
  text: |-
    ### implement — changed
    - evidence: 4 files — crates/mirdan/src/strategy/claude_settings.rs (new), crates/mirdan/src/strategy/mod.rs, crates/mirdan/src/install/profile.rs, crates/mirdan/src/status.rs. `cargo nextest run -p mirdan` 522 passed, 0 failed. `cargo nextest run --workspace` 14195 passed, 0 failed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-14T15:15:42.946083+00:00
- actor: claude-code
  id: 01m00dq4s14tgpywwnb2wejzbe
  text: |-
    ### review — clean
    - evidence: `review sha 333da3fcf~1..333da3fcf`. counts: findings 0, confirmed 0, refuted 0, attempted 9, failed 0, skipped 0. Report scope line: "reviewed the diffs only — lines this change added or modified. 4 file(s) reviewed, 0 not reviewed." No prior Review Findings sections on the card. Zero findings dropped for a false premise, because the engine raised none.
    - next: none. Task moved to done.
  timestamp: 2026-08-14T15:19:45.185685+00:00
- actor: claude-code
  id: 01m00dr1ezqkhsmd59d11jvmdy
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 4 files. The single source is a new `strategy/claude_settings.rs`, and the placement was reasoned from what each module declares about itself rather than by convenience: `settings.rs` opens by calling itself "intentionally agent-agnostic: the caller supplies the JSON pointer/key", so a Claude Code schema key there breaks its own contract, while `strategy/mod.rs` opens with "All agent-specific configuration knowledge lives here". The install → strategy edge already exists via `install/applier.rs`, so this adds none.
    - The guard test was proved by MUTATION in three directions, not asserted: with the pointer written as a literal, changing `POINTER_KEY_DENY` turns 4 tests RED; switching the pointer to the derived form with the key still changed turns the same 4 GREEN; restoring the literal and changing `POINTER_KEY_PERMISSIONS` instead turns the same 4 RED. Three of the four are the `strategy::tests` deny/allow tests, which is what binds the guard to the two call sites the card names.
    - The sweep found a SECOND production site the card never named. 29 string consts against 4107 literals; raw substring gives 612 hits, almost all coincidental word overlap, and 209 in the shape that matters (a const value as a delimited segment of a longer literal). Of the 28 production hits, 26 are `include_str!` arguments, which a macro cannot take a const for. The real one: `status.rs::permissions_present` read the two keys as literals, while `install/profile.rs` documents that the installer and that detector "derive from one source and cannot drift" — true of the tool list, false of the keys. Production restatements: 3 before, 0 after. Nine remain in tests, 8 of them self-consistent fixtures in the agent-agnostic `settings.rs`, deliberately left rather than importing Claude keys into a module that declares itself agent-agnostic.
    - test: green — 522 mirdan, 14195 workspace, 0 failed, 0 skipped. fmt and clippy clean.
    - commit: 333da3fcf
    - review: clean — 0 findings, 9 pairs attempted, 0 failed, 0 skipped. Task moved to done.

    First-round clean, which is worth noting: the report carried `reviewed the diffs only — 4 file(s) reviewed, 0 not reviewed`, so ^apb04az's engine served this call. Under the old whole-file engine, four mirdan files would very likely have produced pre-existing findings and cost a round.
  timestamp: 2026-08-14T15:20:14.559944+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffffd80
title: Remove the second source of the permissions.deny JSON pointer in strategy/mod.rs
---
`crates/mirdan/src/strategy/mod.rs:35` declares:

    const PERMISSIONS_DENY_POINTER: &str = "/permissions/deny";

`crates/mirdan/src/install/profile.rs` declared the same const with the same
value until card `^4kzxdex` round 4 replaced it with a function that builds the
pointer from `POINTER_KEY_PERMISSIONS` and `POINTER_KEY_DENY`.

The pointer names the Claude Code settings schema. It is one external contract
with two sources in one crate. A change to the schema must reach both, and
nothing makes that happen.

## What to build

Give the crate one source of the two keys and one source of the pointer built
from them, and let `strategy/mod.rs` read it. `permissions_deny_pointer`,
`POINTER_KEY_PERMISSIONS` and `POINTER_KEY_DENY` are private to
`install::profile` today, so this needs a decision on where the single source
lives (probably `settings.rs` or a small module both can read), not only a
visibility change.

## Done when

- One declaration of the pointer in the crate.
- `strategy/mod.rs:194` and `strategy/mod.rs:213` read it.
- A test fails when either key changes but the pointer does not.

## How this was found

A substring detector (every `const` VALUE against every string literal, not
whole-literal equality) run for card `^4kzxdex`. Whole-literal sweeps in
earlier rounds could not see it.

#tool-validators