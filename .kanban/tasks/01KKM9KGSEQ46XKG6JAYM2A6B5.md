---
position_column: done
position_ordinal: ffffc880
title: 'Fix agent_resolver.rs: replace home_dir with xdg_base_dir'
---
Replace dirs::home_dir() calls with swissarmyhammer_directory::xdg_base_dir in load_from_user_paths. Use SwissarmyhammerConfig::XDG_NAME instead of hardcoded "sah".