---
position_column: done
position_ordinal: ffe280
title: Fix TypeScript errors in keybindings.test.ts
---
All `keybindings.test.ts` test calls fail `tsc --noEmit` because `vi.fn()` is not assignable to `(id: string) => Promise<boolean>`. The mock needs to be typed: `vi.fn<(id: string) => Promise<boolean>>()` or `vi.fn().mockResolvedValue(false) as unknown as ...`. Also, `KeymapMode` is imported but not exported from keybindings.ts and is unused. There is also an unused `vi` import in `undo-stack.test.ts`. #blocker