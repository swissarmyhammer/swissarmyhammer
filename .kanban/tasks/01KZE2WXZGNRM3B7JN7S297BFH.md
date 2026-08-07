---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9080
title: Doctor has no fix hint for a toolchain-component tool rule
---
A tool rule whose tool ships with the language toolchain declares no `install.commands`, because `install_command_pins_version` requires a version pin and a `rustup` component has no package version to pin. `missing-docs-rust` is the first such rule.

The cost: when clippy is missing, `sah doctor` shows the row as a warning with the degradation detail and the prompt-fallback note, but `degraded_fix` returns `None`, so the user gets no command to run.

Work:
- Give a tool rule a way to state the fix a person runs when there is no pinnable package — for example a `doctor.fix_hint` string that `degraded_fix` falls back to.
- `missing-docs-rust` states `rustup component add clippy`.
- The install lifecycle must not run a fix hint. It is text for a person, never a command the engine tries.

Acceptance:
- With clippy off PATH, the doctor row for `missing-docs-rust` names `rustup component add clippy`.
- `install_command_pins_version` still guards every real install command.

#tool-validators