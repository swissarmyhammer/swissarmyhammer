---
position_column: done
position_ordinal: q0
title: '[WARNING] root_uri construction does not percent-encode the path'
---
File: swissarmyhammer-lsp/src/daemon.rs, line 316-321\n\nThe root_uri is constructed with a naive format!(\"file://{}\", path) which does not percent-encode special characters (spaces, unicode, etc.) in the filesystem path. This violates the LSP spec which requires a proper URI.\n\nShould use the lsp-types or url crate's Url::from_file_path() instead, which handles encoding correctly. The lsp-types crate is already a dependency.\n\nThis will break for any workspace path containing spaces or special characters. #review-finding