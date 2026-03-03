---
position_column: done
position_ordinal: h7
title: Fix unused variable warnings in e2e hook test files
---
Fixed 4 unused variable/constant warnings:\n1. cross_cutting_tests.rs: removed unused `short` variable\n2. exit2_tests.rs: removed unused `CHANNEL_TIMEOUT` constant\n3. json_continue_tests.rs: removed unused `CHANNEL_TIMEOUT` constant\n4. json_specific_output_tests.rs: removed unused `CHANNEL_TIMEOUT` constant