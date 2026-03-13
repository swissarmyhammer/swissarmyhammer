import { invoke } from "@tauri-apps/api/core";
import type { CommandDef } from "@/lib/command-scope";

/** A menu item entry sent to the Rust side for native menu construction. */
export interface MenuItemManifest {
  id: string;
  name: string;
  menu: string;
  group: number;
  order: number;
  accelerator?: string;
  radio_group?: string;
  checked?: boolean;
}

/**
 * Extract a menu manifest from commands that have menuPlacement set.
 *
 * Iterates all commands, filters to those with menuPlacement, resolves
 * the keybinding for the given keymap mode, and returns a sorted array
 * of MenuItemManifest entries (sorted by menu, group, order).
 *
 * @param commands - The full list of command definitions.
 * @param keymapMode - The active keymap mode ("vim", "cua", or "emacs").
 * @returns Sorted array of menu item manifests.
 */
export function buildMenuManifest(
  commands: CommandDef[],
  keymapMode: string,
): MenuItemManifest[] {
  const items: MenuItemManifest[] = [];
  for (const cmd of commands) {
    if (!cmd.menuPlacement) continue;
    const p = cmd.menuPlacement;
    // Resolve key binding for the active keymap mode, falling back to CUA
    const keys = cmd.keys;
    const binding = keys?.[keymapMode as keyof typeof keys] ?? keys?.cua;
    items.push({
      id: cmd.id,
      name: cmd.name,
      menu: p.menu,
      group: p.group,
      order: p.order,
      accelerator: binding ? toAccelerator(binding) : undefined,
      radio_group: p.radioGroup,
      checked: p.checked,
    });
  }
  // Sort by menu, group, order for deterministic menu construction
  items.sort((a, b) => {
    if (a.menu !== b.menu) return a.menu.localeCompare(b.menu);
    if (a.group !== b.group) return a.group - b.group;
    return a.order - b.order;
  });
  return items;
}

/**
 * Convert a key binding string to a Tauri accelerator string.
 *
 * Replaces "Mod" with "CmdOrCtrl" so that Tauri maps it to Cmd on macOS
 * and Ctrl on other platforms.
 *
 * @param binding - A key binding string like "Mod+N" or "Mod+Shift+P".
 * @returns A Tauri accelerator string like "CmdOrCtrl+N".
 */
function toAccelerator(binding: string): string {
  return binding.replace(/\bMod\b/g, "CmdOrCtrl");
}

/**
 * Send the menu manifest to Rust to rebuild the native menu bar.
 *
 * Collects all commands with menuPlacement, builds the manifest, and
 * invokes the `rebuild_menu_from_manifest` Tauri command. Errors are
 * logged to console but not thrown (menu sync is best-effort).
 *
 * @param commands - The full list of command definitions.
 * @param keymapMode - The active keymap mode.
 */
export async function syncMenuToNative(
  commands: CommandDef[],
  keymapMode: string,
): Promise<void> {
  const manifest = buildMenuManifest(commands, keymapMode);
  try {
    await invoke("rebuild_menu_from_manifest", { manifest });
  } catch (e) {
    console.error("Failed to sync menu:", e);
  }
}
