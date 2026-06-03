// command-sdk-direct — exercises both command-registration forms in one plugin.
//
// The `command` operation tool is reachable two ways through the SDK:
//
//   * The **convention** form — `ensureServices(this, ["commands"])` to
//     activate the service, then `registerCommands(this, [...])` to install
//     each command. This is the helper-driven path every command-registering
//     plugin should use.
//   * The **direct** form — `this.commands.command.command.register({...})`
//     reaches the same operation through the SDK's path-form dispatch Proxy.
//     This is what `registerCommands` itself calls under the hood; exercising
//     it directly proves the convention helper is a thin loop and the two
//     paths produce the same observable state.
//
// This bundle registers ONE command through each form. The end-to-end test
// asserts that both commands land on the host's command registry with the
// same shape, and that unload purges both — proving the per-plugin ledger
// cleanup covers both paths uniformly.

import {
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

/** The command id registered through the {@link registerCommands} convention. */
const CONVENTION_COMMAND_ID = "command-sdk.convention";

/** The command id registered through the direct path-form dispatch. */
const DIRECT_COMMAND_ID = "command-sdk.direct";

/**
 * The command-sdk-direct example plugin.
 *
 * Registers two commands — one through {@link registerCommands}, one through
 * the SDK's path-form dispatch Proxy directly — so an integration test can
 * assert the two forms produce the same observable state and that unload
 * purges both via the per-plugin ledger.
 */
export default class CommandSdkDirectPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Command SDK Direct Form";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Registers one command via `registerCommands` and one via the direct `this.commands.command.command.register` form, proving both paths produce the same observable state.";

  /**
   * Activates the `commands` service and registers one command through each
   * form.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    // (1) Activate the `commands` service via the convention helper.
    await ensureServices(this, ["commands"]);

    // (2) Register one command through the `registerCommands` convention.
    await registerCommands(this, [
      {
        id: CONVENTION_COMMAND_ID,
        name: "Convention Form",
        execute: () => "registered via registerCommands",
      },
    ]);

    // (3) Register a second command through the SDK's path-form dispatch
    //     Proxy directly. The dispatch path
    //
    //       this.commands.command.command.register({ ... })
    //
    //     resolves through `tools/call("command", { op: "register command",
    //     ... })` against the registered `commands` server — exactly the
    //     call `registerCommands` makes under the hood. The function value
    //     in `execute` is marshalled to a `$callback` marker by the SDK
    //     before the call leaves the isolate.
    await this.commands.command.command.register({
      id: DIRECT_COMMAND_ID,
      name: "Direct Form",
      execute: () => "registered via this.commands.command.command.register",
    });

    this.log.info(
      `command-sdk-direct: registered '${CONVENTION_COMMAND_ID}' (convention) and '${DIRECT_COMMAND_ID}' (direct)`,
    );
  }
}