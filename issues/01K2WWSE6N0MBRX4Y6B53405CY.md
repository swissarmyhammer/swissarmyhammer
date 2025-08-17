the table output from web-search is ugly -- we don't need teh number of words, text preview, or engine

## Proposed Solution

After analyzing the web-search table output code in `/Users/wballard/github/sah-search/swissarmyhammer-cli/src/web_search.rs`, I can see the current table includes three unwanted elements:

1. **Engine column** - Line 20-21 in `SearchResultRow` struct and populated in lines 255, 264, 283
2. **Word count rows** - Lines 280-286 that add "📄 {word_count} words" rows
3. **Text preview** - The content summary shown in the description of word count rows

**Changes needed:**

1. Remove the `engine` field from `SearchResultRow` struct (line 20-21)
2. Remove engine assignment in main result row creation (line 255)
3. Remove engine assignments in URL and content rows (lines 264, 283) 
4. Remove the entire content info section (lines 268-287) that adds word count and summary rows
5. Update tests to reflect the cleaner output format

This will result in a cleaner table showing only:
- Title (truncated to 60 chars)
- Score 
- Description (truncated to 80 chars)  
- URL as a separate row with link emoji

The table will be more readable and focused on the essential search result information.

## Implementation Complete

Successfully implemented the clean table format for web-search output. The changes made:

1. ✅ **Removed Engine column** - Removed the `engine` field from `SearchResultRow` struct
2. ✅ **Removed engine assignments** - Eliminated all engine value assignments in table row creation
3. ✅ **Removed word count and text preview** - Completely removed the content info section that added "📄 X words" rows and text summaries
4. ✅ **Updated unused variable** - Cleaned up the unused `engine` variable to eliminate compiler warnings

**Results:**

The table now shows a clean, focused format:

```
+------------------------+-------+------------------+
| Title                  | Score | Description      |
+------------------------+-------+------------------+
| Test Title             | 0.95  | Test description |
+------------------------+-------+------------------+
| 🔗 https://example.com |       |                  |
+------------------------+-------+------------------+
```

**Testing:**
- ✅ All existing tests continue to pass
- ✅ Added new test `test_display_search_results_table_clean_format` to validate the clean format
- ✅ cargo fmt and cargo clippy run successfully with no warnings
- ✅ Manual verification shows the clean table output works correctly

The web-search table output is now much cleaner and more readable, focusing on the essential information users need without cluttering the display with engine names, word counts, or text previews.