---
position_column: done
position_ordinal: q5
title: '[WARNING] md5 crate used for content hashing -- weak and slow'
---
File: swissarmyhammer-code-context/Cargo.toml, line 16\n\nThe md5 crate is listed as a dependency. MD5 is cryptographically broken and slower than modern alternatives. The workspace already depends on xxhash-rust (via sem-core) and sha2.\n\nFor content hashing of files (detecting changes), xxhash or blake3 would be faster and equally suitable. If collision resistance matters, sha2 is already available. md5 is not appropriate for either use case in 2026.\n\nNote: md5 is declared as a dep but not yet used in the source files read -- this may be a premature dependency or planned for future use. #review-finding