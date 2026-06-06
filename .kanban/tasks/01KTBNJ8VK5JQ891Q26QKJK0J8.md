---
assignees:
- claude-code
depends_on:
- 01KTBNHSR4EVTVJ35MGGD510R2
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff680
project: local-review
title: Install builtin validators to $XDG_DATA_HOME/validators (deploy + refresh)
---
## What
Materialize the embedded builtin validators onto disk on init so users can read, learn from, and copy them — exactly as builtin **skills** and **agents** are deployed today. The on-disk copy aids discoverability (`ls $XDG_DATA_HOME/validators`) even though the embedded set already loads.

- Deploy the embedded `builtin/validators/**` into **`$XDG_DATA_HOME/validators/`** (default `~/.local/share/validators/`), resolved via the same `ManagedDirectory`/`xdg_data` mechanism task 7 establishes.
- **Reference-copy policy (consistent with skills/agents):** builtin-owned files are OWNED by the installer and **refreshed/overwritten** on each install so builtin updates always propagate — the deployed builtin is a read-only reference, not an edit target (same contract as the generated `.skills/`). To customize, a user creates a NEW validator (own name, or in `./.validators`) that wins by precedence; never edit a deployed builtin in place. User-authored validators and user-created sets are NEVER touched by the installer.
- Route this through the existing deployment mechanism — REUSE the skills/agents deploy path (mirdan Profile-driven installer: store + symlink/copy), do NOT write a parallel deployer. Per the active `mirdan-install` direction, this lands as a Profile entry alongside skills and agents.
- Builtin remains the lowest-precedence layer even when materialized; user/project edits still win via loader precedence.
- The validators doctor check is NOT here — it is the dedicated "Review tool Doctorable" task (`check validators` via the `Doctorable` trait into `sah doctor`).

## Acceptance Criteria
- [ ] Init/install materializes the builtin validators under `$XDG_DATA_HOME/validators/` with the same structure as `builtin/validators/`.
- [ ] Re-running is idempotent: builtin-owned files are refreshed to current embedded content (overwritten); user-authored validators and user-created sets are untouched.
- [ ] The loader picks up the materialized builtin set; builtin → user → project precedence still holds.
- [ ] Deployment goes through the shared skills/agents installer (Profile entry), not a bespoke copy routine.

## Tests
- [ ] Integration test (temp `XDG_DATA_HOME`): run the installer → assert `$XDG_DATA_HOME/validators/<set>/VALIDATOR.md` (+ rules) exist and match the embedded source.
- [ ] Idempotency test: install twice; a hand-edited builtin-deployed file is restored to embedded content (reference-copy policy); a user-authored validator added under the dir survives untouched.
- [ ] `cargo test` green for the owning crate.

## Workflow
- Use `/tdd` — write the temp-`XDG_DATA_HOME` deploy + idempotency tests first. REUSE the skills/agents deployment/store mechanism (mirdan Profile); do not write a second copy routine. Coordinate with the `mirdan-install` project so this is a Profile entry, not a one-off.