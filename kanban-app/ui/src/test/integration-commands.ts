/**
 * Server-side vitest browser commands for integration tests.
 *
 * These run in Node.js (not the browser) and can use filesystem,
 * child_process, etc. Browser tests call them via `commands.xxx()`.
 *
 * They shell out to the real `kanban` CLI to create boards, add tasks,
 * and read back entity state — giving us true end-to-end coverage
 * from UI interaction to file-on-disk mutation.
 *
 * IMPORTANT: All return values must be plain JSON-serializable objects.
 * Vitest serializes command results between server and browser via WebSocket.
 * Large or deeply nested objects cause "Map maximum size exceeded" errors.
 * Strip responses to only what the browser tests need.
 */

import { execSync } from "child_process";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readdirSync,
  readFileSync,
} from "fs";
import { tmpdir } from "os";
import { join, resolve } from "path";
import yaml from "js-yaml";
import type { BrowserCommand } from "vitest/node";

// Resolve the kanban CLI binary from the project's debug build
const KANBAN_BIN = resolve(__dirname, "../../../../target/debug/kanban");

// Resolve builtin YAML directories relative to this file
const DEFINITIONS_DIR = resolve(
  __dirname,
  "../../../../swissarmyhammer-kanban/builtin/definitions",
);
const BUILTIN_ENTITIES_DIR = resolve(
  __dirname,
  "../../../../swissarmyhammer-kanban/builtin/entities",
);

/** Run a kanban CLI command in the given directory. */
function kanban(dir: string, cmd: string): string {
  return execSync(`"${KANBAN_BIN}" ${cmd}`, {
    cwd: dir,
    encoding: "utf-8",
    timeout: 10000,
  });
}

/** Parse YAML output from the kanban CLI. */
function parseYaml(output: string): any {
  return yaml.load(output);
}

/** Strip a task to only the fields the frontend cares about. */
function stripTask(t: any) {
  return {
    id: t.id,
    title: t.title,
    position: { column: t.position?.column, ordinal: t.position?.ordinal },
    assignees: t.assignees || [],
    tags: t.tags || [],
    attachments: t.attachments || [],
    description: t.description || "",
    progress: t.progress || 0,
    depends_on: t.depends_on || [],
  };
}

/** Strip a column to only the fields the frontend cares about. */
function stripColumn(c: any) {
  return { id: c.id, name: c.name, order: c.order };
}

/**
 * Create a real .kanban board in a temp directory with tasks.
 * Returns stripped board data, tasks, and columns.
 */
export const createTestBoard: BrowserCommand<
  [
    config: {
      name: string;
      tasks: { title: string; column?: string }[];
      perspectives?: { name: string; view: string }[];
    },
  ]
> = ({ testPath: _testPath }, config) => {
  const dir = mkdtempSync(join(tmpdir(), "kanban-integration-"));

  kanban(dir, `board init --name "${config.name}"`);

  const taskIds: string[] = [];
  for (const task of config.tasks) {
    const output = parseYaml(kanban(dir, `task add --title "${task.title}"`));
    taskIds.push(output.id);
    if (task.column && task.column !== "todo") {
      kanban(dir, `task move --id ${output.id} --column ${task.column}`);
    }
  }

  // Create perspectives via CLI
  const perspectiveIds: string[] = [];
  for (const p of config.perspectives ?? []) {
    const output = parseYaml(
      kanban(dir, `perspective add --name "${p.name}" --view ${p.view}`),
    );
    perspectiveIds.push(output.id);
  }

  const boardRaw = parseYaml(kanban(dir, "board get --include_counts"));
  const taskList = parseYaml(kanban(dir, "tasks list"));
  const columnList = parseYaml(kanban(dir, "columns list"));

  return {
    dir,
    boardName: boardRaw.name as string,
    summary: boardRaw.summary,
    tasks: (taskList.tasks || []).map(stripTask),
    columns: (columnList.columns || []).map(stripColumn),
    taskIds,
    perspectiveIds,
  };
};

/**
 * Read a single entity from disk via CLI. Returns stripped fields.
 */
export const readEntity: BrowserCommand<
  [config: { dir: string; noun: string; id: string }]
> = ({ testPath: _testPath }, config) => {
  const raw = parseYaml(
    kanban(config.dir, `${config.noun} get --id ${config.id}`),
  );
  if (config.noun === "task") return stripTask(raw);
  if (config.noun === "column") return stripColumn(raw);
  return raw;
};

/**
 * Move a task to a different column via the real CLI.
 */
export const moveTask: BrowserCommand<
  [config: { dir: string; taskId: string; column: string; beforeId?: string }]
> = ({ testPath: _testPath }, config) => {
  let cmd = `task move --id ${config.taskId} --column ${config.column}`;
  if (config.beforeId) {
    cmd += ` --before_id ${config.beforeId}`;
  }
  const raw = parseYaml(kanban(config.dir, cmd));
  return stripTask(raw);
};

/**
 * List tasks from the real .kanban directory.
 */
export const listTasks: BrowserCommand<
  [config: { dir: string; column?: string }]
> = ({ testPath: _testPath }, config) => {
  let cmd = "tasks list";
  if (config.column) {
    cmd += ` --column ${config.column}`;
  }
  const raw = parseYaml(kanban(config.dir, cmd));
  return {
    count: raw.count || 0,
    tasks: (raw.tasks || []).map(stripTask),
  };
};

/**
 * Create a temporary file for drag-and-drop attachment tests.
 */
export const createTempFile: BrowserCommand<
  [config: { dir: string; name: string; content: string }]
> = ({ testPath: _testPath }, config) => {
  const filePath = join(config.dir, config.name);
  writeFileSync(filePath, config.content, "utf-8");
  return filePath;
};

/**
 * List perspectives by reading YAML files from .kanban/perspectives/.
 * The CLI lacks a `perspective list` subcommand, so we read the directory directly.
 */
export const listPerspectives: BrowserCommand<[config: { dir: string }]> = (
  { testPath: _testPath },
  config,
) => {
  const perspDir = join(config.dir, ".kanban", "perspectives");
  try {
    const files = readdirSync(perspDir).filter((f) => f.endsWith(".yaml"));
    const perspectives = files.map((f) => {
      const content = require("fs").readFileSync(join(perspDir, f), "utf-8");
      const p = yaml.load(content) as any;
      return { id: p.id, name: p.name, view: p.view };
    });
    return { perspectives, count: perspectives.length };
  } catch {
    return { perspectives: [], count: 0 };
  }
};

/**
 * Clean up a temp directory.
 */
export const cleanupTestBoard: BrowserCommand<[config: { dir: string }]> = (
  { testPath: _testPath },
  config,
) => {
  try {
    execSync(`rm -rf "${config.dir}"`);
  } catch {
    // Best effort cleanup
  }
};

/**
 * Load all builtin field definitions from YAML.
 * Runs on the Node.js server side so browser tests can access filesystem data.
 */
export const loadFieldDefinitions: BrowserCommand<[]> = () => {
  const files = readdirSync(DEFINITIONS_DIR).filter((f) => f.endsWith(".yaml"));
  return files.map((f) => {
    const content = readFileSync(join(DEFINITIONS_DIR, f), "utf-8");
    return yaml.load(content) as Record<string, unknown>;
  });
};

/**
 * Load a builtin entity definition from YAML by entity type name.
 * Runs on the Node.js server side so browser tests can access filesystem data.
 */
export const loadEntityDefinition: BrowserCommand<
  [config: { entityType: string }]
> = ({ testPath: _testPath }, config) => {
  const filePath = join(BUILTIN_ENTITIES_DIR, `${config.entityType}.yaml`);
  const content = readFileSync(filePath, "utf-8");
  return yaml.load(content) as Record<string, unknown>;
};

/**
 * Load all builtin entity definitions from YAML.
 * Returns an array of parsed entity objects.
 */
export const loadAllEntityDefinitions: BrowserCommand<[]> = () => {
  const files = readdirSync(BUILTIN_ENTITIES_DIR).filter((f) =>
    f.endsWith(".yaml"),
  );
  return files.map((f) => {
    const content = readFileSync(join(BUILTIN_ENTITIES_DIR, f), "utf-8");
    return yaml.load(content) as Record<string, unknown>;
  });
};
