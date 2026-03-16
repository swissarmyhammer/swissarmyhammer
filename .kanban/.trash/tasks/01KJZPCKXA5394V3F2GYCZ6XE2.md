---
position_column: done
position_ordinal: n0
title: 'ane-embedding: missing serde_json dev-dependency causes test compilation failure'
---
ane-embedding/src/types.rs:73-74 uses serde_json in a test but serde_json is not declared as a dev-dependency in ane-embedding/Cargo.toml. This blocks all workspace tests from compiling. Fix: add serde_json as a dev-dependency to ane-embedding/Cargo.toml. #test-failure