/**
 * Type declarations for custom vitest browser commands.
 * These augment the `commands` object available in browser tests.
 */

interface TestBoardConfig {
  name: string;
  tasks: { title: string; column?: string }[];
}

interface TestBoardResult {
  dir: string;
  boardData: any;
  tasks: any[];
  columns: any[];
  taskIds: string[];
}

declare module "vitest/browser" {
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
    cleanupTestBoard: (config: { dir: string }) => Promise<void>;
  }
}
