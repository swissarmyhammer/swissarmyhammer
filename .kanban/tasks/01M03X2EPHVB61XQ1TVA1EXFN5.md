---
assignees:
- claude-code
position_column: todo
position_ordinal: ffe780
title: Stray .kanban board directory sits under crates/swissarmyhammer-mcp-proxy
---
`crates/swissarmyhammer-mcp-proxy/.kanban` is an untracked board directory dated 5 July. It predates the current work by weeks, and no `.gitignore` entry covers it.

A stray board directory is a hazard: the live application opens a board directory it finds, and it then writes to it. A board opened by mistake reads as empty.

## What to do

- Find what wrote it. An agent or a test that ran with the wrong current directory is the usual cause.
- Remove the directory, or make the cause write to the repository root board.
- Sweep the tree for other stray dot directories of the same shape.

## Found by

The implementer of ^s1qh4tv, twice, while it checked its own working tree for probe files. It is out of scope for that card.

#bug