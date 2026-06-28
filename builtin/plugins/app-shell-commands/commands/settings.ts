// Settings sub-domain — ports the three keymap commands from `settings.yaml`.
// Each is a radio-group menu entry that sets the active keymap mode; all three
// route to ui_state `set keymap` with the `mode` param, which rebinds the
// hotkey dispatcher and changes how `list command` resolves `keys`.

import {
  type CommandSpec,
  type UiStateDispatch,
} from "./context.ts";

/** Build the three `settings.keymap.*` command registrations. */
export function settingsCommands(uiState: UiStateDispatch): CommandSpec[] {
  // The three keymaps share an identical shape — a radio-group menu entry that
  // sets one keymap mode — so generate them from a table to keep the metadata
  // 1:1 with `settings.yaml` while avoiding three near-identical literals.
  const keymaps: ReadonlyArray<{ mode: string; name: string; order: number }> = [
    // settings.yaml order: cua(0), vim(1), emacs(2).
    { mode: "cua", name: "Standard Keybindings", order: 0 },
    { mode: "vim", name: "Vim Keybindings", order: 1 },
    { mode: "emacs", name: "Emacs Keybindings", order: 2 },
  ];

  return keymaps.map(({ mode, name, order }) => ({
    id: `settings.keymap.${mode}`,
    name,
    menu: {
      path: ["App", "Settings"],
      group: 0,
      order,
      radio_group: "keymap",
    },
    execute: async () => {
      return await uiState.ui_state.ui_state.keymap.set({ mode });
    },
  }));
}
