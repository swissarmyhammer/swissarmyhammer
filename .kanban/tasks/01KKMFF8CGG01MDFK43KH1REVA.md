---
position_column: done
position_ordinal: z00
title: 'Add #[serial] to all XDG tests that mutate environment variables'
---
Add serial_test as dev-dependency in swissarmyhammer-directory/Cargo.toml, add use serial_test::serial import, add #[serial] to every test that calls set_var or remove_var, and remove dead code at lines 527-530.