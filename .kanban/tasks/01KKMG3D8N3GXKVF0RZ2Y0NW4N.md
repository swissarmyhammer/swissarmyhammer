---
position_column: done
position_ordinal: ffffffb680
title: Fix flaky test test_lsp_reindexing_after_file_change
---
Race condition in swissarmyhammer-code-context test. Need to find timing-sensitive assertions and add retry loop with timeout.