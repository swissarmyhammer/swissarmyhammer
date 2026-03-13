import { describe, it, expect } from "vitest";
import { UndoStack, type UndoableCommand } from "./undo-stack";

/** Helper: creates a mock UndoableCommand that records calls. */
function mockCommand(label = "cmd"): UndoableCommand & {
  doCalls: number;
  undoCalls: number;
  redoCalls: number;
} {
  const cmd = {
    label,
    doCalls: 0,
    undoCalls: 0,
    redoCalls: 0,
    async do() {
      cmd.doCalls++;
    },
    async undo() {
      cmd.undoCalls++;
    },
    async redo() {
      cmd.redoCalls++;
    },
  };
  return cmd;
}

describe("UndoStack", () => {
  it("starts empty with canUndo=false and canRedo=false", () => {
    const stack = new UndoStack();
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(false);
  });

  it("push calls cmd.do() and makes canUndo true", async () => {
    const stack = new UndoStack();
    const cmd = mockCommand();

    await stack.push(cmd);

    expect(cmd.doCalls).toBe(1);
    expect(stack.canUndo).toBe(true);
    expect(stack.canRedo).toBe(false);
  });

  it("undo/redo cycle works correctly", async () => {
    const stack = new UndoStack();
    const cmd = mockCommand("move card");

    await stack.push(cmd);
    expect(stack.canUndo).toBe(true);
    expect(stack.canRedo).toBe(false);

    await stack.undo();
    expect(cmd.undoCalls).toBe(1);
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(true);

    await stack.redo();
    expect(cmd.redoCalls).toBe(1);
    expect(stack.canUndo).toBe(true);
    expect(stack.canRedo).toBe(false);
  });

  it("undo on empty stack is a no-op", async () => {
    const stack = new UndoStack();
    // Should not throw
    await stack.undo();
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(false);
  });

  it("redo on empty stack is a no-op", async () => {
    const stack = new UndoStack();
    // Should not throw
    await stack.redo();
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(false);
  });

  it("redo with nothing to redo is a no-op", async () => {
    const stack = new UndoStack();
    const cmd = mockCommand();
    await stack.push(cmd);

    // No undo performed, so redo should be a no-op
    await stack.redo();
    expect(cmd.redoCalls).toBe(0);
  });

  it("new push after undo clears the redo side", async () => {
    const stack = new UndoStack();
    const cmd1 = mockCommand("first");
    const cmd2 = mockCommand("second");
    const cmd3 = mockCommand("third");

    await stack.push(cmd1);
    await stack.push(cmd2);
    await stack.undo(); // undo cmd2

    expect(stack.canRedo).toBe(true);

    await stack.push(cmd3); // should clear cmd2 from redo side
    expect(stack.canRedo).toBe(false);

    // Undo should now undo cmd3, then cmd1
    await stack.undo();
    expect(cmd3.undoCalls).toBe(1);
    await stack.undo();
    expect(cmd1.undoCalls).toBe(1);

    // No more to undo
    expect(stack.canUndo).toBe(false);
  });

  it("respects maxSize and trims oldest entries", async () => {
    const stack = new UndoStack(3);
    const cmds = Array.from({ length: 5 }, (_, i) => mockCommand(`cmd-${i}`));

    for (const cmd of cmds) {
      await stack.push(cmd);
    }

    // Only 3 entries should remain (cmd-2, cmd-3, cmd-4)
    let undoCount = 0;
    while (stack.canUndo) {
      await stack.undo();
      undoCount++;
    }
    expect(undoCount).toBe(3);

    // The oldest two commands should not have been undone
    expect(cmds[0].undoCalls).toBe(0);
    expect(cmds[1].undoCalls).toBe(0);

    // The kept commands should have been undone
    expect(cmds[2].undoCalls).toBe(1);
    expect(cmds[3].undoCalls).toBe(1);
    expect(cmds[4].undoCalls).toBe(1);
  });

  it("clear resets the stack", async () => {
    const stack = new UndoStack();
    await stack.push(mockCommand());
    await stack.push(mockCommand());

    stack.clear();

    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(false);

    // undo/redo after clear are no-ops
    await stack.undo();
    await stack.redo();
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(false);
  });

  it("multiple undo then multiple redo restores in order", async () => {
    const stack = new UndoStack();
    const cmd1 = mockCommand("a");
    const cmd2 = mockCommand("b");
    const cmd3 = mockCommand("c");

    await stack.push(cmd1);
    await stack.push(cmd2);
    await stack.push(cmd3);

    // Undo all three in reverse order
    await stack.undo(); // undo c
    await stack.undo(); // undo b
    await stack.undo(); // undo a

    expect(cmd3.undoCalls).toBe(1);
    expect(cmd2.undoCalls).toBe(1);
    expect(cmd1.undoCalls).toBe(1);
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(true);

    // Redo all three in forward order
    await stack.redo(); // redo a
    await stack.redo(); // redo b
    await stack.redo(); // redo c

    expect(cmd1.redoCalls).toBe(1);
    expect(cmd2.redoCalls).toBe(1);
    expect(cmd3.redoCalls).toBe(1);
    expect(stack.canUndo).toBe(true);
    expect(stack.canRedo).toBe(false);
  });

  it("defaults maxSize to 100", () => {
    const stack = new UndoStack();
    expect(stack.maxSize).toBe(100);
  });
});
