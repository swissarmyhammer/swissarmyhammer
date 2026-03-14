---
position_column: done
position_ordinal: z00
title: 'nextest.toml treesitter-embedding group: filter too narrow (single test name)'
---
.config/nextest.toml:38\n\nThe `treesitter-embedding` test group is applied via:\n```toml\nfilter = \"test(test_find_all_duplicates_detects_near_identical_functions)\"\n```\nThis hardcodes a single test function name. If future embedding-heavy tests are added to the treesitter crate with different names, they will not pick up the serialisation constraint. The llama-embedding group correctly uses a package filter (`package(llama-embedding)`) which is robust to test name changes.\n\nSuggestion: Use a package-scoped filter instead of a bare test name:\n```toml\nfilter = \"package(swissarmyhammer-treesitter) and test(embedding)\"\n```\nor a more targeted package filter if the entire package should be serialised. At minimum document why a name-based filter is intentional here.",
<parameter name="tags">["review-finding"] #review-finding