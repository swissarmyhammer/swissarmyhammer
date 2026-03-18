---
position_column: done
position_ordinal: ffffffff8580
title: 'mirdan-cli: 18 tests fail with "No such file or directory" (install, list, new modules)'
---
18 tests in mirdan-cli fail with Os { code: 2, kind: NotFound, message: "No such file or directory" }. Affected modules: install (12 tests), list (2 tests), new (4 tests). All unwrap() on file system operations. Files: mirdan-cli/src/install.rs, mirdan-cli/src/list.rs, mirdan-cli/src/new.rs