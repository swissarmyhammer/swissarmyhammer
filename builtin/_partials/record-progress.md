---
title: Record Progress
description: How to keep a conversation log on a kanban task — read prior context, then record milestones, failed approaches, discoveries, and blockers
partial: true
---

The task's comment thread is institutional memory for the next agent (and the user) working the card. Read it before starting; write to it as work happens.

**Before starting a card**, read the prior conversation:

```json
{"op": "list comments", "task_id": "<id>"}
```

Earlier attempts, review notes, and blockers live there — don't repeat work the log already rules out.

**As work happens**, record it on the task:

```json
{"op": "add comment", "task_id": "<id>", "text": "<what happened>"}
```

Record more than progress — record what the next agent needs to know:

- **Milestones** — picked up, research done, implementation landed, moved to review.
- **What did not work** — failed approaches, dead ends, reverted attempts, and WHY they failed, so the next agent doesn't burn the same tokens repeating them.
- **Interesting discoveries** — surprising behavior, latent bugs found along the way, non-obvious constraints, useful context that isn't in the card description.
- **Blockers** — what's blocking and what was tried.

Comments are attributed to the dispatching actor automatically — no need to sign them.
