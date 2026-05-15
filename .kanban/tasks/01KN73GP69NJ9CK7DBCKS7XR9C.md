---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa280
title: Add tests for ModelManager directory loading (process_directory_entries, extract_model_name)
---
swissarmyhammer-config/src/model.rs:1110-1340\n\nUncovered lines: 1120-1121, 1152-1153, 1160, 1162, 1166, 1176, 1186, 1198-1200, 1213, 1223, 1231-1233, 1245, 1256, 1265, 1267, 1270, 1289-1292, 1331-1332, 1339-1340\n\n```rust\nfn validate_directory_path(dir_path: &Path) -> Result<PathBuf, ModelError>\nfn process_directory_entries(...) -> (Vec<ModelInfo>, usize, usize)\nfn extract_model_name(path: &Path) -> Result<String, ModelError>\nfn read_model_content(path: &Path) -> Result<String, ModelError>\n```\n\nDirectory-based model loading pipeline. Uncovered: empty path validation, path length check, suspicious pattern detection, directory permission checks, processing entries with mixed success/failure, model name extraction edge cases. Test with tempdir containing valid and invalid YAML model files. #Coverage_Gap