---
position_column: done
position_ordinal: ffffffe780
title: 'Fix failing test: mcp::tools::files::write::tests::test_write_relative_path_acceptance'
---
Test in swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs:379 panics with 'No such file or directory' error. The test calls unwrap() on a Result::Err(Os { code: 2, kind: NotFound }). Package: swissarmyhammer-tools. Rerun with: cargo test -p swissarmyhammer-tools --lib test_write_relative_path_acceptance #test-failure