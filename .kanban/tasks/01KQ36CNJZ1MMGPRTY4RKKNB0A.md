---
assignees:
- claude-code
depends_on:
- 01KQ36C3JQ5GKVYXAYW66J4H9H
position_column: todo
position_ordinal: fd80
project: acp-upgrade
title: '(Optional) Adopt newly-stable ACP 0.11 features: session/list, session_info_update, boolean toggle config option'
---
## What

Decide whether to opt into stable features added between 0.10.4 and 0.11.1.

Per the 0.11.x changelog, three things stabilized in this window:
1. **`session/list`** — agents can advertise the list of resumable sessions. Useful for the kanban-app history UI and CLI session pickers.
2. **`session_info_update`** — agents can push session metadata changes mid-flight (e.g. updated title, mode change). Could replace some of our manual `SessionUpdate` plumbing.
3. **Boolean toggle config option type** — for the session config options surface; only relevant if/when we surface session config to clients.

For each:
- Decide adopt / defer / decline. Document the reasoning.
- If adopting, add the implementation tasks (separate kanban tasks linked to this one) — implementation, tests, conformance fixtures, CLI exposure.
- If deferring/declining, note why and what would change the decision.

Also explicitly **decide on the `unstable` feature flag**: should any of (`session/close`, elicitation, NES, additional directories, providers, multiple auth methods) be turned on now? Default answer is no until there's a concrete user need; capture that decision here.

## Acceptance Criteria
- [ ] Decision document for each of the three stabilized features (adopt/defer/decline + reasoning) added to the task comments.
- [ ] If any are adopted, follow-up implementation tasks are created in the `acp-upgrade` project linked back to this one via `depends_on`.
- [ ] Decision on the `unstable` feature flag captured (default: do not enable).

## Tests
- [ ] No tests in this discovery task; adoption tasks (if any) carry their own automated tests (TDD: failing test first against new client → make it pass).

## Workflow
- Discovery + decision task. No code changes here unless deciding to enable a feature flag in `Cargo.toml`.

## Depends on
- 01KQ36C3JQ5GKVYXAYW66J4H9H (workspace already green on 0.11).