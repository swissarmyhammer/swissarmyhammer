---
position_column: done
position_ordinal: u7
title: 'Fix failing test: integration::cli_integration::test_root_validate_undefined_variables'
---
Test in swissarmyhammer-cli/tests/integration/cli_integration.rs:356 fails with assertion: validation with undefined variables should return exit code 2 but got exit code 0 (left: 0, right: 2). The validate command is not detecting undefined variables as expected. #test-failure