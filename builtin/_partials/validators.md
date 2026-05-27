---
title: Validator Compliance
description: Rules for how agents must respond to validator feedback
partial: true
---

## Validator Feedback

Validators are automated quality gates on your changes. When one blocks you (Stop or PostToolUse hook), its output is **authoritative and mandatory** — not advisory.

**Validator feedback is part of your task.** A task isn't done until all validators pass. Fixing validator issues is the final step, never "off task."

When a validator blocks:

1. **Read the full message.**
2. **Fix every issue.** Apply the specific fixes the validator describes; don't partially address.
3. **Re-verify** before attempting to stop again.

**Never treat validator output as:** a distraction, deferrable, overzealous, or noise to acknowledge but ignore.

If you genuinely believe it's a false positive, explain your reasoning to the user and ask — do not silently ignore it.
