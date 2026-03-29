---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffba80
title: 'STATUSLINE-M2: model module'
---
## What\nModule: `model`. Source: `input.model.display_name` / `input.model.id`. Default format: `🧠 $name`.\n\nShortens model names: `claude-opus-4-6` → `Opus 4.6`, `claude-sonnet-4-6` → `Sonnet 4.6`, etc. Falls back to display_name if id pattern doesn't match.\n\nFile: `swissarmyhammer-statusline/src/modules/model.rs`\n\n## Acceptance Criteria\n- [ ] Default output includes 🧠 emoji\n- [ ] Known model IDs shortened correctly\n- [ ] Unknown models fall back to display_name\n- [ ] Configurable format and style\n- [ ] Returns None if no model info\n\n## Tests\n- [ ] Test opus, sonnet, haiku ID shortening\n- [ ] Test unknown model falls back to display_name\n- [ ] Test custom format overrides default\n- [ ] Test absent model returns None