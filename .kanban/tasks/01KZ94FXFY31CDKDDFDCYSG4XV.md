---
assignees:
- claude-code
depends_on:
- 01KZ94F228KKTWT5T9Y59VJJVY
position_column: todo
position_ordinal: ff8980
title: Assertion census probe for test-integrity
---
A TreeSitterProbe that measures test bodies.

- Identify test functions structurally: attribute, decorator, or framework naming convention at the definition — never the file name.
- Per test function, count calls that match the framework's assertion set. Keep the assertion sets as data per language/framework, one table.
- Also report: skip/ignore markers, empty bodies, bodies that are only comments, and catch/except blocks that swallow without asserting.
- One ProbeRow per suspect test: location, what was measured (0 assertions, skipped, empty, swallowed).

Wire-up:
- Add the probe to the `test-integrity` set's `probes:` list.
- Update the no-test-cheating rule to consume the rows as computed facts, same style as cognitive-complexity: do not recount.

Acceptance:
- A fixture test with zero assertions yields a row; a normal asserting test yields none.
- A `#[ignore]`/skip-marked test yields a row.

#tool-validators