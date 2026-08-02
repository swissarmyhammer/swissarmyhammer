---
title: Record Progress
description: How to keep a conversation log on a kanban task — read prior context, then record milestones, failed approaches, discoveries, and blockers
partial: true
---

The task's comment thread is institutional memory for the next agent (and the user) working the card. Read it before starting; write to it as work happens.

The card holds two kinds of writing, and they do not mix:

- **The description is the state to act on** — subtask checkboxes and the `## Review Findings` sections. Work that is still open lives there.
- **The comments are the history** — how the card reached that state. Comments are append-only, timestamped, and attributed, so a comment never overwrites the description.

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

The final comment of each step is the **step record** — a fixed block with the step name, one outcome word, and the evidence. See **Step Record** for the shape. The free-text comments above tell the story; the step record makes it machine-readable.
