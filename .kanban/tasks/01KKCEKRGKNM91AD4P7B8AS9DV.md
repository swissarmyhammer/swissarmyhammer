---
position_column: done
position_ordinal: ffa780
title: 'Type error: generate_ts_call_edges and write_ts_edges expect &Connection but receive DbRef in unified.rs'
---
cargo check --workspace fails with 2 errors in swissarmyhammer-treesitter/src/unified.rs (lines 540 and 550). Both generate_ts_call_edges() and write_ts_edges() expect &Connection but cc_conn is a DbRef. Fix: add & before cc_conn at both call sites. #test-failure