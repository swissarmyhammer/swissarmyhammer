---
position_column: done
position_ordinal: n3
title: 'Fix test_load_model: embedding_dimension() returns None after load()'
---
In /Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/ane-embedding/tests/integration_test.rs line 23, assert_eq!(model.embedding_dimension(), Some(384)) fails because embedding_dimension() returns None even after a successful load(). The model loads successfully and is_loaded() returns true, but the dimension metadata is not being set. #test-failure