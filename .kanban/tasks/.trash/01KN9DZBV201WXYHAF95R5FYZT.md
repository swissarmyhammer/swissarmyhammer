---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: 'Fix builtin_attachment_field_round_trips_through_yaml: wrong FieldType'
---
Test `defaults::tests::builtin_attachment_field_round_trips_through_yaml` fails at defaults.rs:436 with:
`expected FieldType::Reference, got Attachment { max_bytes: 104857600, multiple: true }`

The test expects the attachment field to deserialize as FieldType::Reference but a new FieldType::Attachment variant now exists. The test assertion needs updating to match the new type.

#test-failure