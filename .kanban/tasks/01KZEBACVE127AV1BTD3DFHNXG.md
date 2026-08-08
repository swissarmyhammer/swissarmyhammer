---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzgs88p6z4rgvchbm90pye75
  text: |-
    Research done. Facts measured on this machine (ruff 0.14.5, clippy 0.1.97).

    CLIPPY_CONF_DIR verified, three ways:
    - With `cognitive-complexity-threshold = 1000` / `too-many-lines-threshold = 1000` in the conf dir, both lints go silent on a probe crate that trips them at 15/250. So clippy reads the file.
    - With `clippy.toml` in the package saying 1000 AND CLIPPY_CONF_DIR saying 15/250, both lints fire. So the environment variable wins and the project file is never read.
    - A cached second and third run re-emit both warnings, so a repeat review still reports.

    The repository has no clippy.toml of its own today.

    `bash -c` runs the script, so `trap 'rm -rf "$conf"' EXIT` cleans the temporary conf dir and leaves the pipe's exit code as the script's exit code.

    Python statement threshold, measured not guessed. Ran `ruff --select PLR0915 --config lint.pylint.max-statements=0` over the CPython 3.12 standard library; the message carries each function's exact ruff statement count. Compared against code lines (blank and comment-only excluded) computed with `ast`:
    - 60 functions of 80 code lines or more: median 0.732 statements for each code line.
    - 22 functions of 120 or more: median 0.728.
    - 8 functions of 150 or more: median 0.722.
    250 code lines x 0.72 = 180. The rule sets `lint.pylint.max-statements=180`.

    Engine facts that shape the rules:
    - `run_shell` is `bash -c <script> bash <args>`; no `set -e`, no pipefail, no environment injected. cwd is the repository root for both scopes in a real review, and the fixture scratch dir when doctor runs.
    - Workspace-scope findings are kept by exact repo-relative string match; fixture attribution is by base name, so one cargo package may hold every Rust fixture.
    - `ToolSpec` is `deny_unknown_fields`.

    Three test rosters enumerate the shipped tool rules and each needs the new rules: `builtin/mod.rs` (count assertion), `review/tool_rules.rs` (`SHIPPED_*` consts plus the fixture acceptance test), `crates/mirdan/src/builtin_validators.rs` (embedded fixture file names).
  timestamp: 2026-08-08T13:33:29.670381+00:00
depends_on:
- 01KZEB9V0GBG049K0PPGWHWYT8
position_column: doing
position_ordinal: '8280'
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