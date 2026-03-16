---
position_column: todo
position_ordinal: c6
title: 'ane-embedding tests SIGABRT/SIGTRAP: Swift runtime conflict on macOS'
---
All 9 ane-embedding tests (1 coreml_test + 8 integration_test) crash with SIGABRT or SIGTRAP due to duplicate Swift Concurrency classes loaded from both /usr/lib/swift/libswift_Concurrency.dylib and the Xcode toolchain copy. This is a known pre-existing macOS environment issue, not a regression from the tools branch. Fixing requires resolving the Swift runtime linkage conflict (e.g. ensuring only one copy of libswift_Concurrency.dylib is linked).