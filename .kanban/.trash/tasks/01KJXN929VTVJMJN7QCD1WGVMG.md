---
position_column: done
position_ordinal: g8
title: Fix failing doctest in avp-common/src/validator/parser.rs (parse_ruleset_directory line 369)
---
The doctest at line 369 in avp-common/src/validator/parser.rs contains Unicode box-drawing characters (└── ├── etc.) that the Rust compiler tries to parse as code. These characters cause 16 compilation errors. The doc example showing a directory tree structure needs to be wrapped in a code fence (```text ... ```) or otherwise excluded from doctest compilation (e.g., ```no_run or ```ignore).