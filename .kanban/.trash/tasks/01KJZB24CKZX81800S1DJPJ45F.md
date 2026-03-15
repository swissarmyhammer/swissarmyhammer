---
position_column: done
position_ordinal: j2
title: 'Fix failing test: test_load_with_env_vars in swissarmyhammer-config'
---
The test template_context::tests::test_load_with_env_vars in swissarmyhammer-config panics at swissarmyhammer-config/src/template_context.rs:993:55 with: called Result::unwrap() on an Err value: LoadError. The error is 'No such file or directory (os error 2)' when trying to load a TOML config file from a temp directory. The test creates a temp dir but likely has a race condition or missing file setup where the config file path references a non-existent include or nested file. #test-failure