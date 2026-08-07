---
assignees:
- claude-code
depends_on:
- 01KZEB9V0GBG049K0PPGWHWYT8
position_column: todo
position_ordinal: ff9080
title: 'complexity tool rules: Rust + Python (clippy, ruff)'
---
Add tool rules to `builtin/validators/code-hygiene` that supersede the `cognitive-complexity` and `function-length` prompt rules. Follow the missing-docs pattern and the README contract.

Rust — one rule, one clippy run, workspace scope:
- `cargo clippy --message-format=json --quiet -- -W clippy::cognitive_complexity -W clippy::too_many_lines`
- The jq pipe selects the two lint codes. Selection is attribution, not exemption.
- Thresholds: the run script writes a temporary `clippy.toml` and points `CLIPPY_CONF_DIR` at its directory. Set `cognitive-complexity-threshold = 15` (the prompt gate) and `too-many-lines-threshold = 250`. Never read or change the project clippy.toml. Verify CLIPPY_CONF_DIR behavior before you rely on it.
- `supersedes: [cognitive-complexity, function-length]` — blocked by ^gwhwyt8.

Python — two rules, files scope, ruff:
- `complexity-python`: `ruff check --isolated --no-cache --config "lint.mccabe.max-complexity=15" --select C901 --output-format json "$@"` piped through jq. Supersedes `cognitive-complexity`. Note in the rule body: C901 is cyclomatic, not Sonar cognitive; the tool gate replaces the prompt gate.
- `function-length-python`: PLR0915 with a statement threshold that approximates 250 code lines. State the chosen number and the reason in the rule body. Supersedes `function-length`.

Both languages: fail/pass fixture pairs in `fixtures/`. The fail fixture holds one function over each gate; the pass fixture holds the same shapes under the gates. Doctor shows the fixture rows.

The `complexity` tree-sitter probe stays. A language without a healthy tool keeps the probe + prompt path — that is the designed fallback.

#tool-validators