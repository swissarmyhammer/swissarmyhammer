---
assignees:
- claude-code
position_column: todo
position_ordinal: af80
title: 'cargo nextest: mirdan git_source clone tests flake under workspace-wide parallel load (9 failures)'
---
These tests pass individually and pass in isolation (mirdan-only run, even in parallel) but fail when run as part of `cargo nextest run --workspace` (~13551 tests, ~5min duration):

- mirdan git_source::tests::test_clone_anthropics_plugins_discovers_multiple_plugins
- mirdan git_source::tests::test_clone_anthropics_plugins_select_one
- mirdan git_source::tests::test_clone_anthropics_skills_https_url
- mirdan git_source::tests::test_clone_anthropics_skills_frontmatter_is_valid
- mirdan git_source::tests::test_clone_anthropics_skills_shorthand
- mirdan git_source::tests::test_clone_anthropics_plugins_select_nonexistent
- mirdan git_source::tests::test_clone_basecamp_skills_discovers_packages
- mirdan git_source::tests::test_clone_anthropics_skills_select_nonexistent
- mirdan git_source::tests::test_clone_anthropics_skills_select_one

Each takes 65–76s in the failing run vs. ~2-10s when run alone. They do real `git clone` of public repos (anthropics/skills, anthropics/plugins, basecamphq, obra/superpowers).

Likely cause: network/IO contention or git2 file-lock contention when 8+ clone tests race against each other while the rest of the workspace is also hitting disk hard. Per the test skill troubleshooting "tests pass locally but fail with ... when run in parallel" — apply `#[serial_test::serial]` to the `test_clone_*` tests in mirdan/src/git_source.rs (they likely already share `serial_test`'s tempdir; check whether the serial group covers all clone tests or only a subset).

Reproducer:
  cargo nextest run --workspace
Then re-run only mirdan to confirm green:
  cargo nextest run --package mirdan git_source::tests::

NOT caused by branch kanban / 01KQAXPRTCNH8ARTYJJEBTYWW0 (perspective-tab-bar UI change). #test-failure