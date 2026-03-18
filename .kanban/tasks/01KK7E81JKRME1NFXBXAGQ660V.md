---
position_column: done
position_ordinal: fffff880
title: Double-click inspect + unified entity rendering
---
Double-click on any entity (task card, tag pill, actor avatar) should open the inspector, mirroring the existing right-click → inspect behavior.

Also ensure tags and actors render with the SAME component everywhere:
- TagPill for all tag rendering (board cards, inspector, grid cells, markdown)
- MentionPill + Avatar for actor rendering

- [ ] Add `onDoubleClick` handler to FocusScope that executes `entity.inspect`
- [ ] Verify TagPill is used consistently everywhere
- [ ] Add FocusScope + inspect to MentionPill (currently read-only)
- [ ] Add FocusScope + inspect to Avatar/AvatarDisplay
- [ ] Add tests for double-click inspect behavior