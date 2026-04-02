---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffee80
title: Test ProjectSymbols::get() for all project types
---
File: swissarmyhammer-project-detection/src/types.rs (50.0%, 15 uncovered lines)\n\nUncovered:\n- `ProjectSymbols::default()` (lines 88-92): loading from builtin YAML config\n- `ProjectSymbols::get()` (lines 97-109): all match arms for each ProjectType variant (Rust, NodeJs, Python, Go, JavaMaven, JavaGradle, CSharp, CMake, Makefile, Flutter, Php)\n\nTests needed:\n- Test ProjectSymbols::default() loads successfully\n- Test get() returns non-empty string for each ProjectType variant\n\nAcceptance: coverage >= 80% for types.rs #coverage-gap