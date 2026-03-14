---
position_column: done
position_ordinal: ffff8d80
title: Hardcoded mention boundary chars may miss Unicode
---
**W2: Mention boundary detection uses hardcoded ASCII set**

`mention-finder.ts` uses a fixed set of boundary characters. Mentions adjacent to Unicode punctuation (e.g., CJK brackets, em-dashes) won't be detected.

**Fix:** Consider using a Unicode-aware word boundary regex or `\b` equivalent for mention detection boundaries.