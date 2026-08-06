---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8e80
title: Review batch budget shrinks as the diff grows — over-cap splits cannot converge
---
Found while watching ^cbnfe97. The per-batch prompt budget is not stable between review runs on the same code:

- Round 1 (checkpoint 716951039, 9-file diff): batch budget 164176 bytes. 4 files over cap.
- Round 2 (checkpoint 503b74346, 15-file diff): batch budget 54338 bytes — one third of round 1. 13 files over cap, INCLUDING files the round-1 fix just split (fleet.rs 82→52KB source, scope.rs 177→49KB source) and files untouched by the diff logic (doctor.rs).

The feedback loop: an over-cap finding says "split the file" (by design — c45ba2d40 made over-cap a confirmed finding). Splitting adds files to the next diff. A bigger diff shrinks the per-batch budget. A smaller budget puts MORE files over cap. Each fix round makes the next round worse. This cannot converge.

Work:
- Find where the batch budget is computed (batch_work_list / cost math, now in review/scope/batch.rs) and confirm the budget depends on diff size or batch count.
- Make the over-cap threshold stable per file: a constant cap, independent of how many files the diff carries.
- An over-cap verdict must be reproducible: the same file content gets the same verdict on every run.

Acceptance:
- Two consecutive review runs over the same content report the same set of over-cap files.
- A file that satisfied the cap in run N cannot be over cap in run N+1 without growing.

#tool-validators