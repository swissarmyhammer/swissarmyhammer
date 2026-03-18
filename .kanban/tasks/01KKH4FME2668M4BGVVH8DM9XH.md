---
position_column: done
position_ordinal: ffffffff8380
title: 'mirdan-cli: 4 git_source tests fail with network timeout (socket Operation timed out)'
---
4 tests in mirdan-cli/src/git_source.rs fail with Git clone socket timeout errors. These tests clone from remote GitHub repos (anthropics/skills, anthropics/claude-plugins-official, basecamp/skills, obra/superpowers) and fail when network is unavailable/slow. Tests: test_clone_basecamp_skills_discovers_packages, test_clone_anthropics_plugins_select_one, test_clone_obra_superpowers_discovers_plugin, test_clone_anthropics_skills_https_url