# `.agents/`

The **agent store** for SwissArmyHammer (`sah`). This directory is created and
maintained by `sah init`.

## What's here

Each subdirectory is one **subagent** — a specialized agent persona the main
agent can delegate to (for example `implementer/`, `reviewer/`, `tester/`). An
agent is a folder with an `AGENT.md` (its instructions and metadata). Agents
here are symlinked into each detected agent's subagent directory so every agent
shares a single copy.

## Customize and override

Agents resolve with this precedence — **later wins**:

    built-in (shipped in sah)  <  user  <  this project (./.agents/)

An agent in this folder therefore overrides a user-level or built-in agent of
the same name, and any agent you add here is picked up immediately.

- **Add your own agent** — create `./.agents/<name>/AGENT.md`. Agents you add are
  never touched by `sah init`.
- **Replace a built-in** — give your agent the same name as a built-in; yours
  wins by the precedence above.

`sah init` re-deploys the built-in agents on every run, so edits made directly to
a *built-in* agent's files here are refreshed the next time it runs. Keep your
changes as your own named agent so they always persist.

## Learn more

Run `sah --help`.
