---
position_column: done
position_ordinal: ffffb880
title: dispatch_command does not validate command ID format before registry lookup
---
swissarmyhammer-kanban-app/src/commands.rs:477-507\n\nThe `cmd` parameter comes directly from the frontend as an untrusted string. While there is no injection risk (it is used as a HashMap key lookup), there is no validation that the command ID matches the expected dotted format (e.g. 'task.add'). Arbitrary strings like empty strings or very long strings are accepted and simply produce 'Unknown command' errors.\n\nThis is defense-in-depth -- the current behavior is safe (HashMap::get returns None), but adding a quick format check would produce better error messages and prevent accidental misuse.\n\nSuggestion: Consider a lightweight validation like checking for non-empty, reasonable length, and ASCII-only characters. Low priority since there is no actual vulnerability. #review-finding #warning