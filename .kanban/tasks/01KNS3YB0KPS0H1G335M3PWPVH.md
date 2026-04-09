---
assignees:
- claude-code
depends_on:
- 01KNS1TQR2C3TYG1G8STEYZPA5
position_column: done
position_ordinal: ffffffffffffffffffff8d80
project: code-context-cli
title: Add code-context-cli entry to doc/src/SUMMARY.md
---
## What
Add the code-context CLI reference entry to `doc/src/SUMMARY.md` so it appears in the mdbook documentation site.

The current reference section in `doc/src/SUMMARY.md` reads:
```
- [SwissArmyHammer CLI](reference/sah-cli.md)
- [AVP CLI](reference/avp-cli.md)
- [Mirdan CLI](reference/mirdan-cli.md)
- [ShellTool CLI](reference/shelltool-cli.md)
```

Add after the ShellTool entry:
```
- [Code-Context CLI](reference/code-context-cli.md)
```

The file `doc/src/reference/code-context-cli.md` is **generated at build time** by `code-context-cli/build.rs` via `doc_gen::generate_markdown_with_brew()` — it does not need to be created manually.

## Acceptance Criteria
- [ ] `doc/src/SUMMARY.md` contains `[Code-Context CLI](reference/code-context-cli.md)`
- [ ] After `cargo build -p code-context-cli`, `doc/src/reference/code-context-cli.md` exists

## Tests
- [ ] `grep "code-context-cli" doc/src/SUMMARY.md` exits 0

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.