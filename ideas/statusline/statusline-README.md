# Claude Code Custom Statuslines

A shell-based statusline installer for [Claude Code](https://claude.ai/code) that replaces the default statusline with colored progress bars, git branch info, and model details.

## What Problem It Solves

Claude Code's default statusline gives you minimal context while working. This installer drops five purpose-built statusline scripts into `~/.claude/` and wires one up automatically. You get at-a-glance context window usage (with color coding), git branch and dirty-state indicators, and the active model name — without leaving the terminal.

---

## Prerequisites

- **Claude Code** installed (the `~/.claude/` directory must exist)
- **`jq`** available on your PATH (used to parse Claude's JSON statusline input)
- **bash** (the scripts use bash-specific syntax)
- **git** (optional — git features gracefully degrade if not in a repo)

Install `jq` if needed:

```bash
# macOS
brew install jq

# Ubuntu / Debian
sudo apt-get install jq
```

---

## Installation

```bash
bash ~/install-statuslines.sh
```

The script will:

1. Verify `~/.claude/` exists
2. Write five statusline scripts to `~/.claude/`
3. Write a `switch-statusline.sh` helper to `~/.claude/`
4. Patch `~/.claude/settings.json` to activate `statusline-minimal.sh` (the default)

Restart Claude Code after installation to see the new statusline.

---

## Available Styles

| Style | Script | Bar Width | Shows |
|---|---|---|---|
| `minimal` | `statusline-minimal.sh` | 10 chars | Directory, branch, model, context bar, % used |
| `full` | `statusline-full.sh` | 20 chars | Directory icon, branch + clean/dirty flag, model, context bar, % used |
| `context` | `statusline-context.sh` | 30 chars | Model, wide context bar, % used, token counts (e.g. `42K/200K`) |
| `git` | `statusline-git.sh` | — | Directory icon, branch name, clean (`✓`) or dirty (`✗`) flag — no context bar |
| `session` | `statusline-session.sh` | — | First 7 chars of session ID, full workspace path — no context bar |

**Default after install:** `minimal`

---

## Switching Styles

```bash
~/.claude/switch-statusline.sh [minimal|full|context|git|session]
```

Examples:

```bash
# Switch to the wide context-focused view
~/.claude/switch-statusline.sh context

# Switch to git-only view
~/.claude/switch-statusline.sh git

# Back to the compact default
~/.claude/switch-statusline.sh minimal
```

Restart Claude Code after switching for the change to take effect.

The helper edits `~/.claude/settings.json` in place, setting `statusLine.type = "command"` and pointing `statusLine.command` at the selected script.

---

## What the Statusline Looks Like

### `minimal`
```
excalibur [main] • 🧠 Sonnet 4.5 [████░░░░░░] 40%
```

### `full`
```
📁 excalibur [main ✓] • 🧠 Sonnet 4.5 • [████████░░░░░░░░░░░░] 40%
```

### `context`
```
🧠 Sonnet 4.5 [████████████░░░░░░░░░░░░░░░░░░] 40% (82K/200K)
```

### `git`
```
📁 excalibur [A11Y-1434 ✗]
```

### `session`
```
Session: a3f9c12 • /Users/you/repo/excalibur
```

---

## Color Coding

Context bar color changes based on how much of the context window has been consumed:

| Used | Bar color | Percentage color |
|---|---|---|
| < 50% | Green | Cyan |
| 50–79% | Yellow | Cyan |
| >= 80% | Red | Magenta |

---

## How It Works

Each script receives a JSON blob from Claude Code on stdin. The scripts extract fields using `jq`:

- `workspace.current_dir` / `cwd` — working directory
- `model.display_name` — active model name
- `context_window.remaining_percentage` — context window remaining %
- `context_window.context_window_size` — total token capacity
- `context_window.total_input_tokens` + `total_output_tokens` — tokens used so far
- `session_id` — current session identifier

Git state is read directly via `git -C <dir>` subcommands and is independent of the JSON input.

---

## Customization

**Bar width**: Each style hardcodes a `bar_length` variable. Edit the installed script in `~/.claude/statusline-<style>.sh` to change it.

**Model abbreviation**: The scripts shorten `Claude Sonnet X.Y` to `Sonnet X.Y`. Add additional `elif` branches for custom model names if needed.

**Color thresholds**: The 50 / 80 percent thresholds are plain integers in each script — easy to adjust.

**Adding a new style**: Write a new `statusline-<name>.sh` in `~/.claude/`, make it executable (`chmod +x`), then run:

```bash
~/.claude/switch-statusline.sh <name>
```

Note: `switch-statusline.sh` only accepts the five built-in names. You would need to call `jq` directly on `~/.claude/settings.json` to point at a custom script, or edit `switch-statusline.sh` to add your style to the `case` block.
