---
assignees:
- claude-code
depends_on:
- 01KN9M1F7ZN6GMEB8ZR10MKYT5
- 01KN9M1K1CNTWWSJ7AXF0T7WBS
- 01KN9M1PTMP4EN1R3WZAFP5FE6
- 01KN9M1TS9ECRG7EH59VKS6MRM
- 01KN9M1YM69YRV0XMHR1K4XX1C
- 01KN9M27VAJ5NDC7R0HGSG2QWN
- 01KN9M2BQFNW6YCQ5R1EXCYKY6
- 01KN9M2FJ85EDHB1JXJTT95RCF
- 01KN9M2KRS7CQ7R0E0MPAK0WHP
- 01KN9M2QTXNHN58QJV4NK9SXTT
- 01KN9M31AYZET3K1PA0VF7TQZX
- 01KN9M355YR2SC14034B329T5J
- 01KN9M39095F2C2VF12H9D52W8
- 01KN9M3CTJQQM8NRJKC5GY7PC4
- 01KN9M3GMYD48WXWWTEX97YX95
- 01KN9M3S63SDP74M8P2RS9VYVX
- 01KN9M3X4ZBYMS38YYRFAKHKAG
- 01KN9M411J65GDYQAJ7QSG37JZ
- 01KN9M44RNZ8RBBGJPMEXF7T46
- 01KN9M48F5XCQ10YBEMDWC6N4S
- 01KN9M4HJ2TFRXSCSXWAEXHMSD
- 01KN9M4NAQM8MZM4FC6NXA1ZMW
- 01KN9M4S449VNCQDZ0YDBCJTKK
- 01KN9M4WW4Z4DN252N0WK8Z7SV
- 01KN9M50KT6NGZW8HJD4TGZDSV
position_column: todo
position_ordinal: a980
title: Delete backendDispatch, dispatchCommand, and useExecuteCommand exports
---
## What

Final cleanup after all migrations are complete. Remove the deprecated public exports from `kanban-app/ui/src/lib/command-scope.tsx`:

1. Delete `export async function backendDispatch(...)` — keep as private `_backendDispatch` (used internally by `useDispatchCommand` and context-menu.ts)
2. Delete `export async function dispatchCommand(...)` — fully replaced by `useDispatchCommand`
3. Delete `export function useExecuteCommand()` — fully replaced by `useDispatchCommand`
4. Update `kanban-app/ui/src/lib/command-scope.test.tsx` — remove tests for deleted functions, ensure `useDispatchCommand` tests cover the same scenarios
5. Verify: `grep -r 'backendDispatch\|dispatchCommand\|useExecuteCommand' kanban-app/ui/src/ --include='*.ts' --include='*.tsx'` returns only the private `_backendDispatch` definition and context-menu.ts usage

## Acceptance Criteria
- [ ] `backendDispatch` is not exported from command-scope.tsx
- [ ] `dispatchCommand` does not exist in command-scope.tsx
- [ ] `useExecuteCommand` does not exist in command-scope.tsx
- [ ] No file imports `backendDispatch`, `dispatchCommand`, or `useExecuteCommand`
- [ ] `_backendDispatch` exists as private for internal use

## Tests
- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass
- [ ] TypeScript compile: `cd kanban-app/ui && pnpm tsc --noEmit` — no errors