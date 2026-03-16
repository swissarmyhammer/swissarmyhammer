---
position_column: done
position_ordinal: i9
title: 'Fix model-loader compile error: no field `retry_config` on ModelResolver'
---
model-loader/src/loader.rs:324 references `resolver.retry_config.max_retries` but ModelResolver has no `retry_config` field. This prevents the entire model-loader crate from compiling tests. #test-failure