---
position_column: done
position_ordinal: i7
title: 'Fix clippy error: dead_code field `retry_config` in model-loader/src/loader.rs'
---
cargo clippy --workspace -- -D warnings fails because field `retry_config` on `ModelResolver` (model-loader/src/loader.rs:16) is never read. Either use the field, remove it, or add #[allow(dead_code)]. #test-failure