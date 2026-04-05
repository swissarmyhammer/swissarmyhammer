---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffa580
title: Add test for similarity cosine None branch
---
model-embedding/src/similarity.rs:18\n\nCoverage: 80% (4/5 lines)\n\nUncovered: line 18 — the None arm of f32::cosine(). This may be impossible to trigger with normal inputs (simsimd always returns Some for valid same-length non-empty vectors). If untriggerable, document why.