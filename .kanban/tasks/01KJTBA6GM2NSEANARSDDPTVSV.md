---
position_column: todo
position_ordinal: e3
title: 'Fix test_root_validate_undefined_variables: exit code 1 instead of expected 2'
---
Test `integration::cli_integration::test_root_validate_undefined_variables` in swissarmyhammer-cli fails.\n\nLocation: /Users/wballard/github/swissarmyhammer/swissarmyhammer-cli/tests/integration/cli_integration.rs:813\n\nAssertion: `left == right` failed: validation with undefined variables should return exit code 2\n  left: 1\n  right: 2\n\nThe CLI returns exit code 1 when the test expects exit code 2 for undefined variable validation. #test-failure