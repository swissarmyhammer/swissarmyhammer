---
position_column: done
position_ordinal: ffffffca80
title: 'Fix mirdan/src/new.rs: replace home_dir with xdg_base_dir for validator/tool'
---
Replace dirs::home_dir() in run_new_validator and run_new_tool with xdg_base_dir using AvpConfig::XDG_NAME. Add swissarmyhammer-directory dep to mirdan/Cargo.toml.