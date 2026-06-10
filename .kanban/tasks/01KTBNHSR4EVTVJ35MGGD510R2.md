---
assignees:
- claude-code
depends_on:
- 01KTBN925WPAWDYXS12W5HETEH
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff180
project: local-review
title: 'Validator format & directories: drop `trigger`, add `probes`, XDG user dir + ./.validators'
---
## What
Simplify the rules-as-data format and directory layout now that nothing hooks.

1. **Drop the `trigger` field; add `probes`.** Remove `trigger:` from every `builtin/validators/*/VALIDATOR.md` and from the `RuleSet` frontmatter parser/type in `swissarmyhammer-validators`. Add **`probes: Vec<String>`** to the `RuleSet` frontmatter (parsed as plain strings here — validation that each name is a real catalog entry is done by the probe registry's `probe_exists` + `check validators`, NOT here). After this, a validator declares: `name`, `description`, `match.files` (globs), `severity`, optional `tags`, **`probes`**, and the rule bodies. Matching keys on file globs only (the tool-name/hook-type match axes are removed with hooks).
2. **Stray `trigger` policy.** The loader is **lenient** — it ignores unknown/legacy keys (a leftover `trigger` still loads). `check validators` is **strict** — it flags a stray `trigger` so `sah doctor` nudges the user to remove it. (Assert both behaviors in tests.)
3. **Preserve `@file_groups` includes.** The existing `@file_groups/source_code`-style include expansion in `match.files` must keep working (the builtin validators use it); only the surrounding schema changes.
4. **Directories (XDG).** Replace the `.avp`/`$XDG_DATA_HOME/avp` layout with:
   - **User: `$XDG_DATA_HOME/validators/`** (default `~/.local/share/validators/`) — resolved via the existing `ManagedDirectory`/`xdg_data()` helper (the same mechanism the old `AvpConfig` used; introduce a `ValidatorsConfig`/equivalent `ManagedDirectory` type). NOT a bare `~/.validators` dotfile.
   - **Project: `./.validators/`**.
   - **Builtin: embedded** in the binary (lowest precedence).
   Precedence stays builtin → user → project. Update the loader's directory constants and `doc/src/concepts/validators.md`. `AvpConfig` can be removed here once the loader stops referencing `$XDG_DATA_HOME/avp`.

## Acceptance Criteria
- [ ] No `trigger` key remains in any `VALIDATOR.md` or in the `RuleSet` type; loading a validator without `trigger` succeeds; a validator WITH a stray `trigger` still loads (lenient) but `check validators` flags it.
- [ ] `RuleSet` frontmatter parses `probes` into `Vec<String>` (no catalog validation here).
- [ ] The loader discovers validators from builtin (embedded) + `$XDG_DATA_HOME/validators` + `./.validators` via `ManagedDirectory`, builtin → user → project precedence; no code references `.avp/` or `$XDG_DATA_HOME/avp/`.
- [ ] `@file_groups` include expansion still resolves; matching uses file globs only.
- [ ] `doc/src/concepts/validators.md` reflects the new format and XDG directories.

## Tests
- [ ] Loader test: a `VALIDATOR.md` with no `trigger` and a `probes:` list parses (probes captured as strings); a leftover `trigger` is ignored by the loader.
- [ ] Directory test (temp `XDG_DATA_HOME` + temp CWD): a validator in `$XDG_DATA_HOME/validators` and one in `./.validators` both load; project overrides user overrides builtin for same-named sets.
- [ ] `@file_groups/source_code` expansion test still passes; `cargo test -p swissarmyhammer-validators` green.

## Workflow
- Use `/tdd` — write the no-`trigger`/`probes`-parse test and the XDG + `./.validators` precedence test first. Reuse the existing precedence/glob code and the `ManagedDirectory`/`xdg_data` helper; only the directory constants and frontmatter schema change. Use a temp `XDG_DATA_HOME` env + CWD guard for the directory test (test-isolation conventions).