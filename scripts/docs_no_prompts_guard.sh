#!/usr/bin/env bash
# Docs guard: the "prompts" feature has been removed from SwissArmyHammer, so it
# must not be marketed or documented as a user-facing capability anymore.
#
# This guard FAILS if any user-facing documentation source still references the
# removed `sah prompt` command, a `prompts/` / `.prompts/` content directory, or
# the old "prompt management/system" feature framing.
#
# Legitimate LLM-prompting wording is intentionally NOT matched here: phrases
# like "system prompt", "agent prompt", "expands into a prompt", the Claude
# `UserPromptSubmit` hook event, and the llama `prompt` hook handler all describe
# how agents are prompted and must be preserved. The patterns below target only
# the sah-prompts *feature*, not LLM prompting in general.
#
# Usage: scripts/docs_no_prompts_guard.sh
# Exit 0 when clean, 1 (with a report) when a forbidden reference is found.

set -euo pipefail

cd "$(dirname "$0")/.."

# Files in scope: the README, the mdbook sources, the man pages, and book.toml.
scope=(README.md doc/book.toml)
while IFS= read -r f; do scope+=("$f"); done < <(find doc/src -type f -name '*.md')
while IFS= read -r f; do scope+=("$f"); done < <(find docs -type f -name '*.1')

# Forbidden patterns describing the removed sah-prompts feature.
patterns=(
  'sah prompt'          # the removed CLI command
  'prompt list'         # removed subcommand
  'prompt test'         # removed subcommand
  '\.prompts/'          # the removed content directory
  'prompts/ *#'         # a "prompts/  # ..." directory listing entry
  'prompt management'   # old feature framing
  'AI prompt'           # old marketing framing ("AI prompt management")
  'Prompt System'       # doctor/validate feature section
  'Prompt files'        # validate feature wording
  'Prompt File'         # validate feature wording
  'built-?in prompts'   # "Built-in prompts (embedded in binary)"
  'user prompts'        # "User prompts (~/.prompts/)"
  'project prompts'     # "Project prompts (./.prompts/)"
  'Validate prompt'     # man-page / validate description
  'validate prompt'
)

found=0
for pat in "${patterns[@]}"; do
  if hits=$(grep -rinE "$pat" "${scope[@]}" 2>/dev/null); then
    echo "FORBIDDEN prompt-feature reference matching /$pat/:"
    echo "$hits"
    echo
    found=1
  fi
done

if [ "$found" -ne 0 ]; then
  echo "docs guard FAILED: the prompts feature must not appear in user-facing docs." >&2
  exit 1
fi

echo "docs guard OK: no sah-prompts feature references in user-facing docs."
