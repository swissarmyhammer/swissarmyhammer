---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8f80
title: Resolve the project validator directory from the session working dir, not the process CWD
---
Found while doing ^3hwy2pd.

`ValidatorLoader::load_all` (and `validator_directories` / the loader diagnostics) resolve the PROJECT validator layer with `ManagedDirectory::<ValidatorsConfig>::from_git_root()`, which reads the process current directory. `load_rules()` calls it, and every `review` op calls `load_rules()`.

So the `list/dump/get/check validators` ops and `match_rules` load `<cwd git root>/.validators`, not `<session working dir git root>/.validators`. A server whose process CWD differs from the session working dir loads the wrong project layer, or none.

^3hwy2pd threaded a workspace root into the same ops for PROJECT TYPE resolution. This card threads the same root into the RuleSet LOAD.

Work:
- Give `load_rules` a workspace root parameter and pass it down to the project-layer directory resolution.
- Keep the current behavior when no root is available.
- Update every caller: the `review` tool ops, `match_rules`, the doctor surface.

Acceptance:
- A `list validators` call whose session working dir names project A returns project A's validators while the process CWD sits in project B.
- No production path calls `std::env::current_dir()` to find `.validators`.

#tool-validators