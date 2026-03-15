---
position_column: done
position_ordinal: ffff8780
title: 'WARNING: useFocusedScope reads from ref without triggering re-render'
---
entity-focus-context.tsx: `useFocusedScope()` calls `getScope(focusedMoniker)` which reads from a `useRef` (registryRef). Since ref mutations don't trigger re-renders, there is a race: if a FocusScope mounts and registers its scope *after* `setFocus` is called for that moniker, `useFocusedScope` will return null (stale) because the component never re-rendered after the ref write. In practice this may not manifest often because `setFocus` updates state (causing re-render) and by that time the scope is usually registered. But the timing is fragile -- if a component calls `setFocus` in the same effect as a FocusScope mounts, the scope may not be in the registry yet. Consider documenting this ordering constraint or adding a safety re-read. #review-finding