---
position_column: done
position_ordinal: fa80
title: 'CI: install rust-analyzer in test pipeline'
---
## What\nThe CI pipeline (`/.github/workflows/ci.yml`) runs on self-hosted runners with no rust-analyzer installed. LSP integration tests skip silently when it's missing. Add `rustup component add rust-analyzer` to the CI setup so LSP tests actually run.\n\n## Acceptance Criteria\n- [ ] CI installs rust-analyzer before running tests\n- [ ] LSP tests that previously skipped now execute in CI\n- [ ] CI still passes (no new failures from LSP tests)\n\n## Tests\n- [ ] `cargo nextest run` in CI includes LSP test results (not skipped)\n- [ ] `test_lsp_server_startup` runs and passes in CI"