---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffdf80
title: 'BLOCKER: encode_diff_filename / decode_diff_filename is not reversible for paths containing double underscores'
---
avp-common/src/turn/state.rs:256-267\n\nThe encoding replaces `/` with `__` and the decoding replaces `__` back to `/`. This is not a bijection: a file path containing literal `__` (e.g. `/src/__init__.py` or `/src/my__module.rs`) will be corrupted on decode.\n\nExample: `/src/__init__.py` -> encode -> `__src____init__.py.diff` -> decode -> `/src//init/.py` (wrong)\n\nThis is a data corruption bug that silently produces wrong diff lookups for affected paths.\n\nSuggestion: Use percent-encoding (URL-encode `/` as `%2F` and `%` as `%25`) or hex encoding for the separator. Alternatively, use a scheme like `_SLASH_` that is unambiguous, though percent-encoding is the standard approach." #review-finding