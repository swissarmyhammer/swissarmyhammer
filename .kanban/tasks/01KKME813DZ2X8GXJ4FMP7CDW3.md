---
position_column: done
position_ordinal: ffffffab80
title: 'warning: lib.rs doc example omits XDG_NAME from DirectoryConfig impl'
---
`swissarmyhammer-directory/src/lib.rs:43-59`\n\nThe crate-level doc example for implementing `DirectoryConfig` is:\n```rust\nimpl DirectoryConfig for MyToolConfig {\n    const DIR_NAME: &'static str = \".mytool\";\n    const GITIGNORE_CONTENT: &'static str = \"*.log\\ntmp/\\n\";\n    ...\n}\n```\n\nIt is missing `const XDG_NAME: &'static str = \"mytool\";`, which is now a required associated constant of the trait. This will cause a compile error for any user that copies the example verbatim.\n\nSuggestion: Add `const XDG_NAME: &'static str = \"mytool\";` to the example in the doc comment. #review-finding