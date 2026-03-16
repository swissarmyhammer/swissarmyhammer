---
position_column: done
position_ordinal: a7
title: 'onnxruntime-coreml-sys: Session::new ignores errors from get_input_count / get_output_name'
---
File: onnxruntime-coreml-sys/src/lib.rs:322-335

After successfully creating the session, the return values from `ort_wrapper_session_get_input_count` and `ort_wrapper_session_get_input_name` are silently ignored. If these calls fail (ret != 0), the session is returned with empty input/output name lists, causing silent incorrect behavior when `run()` is called (wrong number of inputs).

Suggestion: Check the return code of each get_input_count / get_output_count / get_input_name / get_output_name call and propagate errors. Release the session on failure. #review-finding #blocker