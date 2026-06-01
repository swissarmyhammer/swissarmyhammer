---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffcf80
title: Consolidate qwen + qwen-moe onto Qwen3.6-35B-A3B-MTP and enable MTP
---
SUPERSEDED. The original umbrella card from before implementation started. The actual delivery is in card 01KSTCQT823WN3W7SC1S6W05TQ (now done).

Net outcome:
- qwen is on the MTP GGUF and runs draft-mtp speculative decoding through llama-agent/ACP end-to-end (auto-detected via model.has_mtp()).
- Verified on Metal against both the small Qwen3.5-0.8B-MTP and the full Qwen3.6-35B-A3B-MTP via cargo run --example mtp_smoke (keystone commit 3e959733c).
- Fork edits and consumer commits are local; push order documented on 01KSTCQT823WN3W7SC1S6W05TQ.