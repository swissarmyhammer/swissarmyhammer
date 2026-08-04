---
assignees:
- claude-code
position_column: todo
position_ordinal: f680
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

Spun out of ^t6a2952 (triage of 13 pre-existing findings against swissarmyhammer-entity io.rs/store.rs) per that task's own acceptance criterion: "Anything confirmed as a genuine security issue... is lifted into its own new kanban card with a concrete exploit path — not left buried in this triage list." #security #bug