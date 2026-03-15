---
position_column: done
position_ordinal: n1
title: 'ane-embedding: clippy ptr_arg warning - use &Path instead of &PathBuf in load_tokenizer'
---
ane-embedding/src/model.rs:276 - clippy::ptr_arg: function load_tokenizer takes &PathBuf but should take &Path. Fix: change parameter type from &PathBuf to &Path. #test-failure