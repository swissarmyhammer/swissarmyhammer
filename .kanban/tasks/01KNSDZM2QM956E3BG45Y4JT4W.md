---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffbc80
title: '[nit] Missing run_doctor and run_doctor_verbose tests in doctor.rs'
---
**File**: code-context-cli/src/doctor.rs\n\n**What**: The shelltool-cli doctor.rs has `test_run_doctor` and `test_run_doctor_verbose` tests that verify the top-level `run_doctor()` function (which includes `print_table`). The code-context-cli doctor.rs omits these -- it only tests `run_diagnostics` on the struct, not the public `run_doctor(verbose)` entry point.\n\n**Suggestion**: Add tests mirroring the shelltool pattern:\n```rust\n#[test]\nfn test_run_doctor() {\n    let exit_code = run_doctor(false);\n    assert!(exit_code <= 2);\n}\n\n#[test]\nfn test_run_doctor_verbose() {\n    let exit_code = run_doctor(true);\n    assert!(exit_code <= 2);\n}\n```" #review-finding