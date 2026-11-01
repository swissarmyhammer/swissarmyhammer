---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m16retr024m1xhkx4t1fdbhq
  text: |-
    ## Decision (from the user, 2026-08-29)

    **Any locally-readable path stays legal. Every attachment copy is audited.**

    The card asked three questions and this answers all three:

    - Should `copy_attachment`'s source be restricted to an allow-listed root? **No.** Attaching a file from anywhere the process can read it is the intended capability, and narrowing it to a staging directory or the board tree would break the ordinary "attach this file from my Downloads" flow for no gain in this threat model.
    - Should `AddAttachment` require proven read access some other way, such as a prior stage step? **No.** That adds a round trip and new API surface to defend against a caller that is already trusted.
    - Is this only a concern for untrusted or remote MCP clients? **It is not a concern here at all.** Every contributor to this board is the user's own agent on the user's own machine, running as the user's own OS account. A caller that can reach this MCP tool can already read the same files directly. The copy grants no access the caller did not have.

    So the fix is not a restriction. It is a record.

    ## What this changes the acceptance to

    The card's original acceptance asked for a test proving a path outside the allowed roots is REJECTED. Under this decision nothing is rejected, so that item cannot stand as written. It becomes:

    - The decision above is recorded on the card. Done by this comment.
    - Every attachment copy emits an audit record naming the source path, the destination, and the byte count.
    - A regression test proves the record is emitted for a copy from outside the board tree — the exact case the card called an exploit.

    Use `tracing`, never `eprintln!`: stderr is swallowed under MCP, so an audit line on stderr would not survive the transport.
  timestamp: 2026-08-29T12:38:35.520201+00:00
- actor: claude-code
  id: 01m16rt16ap7wrdjrhrew6g6kn
  text: |-
    ## Research

    Picked up. Following the user decision in the comment above: no path restriction, an audit record instead.

    Where the audit goes: `io::copy_attachment`. It is the single function that runs the `fs::copy`, so every copy passes through it and no caller can bypass it. `context.rs::resolve_attachment_value` is only ONE of its callers — `swissarmyhammer-kanban/src/commands/paste_handlers/attachment_onto_task.rs` reaches the same copy through a staged temp file, and any future caller would too. An audit emitted in the caller would miss those.

    Level: `info!`. The copy is a routine success, not a fault, so `warn!`/`error!` would misreport it. `debug!`/`trace!` are filtered out at the default subscriber level, and an audit record that the default configuration discards is not an audit record. `info!` is the lowest level that survives the normal filter.

    Log capture in tests: the workspace already standardises on `tracing-test` (`tracing-test = { version = "0.2", features = ["no-env-filter"] }` in the root `Cargo.toml`, used by 13 crates). Reference pattern: `crates/swissarmyhammer-kanban/tests/perspective_migration.rs` — `#[traced_test]` on the test plus `logs_contain(...)` assertions. `swissarmyhammer-entity` does not have the dev-dependency yet, so it gets added.
  timestamp: 2026-08-29T12:44:42.570268+00:00
- actor: claude-code
  id: 01m6bqxjk70hgpp1vk4rceqsqn
  text: |-
    ### implement — changed
    - Re-checked code: the audit in `io::copy_attachment` (tracing info: source, destination, bytes) and its io-level test are already committed (dc0246e2e). So a RED run against the real code was not possible.
    - Added op-level test `test_add_attachment_audits_a_source_outside_the_board` in `crates/swissarmyhammer-kanban/src/attachment/add.rs`. It runs the real `AddAttachment::execute` with a source outside the board tree.
    - Non-vacuity proof: with the `info!` call disabled, the test failed ("the audit record must name the source path"). Code restored; test passes.
    - evidence: 1 file changed; `cargo test -p swissarmyhammer-kanban -p swissarmyhammer-entity`: 0 failed, 0 warnings, 0 ignored (kanban lib 1402 passed).
    - next: /review
  timestamp: 2026-11-01T14:23:51.399377+00:00
- actor: claude-code
  id: 01m6br0bhb0m2erz8s766t8cre
  text: |-
    ### test — green
    - evidence: cargo build --workspace --all-targets exit 0, 0 warnings; cargo test -p swissarmyhammer-kanban -p swissarmyhammer-entity -p kanban-cli exit 0, 0 failed, 0 ignored, 0 warnings; test_add_attachment_audits_a_source_outside_the_board ok. Note: cargo nextest is 0.9.116 but the repo needs 0.9.145, so cargo test was used.
    - next: review
  timestamp: 2026-11-01T14:25:22.475690+00:00
- actor: claude-code
  id: 01m6br0ybjt53ag21yw0b2f9a7
  text: |-
    ### commit — changed
    - evidence: 00e7513fb test(kanban): cover the AddAttachment audit record for sources outside the board (crates/swissarmyhammer-kanban/src/attachment/add.rs)
    - next: none. Local commit only, not pushed.
  timestamp: 2026-11-01T14:25:41.746228+00:00
- actor: claude-code
  id: 01m6br555qczmz0586m0n2p78h
  text: |-
    ### review — clean
    - scope: review sha 00e7513fb~1..00e7513fb; counts: findings 0, confirmed 0, refuted 0, attempted 7, failed 0, skipped 0.
    - evidence: crates/swissarmyhammer-kanban/src/attachment/add.rs (test_add_attachment_audits_a_source_outside_the_board runs the real AddAttachment op with a source outside the board and asserts the audit log names the source path and bytes). The audit itself is tracing::info! in crates/swissarmyhammer-entity/src/io.rs copy_attachment (field, source, destination, bytes), from commit dc0246e2e.
    - card requirements per the 2026-08-29 decision: decision recorded (comment), audit record on every copy (copy_attachment), regression test for a copy from outside the board (this commit). No unmet requirement found.
    - next: none. Task moved to done.
  timestamp: 2026-11-01T14:27:59.799843+00:00
- actor: claude-code
  id: 01m6br5j4tx0j0gbge8fp5qpaz
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — added test_add_attachment_audits_a_source_outside_the_board (audit code already existed from dc0246e2e)
    - test: green — 2074 passed, 0 failed, 0 warnings
    - commit: 00e7513fb
    - review: clean — 0 findings, task moved to done
  timestamp: 2026-11-01T14:28:13.082213+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffd880
title: AddAttachment.path lets a caller copy any locally-readable file into the board's .attachments/ (unbounded local file read)
---
## Concrete exploit path

1. An MCP client calls the `kanban` tool: `{"op": "add attachment", "task_id": "<id>", "name": "leak", "path": "/etc/passwd"}` (or `~/.ssh/id_rsa`, or any other file the process's OS user can read).
2. `AddAttachment::execute` (`crates/swissarmyhammer-kanban/src/attachment/add.rs`) takes `self.path` verbatim and sets it on the task's `attachments` field: `task.set("attachments", json!(attachment_paths))` (no validation of `path` anywhere in this command).
3. `ectx.write(&task)` runs the entity field pipeline. `resolve_attachment_value` (`crates/swissarmyhammer-entity/src/context.rs:1158`) sees a value that is not an existing stored filename and treats it as "a source file path to copy" — calling `io::copy_attachment(Path::new(value), entity_type_dir, field_name, max_bytes)` (`context.rs:1179`) unconditionally.
4. `copy_attachment` (`crates/swissarmyhammer-entity/src/io.rs:519`) does `fs::metadata(source)` then `fs::copy(source, &temp_path)` with **no check that `source` is confined to any allowed root**. `source` can be any absolute (or relative) path readable by the process.
5. The file's bytes are persisted at `<board>/tasks/.attachments/{ulid}-leak`, inside the board's own storage tree, where they remain readable afterward by anything that can read the board directory (the kanban-app GUI, another MCP client, `git` if the board is version-controlled, etc.).

## Why this is not just the capability question ^t6a2952 flagged

`^t6a2952` (triage of pre-existing io.rs/store.rs findings) already established that `copy_attachment`'s destination write is safely constrained by `sanitize_filename` + `attachments_dir()` + a ULID prefix, and that reading a caller-named source file is the intended capability for a legitimate attach-from-disk flow. That triage explicitly deferred the question "is there a real unbounded-read concern" to a new card if the capability turned out to be reachable from an untrusted boundary with no root restriction. It is: `AddAttachment.path` is a bare `String` field on an MCP-tool-exposed command with zero path validation, and `copy_attachment`'s doc comment ("Validates that the source exists and does not exceed `max_bytes`") does not mention — and the code does not implement — any restriction on which directories `source` may come from.

## What needs a decision

This is a product/security policy decision, not a code fix that can be inferred:

- Should `copy_attachment`'s `source` be restricted to an allow-listed root (e.g. a configured "uploads"/staging directory, the board's own directory tree, or the directory of a file the client already had independent access to)?
- Should `AddAttachment` require the caller to have proven read access some other way (e.g. only accept a path returned by a prior "stage this file" step) rather than accepting any absolute path directly?
- Is this only a concern for untrusted/remote MCP clients, or does it also apply to the trusted local-agent case (in which case the fix is different — e.g. audit logging rather than a hard block)?

## Acceptance

- A decision recorded on which source roots are legitimate for attachment copies.
- `copy_attachment` (or its caller) enforces that decision.
- A regression test proving a path outside the allowed root(s) is rejected (e.g. attempting to attach `/etc/passwd` returns an error, not a copied file).

## Origin

Spun out of ^t6a2952 (triage of 13 pre-existing findings against swissarmyhammer-entity io.rs/store.rs) per that task's own acceptance criterion: "Anything confirmed as a genuine security issue... is lifted into its own new kanban card with a concrete exploit path — not left buried in this triage list." #security #bug #kanban