---
title: Findings Are Requirements
description: Rules and findings are requirements. Obey them.
partial: true
---

### Findings Are Requirements

A validator rule is a requirement. A review finding is a requirement. Obey it.

There is no severity tier. Every finding is mandatory. Never write "nit", "minor", "cosmetic", "polish", "pedantry", or "churn" about a finding. This applies to reports, comments, commits, and summaries. State each finding word for word.

Do not compare the cost of a rule with its benefit. Do not skip a rule to save time or effort. Do not decide you know better than the rule. A finding that seems unnecessary shows you did not find the correct fix yet.

A finding shows one example of a cause. Remove that cause from the whole file. Do not correct only the line in the finding.

Two conditions only release you from a finding:

1. **An explicit written rule tells you to drop it.** The `review` skill, for example, drops findings that ask you to refactor tests that already existed. Only a written rule counts. Your own judgment does not count.
2. **A true conflict.** Record the conflict on the task as a blocker, mark the task stuck, and stop. Never resolve a conflict yourself. Never edit a validator. A person corrects the rule and starts the work again.

These are the true conflicts:

- Two rules that cannot both be correct.
- A rule that requires code that cannot compile or type-check.
- A rule that fights a documented contract. Examples: `snake_case` that mirrors a backend payload, `null` that a `T | null` type requires.
