---
assignees:
- claude-code
position_column: todo
position_ordinal: ffe580
title: dead-code-typescript states the 0.55 s placement cost without saying it is under a measured spread
---
`builtin/validators/code-hygiene/rules/dead-code-typescript.md` holds a timing table with three placements, three readings each, a lowest and a spread:

| placement | the three readings | lowest | spread |
|---|---|---|---|
| the shell placement this replaced | 6.31 s, 6.72 s, 6.76 s | 6.31 s | 0.45 s |
| the file-list placement | 6.86 s, 6.96 s, 8.18 s | 6.86 s | 1.32 s |
| the same, reading the real path of each listed file as well | 7.17 s, 7.37 s, 7.85 s | 7.17 s | 0.68 s |

Every number of the table is correct. 6.76 - 6.31 = 0.45. 8.18 - 6.86 = 1.32. 7.85 - 7.17 = 0.68. 6.86 - 6.31 = 0.55. 7.17 - 6.86 = 0.31.

The paragraph under the table treats the two deltas differently. It states the 0.31 s and then says it is "under each of the three spreads above, so this measurement does not tell that cost from noise either". It states the 0.55 s as "what the WHOLE placement costs over the shell loop it replaced", and says only that the measurement does not divide it between the two added calls.

The 0.55 s is under the spread of the second row, 1.32 s. So the table does not tell the 0.55 s from noise either, and the text does not say so. A reader takes the 0.55 s as a measured cost and the 0.31 s as noise, and the three readings carry no such difference.

Say of the 0.55 s what the file already says of the 0.31 s: name the spread it stands under, or take more readings until the delta stands above every spread.

This is prose. The shipped script is unaffected. Raised by the round-7 review of ^yxky1aj and filed apart, because the card it was found on carries no behavior defect.

#tool-validators