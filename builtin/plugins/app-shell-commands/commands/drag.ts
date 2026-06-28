// Drag sub-domain — ports the three cross-window drag commands from
// `drag.yaml`. All three are non-undoable, invisible (driven by the drag
// gesture, never the palette/menu) and route to the ui_state drag state
// machine: `start drag` opens a session, `complete drag` takes it, `cancel
// drag` clears it.

import {
  type CommandContext,
  type CommandSpec,
  type UiStateDispatch,
} from "./context.ts";

/** Build the three `drag.*` command registrations. */
export function dragCommands(uiState: UiStateDispatch): CommandSpec[] {
  return [
    // ─── drag.start ─────────────────────────────────────────────────────────
    // YAML: undoable:false, visible:false. Routes to ui_state `start drag`,
    // threading the drag session fields the gesture surface pre-fills in args
    // (session_id / entity_type / entity_id / source_board_path /
    // source_window_label / copy_mode / started_at_ms).
    {
      id: "drag.start",
      name: "Start Drag",
      undoable: false,
      visible: false,
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.drag.start({ ...(ctx.args ?? {}) });
      },
    },

    // ─── drag.cancel ────────────────────────────────────────────────────────
    // YAML: undoable:false, visible:false. Routes to ui_state `cancel drag`
    // (clears the active session).
    {
      id: "drag.cancel",
      name: "Cancel Drag",
      undoable: false,
      visible: false,
      execute: async () => {
        return await uiState.ui_state.ui_state.drag.cancel({});
      },
    },

    // ─── drag.complete ──────────────────────────────────────────────────────
    // YAML: undoable:false, visible:false. Routes to ui_state `complete drag`
    // (returns and clears the active session).
    {
      id: "drag.complete",
      name: "Complete Drag",
      undoable: false,
      visible: false,
      execute: async () => {
        return await uiState.ui_state.ui_state.drag.complete({});
      },
    },
  ];
}
