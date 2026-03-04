/**
 * Represents a reversible command that can be executed, undone, and redone.
 *
 * Each command carries a human-readable label (e.g. "Move card to Done")
 * for display in UI undo/redo menus.
 */
export interface UndoableCommand {
  /** Execute the command for the first time. */
  do(): Promise<void>;
  /** Reverse the effect of the command. */
  undo(): Promise<void>;
  /** Re-apply the command after it has been undone. */
  redo(): Promise<void>;
  /** Human-readable description of the command. */
  label: string;
}

/**
 * A bounded stack of undoable commands with a moving pointer.
 *
 * `pointer` always points one past the last executed command:
 * - entries[0..pointer) have been done (and not undone)
 * - entries[pointer..length) are available for redo
 *
 * When a new command is pushed, the redo side is discarded.
 * When the stack exceeds `maxSize`, the oldest entries are trimmed.
 */
export class UndoStack {
  /** The ordered list of commands. */
  private entries: UndoableCommand[] = [];

  /** Index one past the last executed command. */
  private pointer = 0;

  /** Maximum number of entries to retain. */
  readonly maxSize: number;

  /**
   * Create an UndoStack.
   *
   * @param maxSize - Maximum number of entries to keep. Defaults to 100.
   */
  constructor(maxSize = 100) {
    this.maxSize = maxSize;
  }

  /** Whether there is at least one command that can be undone. */
  get canUndo(): boolean {
    return this.pointer > 0;
  }

  /** Whether there is at least one command that can be redone. */
  get canRedo(): boolean {
    return this.pointer < this.entries.length;
  }

  /**
   * Execute a command and push it onto the stack.
   *
   * Calls `cmd.do()`, appends the command at the current pointer position,
   * discards any commands on the redo side, and trims the stack if it
   * exceeds `maxSize`.
   *
   * @param cmd - The command to execute and record.
   */
  async push(cmd: UndoableCommand): Promise<void> {
    await cmd.do();

    // Discard redo side
    this.entries.length = this.pointer;

    this.entries.push(cmd);
    this.pointer++;

    // Trim oldest entries if over capacity
    if (this.entries.length > this.maxSize) {
      const excess = this.entries.length - this.maxSize;
      this.entries.splice(0, excess);
      this.pointer -= excess;
    }
  }

  /**
   * Undo the most recently executed command.
   *
   * Decrements the pointer and calls `undo()` on the command.
   * If there is nothing to undo, this is a no-op.
   */
  async undo(): Promise<void> {
    if (!this.canUndo) return;
    this.pointer--;
    await this.entries[this.pointer].undo();
  }

  /**
   * Redo the most recently undone command.
   *
   * Calls `redo()` on the command at the pointer and increments it.
   * If there is nothing to redo, this is a no-op.
   */
  async redo(): Promise<void> {
    if (!this.canRedo) return;
    await this.entries[this.pointer].redo();
    this.pointer++;
  }

  /**
   * Clear all entries and reset the pointer.
   */
  clear(): void {
    this.entries = [];
    this.pointer = 0;
  }
}
