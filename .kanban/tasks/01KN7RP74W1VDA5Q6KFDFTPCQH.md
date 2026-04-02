---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd080
title: 'Integration tests: real .kanban board + browser UI for drag-and-drop scenarios'
---
## What

True end-to-end integration tests that create a real `.kanban` board in a temp directory, render the actual UI in Chromium via vitest-browser-react, perform real drag interactions, and assert both the visual state AND the underlying entity data on disk.

## Why

The existing browser tests verify DOM event behavior (preventDefault, stopPropagation, MIME discrimination) but mock all backend calls. We need tests that prove the full pipeline works: drag card → UI handler → backend command → file on disk changes → UI reflects new state.

## Architecture

### Server-side vitest commands (Node.js)

Define custom vitest browser commands that run in Node and can use filesystem + child_process:

```typescript
// kanban-app/ui/src/test/integration-commands.ts
import { execSync } from 'child_process';
import { mkdtempSync, readFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import yaml from 'js-yaml';

// Runs on the Node server, callable from browser tests
export function createTestBoard(ctx, config: { name: string, tasks: { title: string, column?: string }[] }) {
  const dir = mkdtempSync(join(tmpdir(), 'kanban-test-'));
  const kanban = (cmd: string) =>
    execSync(`cd ${dir} && kanban ${cmd}`, { encoding: 'utf-8' });

  kanban(`board init --name \"${config.name}\"`);

  for (const task of config.tasks) {
    const output = kanban(`task add --title \"${task.title}\"`);
    if (task.column) {
      const id = yaml.load(output).id;
      kanban(`task move --id ${id} --column ${task.column}`);
    }
  }

  // Read back real data in the shape the frontend expects
  const boardData = yaml.load(kanban('board get'));
  const tasks = yaml.load(kanban('tasks list'));
  const columns = yaml.load(kanban('columns list'));
  return { dir, boardData, tasks: tasks.tasks, columns: columns.columns };
}

export function readEntity(ctx, config: { dir: string, entityType: string, id: string }) {
  const kanban = (cmd: string) =>
    execSync(`cd ${config.dir} && kanban ${cmd}`, { encoding: 'utf-8' });
  return yaml.load(kanban(`${config.entityType} get --id ${config.id}`));
}

export function dispatchCommand(ctx, config: { dir: string, cmd: string, args: Record<string, any> }) {
  // Build CLI args from the command args
  // This routes dispatch_command through the real kanban CLI
  const kanban = (cmd: string) =>
    execSync(`cd ${config.dir} && kanban ${cmd}`, { encoding: 'utf-8' });
  // Map command names to CLI invocations...
  return yaml.load(kanban(/* mapped command */));
}

export function cleanupTestBoard(ctx, config: { dir: string }) {
  execSync(`rm -rf ${config.dir}`);
}
```

Register in vitest config:

```typescript
// vite.config.ts browser project
test: {
  browser: {
    commands: {
      createTestBoard,
      readEntity,
      dispatchCommand,
      cleanupTestBoard,
    },
  },
}
```

### Smart invoke mock (browser-side)

```typescript
// kanban-app/ui/src/test/integration-invoke-mock.ts
// Caches fixture data from server commands, routes invoke() calls

let boardDir: string;
let cachedBoardData: any;
let cachedTasks: any[];

export async function setupIntegrationMock(config) {
  const result = await commands.createTestBoard(config);
  boardDir = result.dir;
  cachedBoardData = result.boardData;
  cachedTasks = result.tasks;
}

export async function integrationInvoke(cmd: string, args?: any) {
  switch (cmd) {
    case 'get_board_data':
      return cachedBoardData;
    case 'list_entities':
      if (args.entityType === 'task') return cachedTasks;
      return [];
    case 'dispatch_command':
      // Route through real CLI via server command
      const result = await commands.dispatchCommand({
        dir: boardDir,
        cmd: args.cmd,
        args: args.args,
      });
      // Refresh cached data after mutation
      const fresh = await commands.createTestBoard({...});
      cachedTasks = fresh.tasks;
      return result;
    // ... other commands
  }
}
```

### Test scenarios

Each test creates a real board, renders the full UI, interacts, and asserts.

## Test Scenarios

### 1. Move card within column (reorder)

```
Setup: Board with 3 tasks in todo: [A, B, C]
Action: Drag task C above task A
Assert UI: todo column shows [C, A, B]
Assert disk: readEntity(C).position_ordinal < readEntity(A).position_ordinal
```

### 2. Move card between columns

```
Setup: Board with task A in todo, task B in doing
Action: Drag task A to doing column
Assert UI: todo column empty, doing column shows [B, A] or [A, B]
Assert disk: readEntity(A).position_column === 'doing'
```

### 3. File attachment drop on card inspector

```
Setup: Board with task A, inspector open on task A
Action: Drag a file onto the attachment field in inspector
Assert UI: Attachment pill/filename appears in inspector
Assert disk: readEntity(A).fields.attachments includes the file path
```

## Files to create

- `kanban-app/ui/src/test/integration-commands.ts` — server-side vitest commands (Node.js)
- `kanban-app/ui/src/test/integration-helpers.tsx` — shared provider tree, invoke mock, test board factory
- `kanban-app/ui/src/test/integration-helpers.d.ts` — type declarations for custom commands
- `kanban-app/ui/src/components/board-integration.browser.test.tsx` — move within column, move between columns
- `kanban-app/ui/src/components/inspector-attachment.browser.test.tsx` — file drop on inspector
- `kanban-app/ui/vite.config.ts` — register custom commands in browser project

## Acceptance Criteria

- [ ] Tests create a real .kanban directory with real entity files
- [ ] Tests render the actual BoardView with real entity data (not hardcoded mocks)
- [ ] Card move within column: UI shows correct order + entity file has updated ordinal
- [ ] Card move between columns: UI shows card in new column + entity file has updated column
- [ ] File drop on inspector: UI shows attachment + entity file has attachment path
- [ ] Temp directories are cleaned up after tests
- [ ] All existing unit and browser tests still pass

## Dependencies

- `kanban` CLI binary must be built and available in PATH (or referenced by absolute path)
- vitest browser custom commands (available in vitest 4.x)
- `js-yaml` already in devDependencies