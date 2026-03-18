---
position_column: done
position_ordinal: ffffffdb80
title: 'nit: from_user_home deprecation note should mention the preferred replacement per use-case'
---
`swissarmyhammer-directory/src/directory.rs:157`\n\nThe deprecation note says:\n```rust\n#[deprecated(note = \"Use xdg_config(), xdg_data(), or xdg_cache() instead\")]\n```\n\nThis is helpful but leaves users to guess which XDG directory to use. Config, data, and cache have different semantics:\n- Config: files the user edits (settings, prompts) → `xdg_config()`\n- Data: persistent app data (validators, agents, embeddings) → `xdg_data()`\n- Cache: reproducible derived content → `xdg_cache()`\n\nSuggestion: Expand the deprecation note or the doc comment to clarify:\n```\n#[deprecated(note = \"Use xdg_data() for user data (validators, agents), xdg_config() for settings, or xdg_cache() for derived content\")]\n``` #review-finding