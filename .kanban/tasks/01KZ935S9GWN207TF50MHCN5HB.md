---
assignees:
- claude-code
depends_on:
- 01KZ935GJX1YS2EAD7C2HK89AJ
position_column: todo
position_ordinal: ff8180
title: Runner install lifecycle with LLM installer fallback
---
Install missing runner tools through the unified mirdan tool-install lifecycle.

The contract is the Install lifecycle section of `builtin/validators/README.md`.

Work:
- On a missing tool: try each entry in `install.commands` in order. Re-run the doctor check after each try.
- Pin tool versions in the builtin runner specs. An unpinned tool can change rules and break the gate.
- If every command fails, spawn a bounded install agent. Inputs: the runner spec, the platform, the error output. Goal: make `doctor.check_command` pass. Doctor confirms the result. The agent cannot assert success.
- If the tool is still missing, the review falls back to the prompt rule and doctor keeps a warning. A missing tool never blocks a review.
- Register the runner tools in the mirdan Profile manifest so `sah init` / mirdan install can pre-install them.

Acceptance:
- With the tool absent and a working install command, the review installs it and runs the runner.
- With all installs failing, the review completes on the prompt fallback and doctor shows the warning.

#tool-validators