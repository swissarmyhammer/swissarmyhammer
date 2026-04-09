---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: code-context-cli
title: Create code-context.png icon from source image
---
## What
Convert `~/Downloads/image-CfkiACHBV6FV2xGzMUk5i8iBk4pNoP.png` to `code-context-cli/code-context.png`.

The icon lives in the **crate root** named after the tool (matching `shelltool-cli/shelltool.png` convention).

Use `sips` (macOS built-in, always available):
```
sips -z 512 512 ~/Downloads/image-CfkiACHBV6FV2xGzMUk5i8iBk4pNoP.png --out code-context-cli/code-context.png
```

If ImageMagick `convert` is available, prefer it for quality:
```
convert ~/Downloads/image-CfkiACHBV6FV2xGzMUk5i8iBk4pNoP.png -resize 512x512 code-context-cli/code-context.png
```

## Acceptance Criteria
- [ ] `code-context-cli/code-context.png` exists as a valid PNG
- [ ] `file code-context-cli/code-context.png` reports PNG image data

## Tests
- [ ] `file code-context-cli/code-context.png` confirms PNG format

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.