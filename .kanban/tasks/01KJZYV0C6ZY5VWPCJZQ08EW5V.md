---
position_column: done
position_ordinal: fffe80
title: 'WARNING: Changelog written before mutation in view CRUD commands'
---
commands.rs:704,734,759\n\nIn view.create, view.update, view.delete arms, changelog is appended before the write/delete. If mutation fails, phantom entry exists.\n\nFix: mutate first, log second.