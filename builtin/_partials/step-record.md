---
title: Step Record
description: The shared outcome vocabulary for pipeline steps — the result block a step returns to its caller, and the matching record it writes on the kanban task
partial: true
---

## Step Record

Every pipeline step ends the same way. It names the step, one outcome, and the
evidence. Use these words exactly. An orchestrator reads them back.

### Outcome words

| Outcome | Meaning | Reported by |
|---------|---------|-------------|
| `changed` | Files were modified. For `commit`, a commit was created. | implement, commit |
| `no-change` | Nothing was modified. For `commit`, there was nothing to commit. | implement, commit |
| `green` | All tests pass. Zero failures, zero warnings, zero skipped. | test |
| `red` | A test failed, or a warning is present. | test |
| `clean` | Zero new findings, and every prior finding is checked. | review |
| `findings` | Open findings are recorded on the task. | review |
| `stuck` | The step cannot continue. A human must decide. | any step |

There is no other outcome word, no severity tier, and no partial pass.

### Return the block to the caller

A step that runs in a sub agent ends its reply with this block. The orchestrator
reads the block instead of the full output of the step.

```
step: <implement|test|review|commit>
outcome: <outcome word>
evidence: <the command and counts, the commit sha, or the file:line list>
task: <^short-id, or none>
```

Write `task: none` when the step has no kanban task. A `/test` run on its own is
the usual case.

### Write the same record on the task

A step that has a task id also writes the record as a comment:

```json
{"op": "add comment", "task_id": "<id>", "text": "### <step> — <outcome>\n- evidence: <...>\n- next: <...>"}
```

A step with no task id writes nothing to the board. It returns the block, and
the caller writes the comment.

The comment is the durable copy. The chat scrolls away, and a context summary
erases it. The comment stays. Read the comments to learn what the last agent
did. Do not trust your own memory of it.
