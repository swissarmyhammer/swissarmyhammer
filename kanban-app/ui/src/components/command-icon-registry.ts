/**
 * Icon registry mapping kebab-case lucide-react icon names to the concrete
 * icon components used by `<CommandButton>`.
 *
 * The registry is a small hand-curated map rather than a dynamic lookup
 * against lucide's full `icons` export. This keeps the tab-button icon set
 * intentional and discoverable: when a new YAML command annotates itself with
 * a `tab_button.icon` we have not seen before, that command's migration task
 * is responsible for adding the icon import here. The dev-time payoff is an
 * obvious error trail at code-review time rather than a silent fallback in
 * the running UI.
 *
 * # Adding a new tab-button icon
 *
 * 1. Import the lucide component below.
 * 2. Add an entry to `COMMAND_ICONS` keyed by the kebab-case name the YAML
 *    uses (matches lucide's icon name, e.g. `arrow-up-down`).
 * 3. The new icon becomes available to every `<CommandButton>` with
 *    `tab_button.icon: <name>`.
 *
 * Unknown names render `<HelpCircle>` rather than crashing — a deliberate
 * fallback so a typo in YAML produces a visible-but-broken icon instead of a
 * blank tab bar.
 */

import {
  ArrowUpDown,
  Filter,
  Group,
  HelpCircle,
  Plus,
  type LucideIcon,
} from "lucide-react";

/**
 * Hand-curated map from `tab_button.icon` names (as they appear in YAML
 * command definitions) to lucide-react icon components.
 *
 * Keys are kebab-case to match lucide's own naming convention (and the YAML
 * field's documented shape). See file-level doc for the contract around
 * adding new entries.
 */
const COMMAND_ICONS: Readonly<Record<string, LucideIcon>> = {
  filter: Filter,
  group: Group,
  plus: Plus,
  "arrow-up-down": ArrowUpDown,
};

/**
 * Resolve a `tab_button.icon` name to a lucide-react icon component.
 *
 * Returns `HelpCircle` for any name not present in {@link COMMAND_ICONS} so
 * callers never have to render conditionally on the registry hit/miss — the
 * fallback is itself a renderable component with the same surface as a real
 * hit.
 */
export function commandIconFor(name: string): LucideIcon {
  return COMMAND_ICONS[name] ?? HelpCircle;
}
