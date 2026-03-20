---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffb080
title: 'nit: avp_hooks_config, is_avp_hook, merge_hooks, remove_hooks are pub but have no doc examples'
---
avp-common/src/install.rs:24, 71, 85, 113\n\nAll four helper functions are now `pub` in the shared library crate. Per the Rust guidelines, all public items need doc comments, and examples should use `?` not `.unwrap()`. The existing doc comments are single-line descriptions only — they are missing:\n- Parameter documentation (what `avp_hooks` should contain in `merge_hooks`)\n- Return value documentation for `is_avp_hook` (what constitutes an AVP hook per the detection logic)\n- A note on the idempotency guarantee of `merge_hooks`\n- An explanation in `remove_hooks` that it does not remove empty `hooks` keys (callers must do that themselves, as `uninstall` does at line 214-218)\n\nThe `AVP_README` constant is also `pub` but undocumented.\n\nSuggestion: Expand the doc comments with `# Arguments`, `# Returns`, and where relevant `# Examples` sections.\n\nVerification: `cargo doc -p avp-common --no-deps` produces no warnings." #review-finding