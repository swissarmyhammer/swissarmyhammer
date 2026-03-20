---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffb280
title: 'nit: avp-cli/src/install.rs test only checks From conversion, not the wrappers themselves'
---
avp-cli/src/install.rs:41-51\n\nThe test module in the thinned-down `avp-cli/src/install.rs` contains a single test (`test_install_target_to_init_scope`) that checks the `From<InstallTarget>` mapping. The actual `install` and `uninstall` wrapper functions — which call `std::env::current_dir()` and delegate to `avp_common` — have no tests at all in this file.\n\nThis is acceptable given that the real logic is tested in `avp-common`, but the `current_dir()` call in both wrappers is an I/O operation that could fail and is not covered. If this is intentional (i.e., the avp-cli wrappers are considered thin enough to not warrant separate tests), a comment to that effect would prevent future contributors from adding redundant tests.\n\nSuggestion: Either add a brief test that confirms `install(InstallTarget::Project)` delegates correctly (using a temp dir), or add a `// Tests live in avp-common/src/install.rs` comment in the test module.\n\nVerification: `cargo nextest run -E 'package(avp-cli)'` passes."