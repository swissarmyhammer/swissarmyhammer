---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd580
title: Fix attachment-editor.test.tsx failures (9 tests)
---
All 9 tests failing in `src/components/fields/editors/attachment-editor.test.tsx`:\n- renders attachment filenames\n- renders remove buttons for each attachment\n- fires onChange with attachment removed/empty\n- fires onChange with path appended\n- does NOT fire onChange when cancelled\n- filters out numbers, objects without id\n- keeps a single AttachmentMeta object\n\nMay be related to FieldType::Attachment model change (see commit af273a4f4).\n\n#test-failure #test-failure