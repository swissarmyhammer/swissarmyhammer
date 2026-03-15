---
position_column: done
position_ordinal: f4
title: 'Fix mirdan-cli test: test_clone_anthropics_plugins_select_nonexistent network failure'
---
The test `git_source::tests::test_clone_anthropics_plugins_select_nonexistent` in mirdan-cli fails with: Git clone failed for 'https://github.com/anthropics/claude-plugins-official.git': unexpected EOF; class=Http (34). This test depends on network access to GitHub and fails when the network request fails.