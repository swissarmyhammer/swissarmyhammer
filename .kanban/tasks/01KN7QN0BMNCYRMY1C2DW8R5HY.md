---
assignees:
- claude-code
position_column: todo
position_ordinal: '8880'
title: 'Fix app-mode-context.test.tsx: ''changes mode via setMode'' finds duplicate testid'
---
Test 'changes mode via setMode' in src/lib/app-mode-context.test.tsx fails with TestingLibraryElementError: Found multiple elements by [data-testid='mode']. The test renders a component, clicks a button to change mode, then calls screen.getByTestId('mode') which finds two elements (the old 'normal' span and the new 'command' span). The test needs to use getAllByTestId or the component/test needs cleanup so only one element with that testid exists at query time. #test-failure