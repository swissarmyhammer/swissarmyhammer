---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa280
title: '[nit] skill.rs render_instructions silently swallows template errors'
---
**File**: code-context-cli/src/skill.rs (render_instructions function)\n\n**What**: The `render_instructions` function uses `.unwrap_or_else(|_| skill.instructions.clone())` to fall back to raw instructions if templating fails. This silently swallows the error -- the user gets no indication that template rendering failed.\n\n**Suggestion**: At minimum, log a warning via `tracing::warn!` so the failure is visible in the log file. Or propagate the error and let `run_skill()` report it." #review-finding