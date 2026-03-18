---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffe480
title: 'nit: AvpHooks doc comment has a missing leading slash on the continuation line'
---
swissarmyhammer-cli/src/commands/install/components/mod.rs:1226-1228\n\nThe doc comment for `AvpHooks` reads:\n\n```\n/// Installs/removes AVP (Agent Validator Protocol) hooks in Claude Code settings.\n///\n/ Delegates to `avp_common::install` — the same logic used by `avp init`.\n```\n\nThe third line uses a single `/` instead of `///`. This means the last sentence is not rendered as a doc comment — it becomes a plain line comment and is invisible in `cargo doc` output.\n\nSuggestion: Change `/ Delegates to` to `/// Delegates to`.\n\nVerification: `cargo doc -p swissarmyhammer-cli` renders the full doc comment for `AvpHooks`." #review-finding