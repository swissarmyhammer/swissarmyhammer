# `.skills/`

The **skill store** for SwissArmyHammer (`sah`). This directory is created and
maintained by `sah init`.

## What's here

Each subdirectory is one **skill** — a focused, reusable instruction set an
agent can invoke (for example `commit/`, `review/`, `plan/`). A skill is a
folder with a `SKILL.md` (its instructions and metadata) plus any reference
files it needs. Skills here are symlinked into each detected agent's skill
directory (such as `.claude/skills/`) so every agent shares a single copy.

## Customize and override

Skills resolve with this precedence — **later wins**:

    built-in (shipped in sah)  <  user (~/.skills/)  <  this project (./.skills/)

A skill in this folder therefore overrides a user-level or built-in skill of the
same name, and any skill you add here is picked up immediately.

- **Add your own skill** — create `./.skills/<name>/SKILL.md`. Skills you add are
  never touched by `sah init`.
- **Replace a built-in** — give your skill the same name as a built-in; yours
  wins by the precedence above.

`sah init` re-deploys the built-in skills on every run, so edits made directly to
a *built-in* skill's files here are refreshed the next time it runs. Keep your
changes as your own named skill so they always persist.

## Learn more

Run `sah --help`.
