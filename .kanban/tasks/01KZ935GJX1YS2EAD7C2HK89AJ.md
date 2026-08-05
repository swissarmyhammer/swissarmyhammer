---
assignees:
- claude-code
depends_on:
- 01KZ934SNEJ1TXNS2G9Q4909TF
position_column: todo
position_ordinal: ff80
title: Make the review engine doctorable per project and project type
---
`sah doctor` must report the review engine state for the current project.

The contract is the Doctor section of `builtin/validators/README.md`.

Work:
- Report the detected project types (from PROJECT_TYPE_SPECS detection).
- Report each validator set and whether it applies to this project.
- For each runner of a detected project type: tool present or missing, tool version (`check_version_command`), and fixture result.
- Run each available runner against its `fixtures/*.fail.*` and `fixtures/*.pass.*` files. A runner that fails its fixtures is reported and not used.
- Report each language on the prompt fallback, with the `install.commands` to fix it.
- Follow the agent-agnostic status pattern: `mirdan::status` style facts that `sah doctor` and `mirdan doctor` both consume.

Acceptance:
- On this repo, doctor shows the rust project type, the applicable sets, and each runner row.
- Removing a tool from PATH flips its row to missing with install commands shown.

#tool-validators