---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8c80
title: Review prompt framing eats 70-90% of the agent prompt cap
---
Found while fixing ^tsram0q. `prompt_framing_bytes` measured 360112 bytes on a 9-file change and 469950 bytes on a 15-file change, against an `AGENT_PROMPT_CAP` of 524288. The framing alone takes 69% then 90% of every batch prompt, so file blocks get what little is left.

Two terms make it up:
- the largest validator suffix — every prompt rule body of the biggest validator, authored markdown of unbounded size;
- `render_shared_probe_evidence` over `WorkList::shared_probe_results` — the `<changed-set>` `duplicates` rows, which grew about 18 KB per added file between the two rounds. `project_onto_files` carries this block verbatim into EVERY batch, so each batch pays the whole change's evidence.

Consequences:
- `FleetConfig::file_payload_budget` returns a small number, so batches hold very few files and the run makes many more agent turns than it needs.
- ^tsram0q made the over-cap verdict a constant (`MAX_FILE_BLOCK_BYTES`, half the cap), so a file between the packing budget and the cap now takes a batch of its own. When the framing is this large that one prompt can pass `AGENT_PROMPT_CAP`, and `AgentPool::enqueue` refuses it with `PromptTooLong`.

Work:
- Measure the framing decomposition on a real run of this repo: purpose, shared evidence, and per-validator suffix.
- Decide how to bound each term. Candidates: filter the shared evidence rows to the batch's files, cap the evidence block, or charge the focus-file list per file instead of as run framing.
- Prove a batch prompt stays inside `AGENT_PROMPT_CAP` even when one file fills the per-file cap.

#tool-validators
