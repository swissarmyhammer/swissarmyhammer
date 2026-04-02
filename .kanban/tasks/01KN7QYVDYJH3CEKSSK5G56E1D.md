---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffa280
title: Add tests for error.rs text_encoding/configuration constructors
---
model-embedding/src/error.rs:59-64\n\nCoverage: 60% (6/10 lines)\n\nUncovered: text_encoding() and configuration() convenience constructors\n\nAdd test cases calling these two constructors and verifying Display output.