---
position_column: done
position_ordinal: q6
title: '[WARNING] supervisor.rs start() accepts &Path via PathBuf but detect_projects returns Result<_, String>'
---
File: swissarmyhammer-lsp/src/supervisor.rs, line 42\n\ndetect_projects() returns Result<Vec<DetectedProject>, String> -- the error type is String, not a structured error. This is an upstream issue in swissarmyhammer-project-detection, but the supervisor propagates it without context.\n\nThe error message 'project detection failed: {e}' could use .context() or at least include the workspace path for debuggability. Consider filing a follow-up to fix detect_projects to return a typed error. #review-finding