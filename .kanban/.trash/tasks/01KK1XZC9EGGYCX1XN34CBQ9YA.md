---
position_column: todo
position_ordinal: c0
title: 'Update agent builtin models: upgrade GLM/MiniMax, remove stale models'
---
Housekeeping on the agent (non-embedding) builtin model configs. All agent models use unsloth GGUF repos. No ANE variants needed for agent models.

- [ ] Upgrade `builtin/models/GLM-4.7.yaml` → GLM 5.0 — use `unsloth/GLM-5.0-GGUF` (find exact repo name on HuggingFace, IQ4_NL quant)
- [ ] Upgrade `builtin/models/MiniMax-2.1.yaml` → MiniMax 2.5 — use `unsloth/MiniMax-M2.5-GGUF` (find exact repo name, IQ4_NL quant)
- [ ] Delete `builtin/models/qwen-next.yaml` — redundant with qwen-coder (which already refs Qwen3-Coder-Next)
- [ ] Delete `builtin/models/devstral.yaml`
- [ ] Delete `builtin/models/devstral-small.yaml`
- [ ] Delete `builtin/models/deepseek-terminus.yaml`
- [ ] Verify remaining agent models all use unsloth GGUF repos: `claude-code.yaml`, `qwen-coder.yaml`, `qwen-0.6b-test.yaml`, `GLM-5.0.yaml`, `MiniMax-2.5.yaml`
- [ ] Update any references to deleted models in tests or constants
- [ ] Run tests to verify no breakage

Files: `builtin/models/GLM-4.7.yaml`, `builtin/models/MiniMax-2.1.yaml`, `builtin/models/qwen-next.yaml` (delete), `builtin/models/devstral.yaml` (delete), `builtin/models/devstral-small.yaml` (delete), `builtin/models/deepseek-terminus.yaml` (delete)