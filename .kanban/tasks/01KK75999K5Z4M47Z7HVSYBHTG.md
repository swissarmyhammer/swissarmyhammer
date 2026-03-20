---
position_column: done
position_ordinal: fffff880
title: Multi-select dropdown always visible instead of popover
---
**W5: Multi-select editor dropdown always visible**

The CM6 multi-select editor for reference fields (assignees, depends_on) renders the dropdown inline and always visible rather than as a popover triggered by clicking the cell display.

**Fix:** Wrap the CM6 editor in a Popover component, triggered by clicking the cell's avatar/pill display. The editor should auto-focus on open and close on blur or Escape.