---
position_column: done
position_ordinal: ffffffffffa980
title: 'NIT: README command table lists /test twice under different descriptions after the change'
---
README.md:75-80\n\nBefore this change, the command block listed `/test` with the description \"Run tests, fix failures\". After the change it is listed as \"Run tests, report failures as cards\". This is more accurate. However, in the table at lines 98-107, `/test` is described as \"Run the full suite, analyze failures, fix them\" which is now inconsistent with the updated description in the command block above.\n\nNot introduced by this PR (the table was pre-existing), but the change made the inconsistency more visible by updating one description and not the other.\n\nSuggestion: Update the table row for /test (line 102) to match: \"Run the full suite, report failures as kanban cards\"."