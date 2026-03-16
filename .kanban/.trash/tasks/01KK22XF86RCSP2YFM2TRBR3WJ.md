---
position_column: todo
position_ordinal: c5
title: 'Workspace fails to load: missing coreml-rs dependency for ane-embedding'
---
The entire workspace at /Users/wballard/github/swissarmyhammer/swissarmyhammer-tools cannot compile or run tests because ane-embedding depends on coreml-rs via a path dependency at /Users/wballard/github/swissarmyhammer/coreml-rs/Cargo.toml, which does not exist. This blocks all cargo test, cargo clippy, and cargo build commands for the entire workspace. Either clone/create the coreml-rs repo at the expected path, or make it an optional/conditional dependency. #test-failure