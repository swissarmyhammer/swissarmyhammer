/**
 * Type declarations for custom vitest browser commands.
 * These augment the `commands` object available in browser tests.
 *
 * NOTE: This file must NOT share a basename with integration-commands.ts,
 * otherwise TypeScript shadows the .d.ts with the .ts source file.
 */

interface TestBoardConfig {
  name: string;
  tasks: { title: string; column?: string }[];
  perspectives?: { name: string; view: string }[];
}

interface TestBoardResult {
  dir: string;
  boardName: string;
  summary: any;
  tasks: any[];
  columns: any[];
  taskIds: string[];
  perspectiveIds: string[];
}

declare module "vitest/internal/browser" {
  interface BrowserCommands {
    createTestBoard: (config: TestBoardConfig) => Promise<TestBoardResult>;
    readEntity: (config: {
      dir: string;
      noun: string;
      id: string;
    }) => Promise<any>;
    moveTask: (config: {
      dir: string;
      taskId: string;
      column: string;
      beforeId?: string;
    }) => Promise<any>;
    listTasks: (config: {
      dir: string;
      column?: string;
    }) => Promise<{ count: number; tasks: any[] }>;
    createTempFile: (config: {
      dir: string;
      name: string;
      content: string;
    }) => Promise<string>;
    listPerspectives: (config: {
      dir: string;
    }) => Promise<{
      count: number;
      perspectives: { id: string; name: string; view: string }[];
    }>;
    cleanupTestBoard: (config: { dir: string }) => Promise<void>;
  }
}
