---
assignees:
- claude-code
position_column: todo
position_ordinal: b080
title: Fix attachment-editor.test.tsx failures (9 tests)
---
Nine failures in src/components/fields/editors/attachment-editor.test.tsx:\n- renders attachment filenames\n- renders remove buttons for each attachment\n- fires onChange with attachment removed from array\n- fires onChange with empty array when removing last attachment\n- fires onChange with path appended when open() resolves with paths\n- does NOT fire onChange when open() resolves with null (cancelled)\n- filters out numbers from an attachment array\n- filters out objects without an id property\n- keeps a single AttachmentMeta object\n\n#test-failure #test-failure