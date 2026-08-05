---
assignees:
- claude-code
depends_on:
- 01KZ94F228KKTWT5T9Y59VJJVY
position_column: todo
position_ordinal: ff8880
title: Inverse-pair census probe for completeness
---
A TreeSitterProbe that reports when a diff touches one side of a paired operation and not the other.

- Extract symbol names per module from the parse.
- Pair by naming convention: serialize/deserialize, encode/decode, to_x/from_x, read/write, open/close, save/load, lock/unlock, push/pop. Keep the pair table as data, one list, easy to extend.
- One ProbeRow per broken pair: the touched symbol, its untouched partner, and the convention that paired them.

Wire-up:
- Add the probe to the `completeness` set's `probes:` list.
- Update the inverse-operation-coverage rule to consume the rows: the pairs are found for you; judge whether the partner needed the change.

Acceptance:
- A diff that edits `serialize` and not `deserialize` in the same module yields one row.
- A diff that edits both yields no rows.

#tool-validators