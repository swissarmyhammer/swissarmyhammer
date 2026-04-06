---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff080
title: Test skill_library and skill_resolver — 58-68% coverage
---
Files:\n- swissarmyhammer-skills/src/skill_library.rs: 14/24 (58.3%) — load_with_resolver, len, names untested\n- swissarmyhammer-skills/src/skill_resolver.rs: 60/88 (68.2%) — Default impl and some resolver paths untested\n- swissarmyhammer-skills/src/parse.rs: 24/35 (68.6%) — partial parse coverage\n- swissarmyhammer-skills/src/skill.rs: 14/20 (70%) — Display for SkillName untested\n\nNeed tests for library loading, skill listing/counting, and resolver edge cases.\n\n#coverage-gap #coverage-gap