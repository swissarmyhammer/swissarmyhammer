// @swissarmyhammer/plugin — the SwissArmyHammer plugin SDK.
//
// This module is the host SDK a plugin imports as `@swissarmyhammer/plugin`.
// It is served from host memory as a virtual module (it never lives on disk)
// and is transpiled to JavaScript by the runtime before a plugin loads.
//
// The SDK is two things:
//
//   1. The `Plugin` base class — the entire API surface a plugin author
//      subclasses: optional `load`/`unload` lifecycle hooks, `register` /
//      `unregister` for pointing the platform at MCP servers, a scoped
//      `log`, and `track` for mid-session disposables.
//   2. The generic dispatch Proxy — `makeDispatcher` and `makePluginThis` —
//      that turns a plain property path such as `this.srv.kanban.task.add`
//      into an MCP `tools/call`. No server, tool, noun, or verb name is ever
//      baked into the SDK; every name works because a server with that name
//      is registered and the Proxy asks the host on every call.
//
// All host traffic crosses the JavaScript/Rust boundary through one
// `deno_core` op, `op_host_dispatch`. The SDK speaks a small JSON envelope
// over that op (see `HostBridge`); the host's `HostDispatcher` answers it.

/**
 * The `deno_core` op-call surface exposed inside every plugin isolate.
 *
 * `op_host_dispatch` is the single synchronous seam plugin code uses to call
 * the host. It takes one JSON payload and returns the host's JSON response,
 * or throws if the host rejects the call.
 */
declare const Deno: {
  core: { ops: { op_host_dispatch(payload: unknown): unknown } };
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/**
 * Where a registered MCP server lives. A plugin picks exactly one kind.
 *
 * - `{ url }`  — an MCP endpoint served over HTTP, with optional headers.
 * - `{ cli }`  — an MCP server spawned as a stdio subprocess.
 * - `{ rust }` — a host-exposed in-process Rust server, addressed by id.
 */
export type ServerSource =
  | { url: string; headers?: Record<string, string> }
  | { cli: string[]; env?: Record<string, string>; cwd?: string }
  | { rust: string };

/**
 * A scoped logger handed to every plugin as `this.log`.
 *
 * Each method takes a message and optional structured fields; the host
 * decides how the records are surfaced.
 */
export interface Logger {
  /** Log at debug level. */
  debug(message: string, fields?: Record<string, unknown>): void;
  /** Log at info level. */
  info(message: string, fields?: Record<string, unknown>): void;
  /** Log at warning level. */
  warn(message: string, fields?: Record<string, unknown>): void;
  /** Log at error level. */
  error(message: string, fields?: Record<string, unknown>): void;
}

/** Something that can be disposed. Passed to `track` for mid-session cleanup. */
export interface Disposable {
  /** Release whatever this disposable holds. */
  dispose(): void;
}

/**
 * A callable leaf in the dispatch Proxy.
 *
 * Every property access on a server dispatcher extends the call path and
 * yields another `ServerDispatcher`; calling one invokes the resolved tool.
 */
export interface ServerDispatcher {
  /** Invoke the tool this path resolves to with a single arguments object. */
  (input?: Record<string, unknown>): Promise<unknown>;
  /** Extend the call path by one segment (server, tool, noun, or verb). */
  [segment: string]: ServerDispatcher;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/**
 * Raised when a plugin calls a server that is not registered.
 *
 * The dispatch Proxy itself does not know what is registered; this surfaces
 * when the host reports the server unknown at `tools/list` time.
 */
export class UnknownServer extends Error {
  /** Construct an `UnknownServer` for the named server. */
  constructor(server: string) {
    super(`unknown server '${server}'`);
    this.name = "UnknownServer";
  }
}

/**
 * Raised when an operation-tool path names a noun or verb the tool's `_meta`
 * does not describe.
 *
 * The message lists the valid verbs for the noun (or the valid nouns) so the
 * plugin author can see the correct spelling without leaving the error.
 */
export class UnknownOperation extends Error {
  /** Construct an `UnknownOperation` from a human-readable explanation. */
  constructor(message: string) {
    super(message);
    this.name = "UnknownOperation";
  }
}

// ---------------------------------------------------------------------------
// The host bridge
// ---------------------------------------------------------------------------

/** One operation's `_meta` entry: its wire `op` string and parameter schema. */
interface OperationMeta {
  /** The exact `op` selector string the wire call carries. */
  op: string;
  /** Optional human-readable description of the operation. */
  description?: string;
  /** The operation's parameter schema, by parameter name. */
  parameters?: Record<string, unknown>;
}

/** An operation tool's `_meta` tree: noun → verb → operation metadata. */
type OperationsMeta = Record<string, Record<string, OperationMeta>>;

/**
 * A tool's definition as returned by `tools/list`.
 *
 * A tool is an *operation tool* iff its `_meta` carries the
 * `io.swissarmyhammer/operations` key; otherwise it is a *flat tool*.
 */
interface ToolDefinition {
  /** The tool name addressed by a `tools/call`. */
  name: string;
  /** Optional human-readable description. */
  description?: string;
  /** The tool's input schema (unused by the dispatch Proxy itself). */
  inputSchema?: unknown;
  /**
   * MCP `_meta` extension map; operation tools carry their operations tree
   * under `io.swissarmyhammer/operations`.
   */
  _meta?: Record<string, unknown>;
}

/** The `_meta` key under which an operation tool carries its operations tree. */
const OPERATIONS_META_KEY = "io.swissarmyhammer/operations";

/**
 * The seam between the SDK and the host.
 *
 * Every method marshals a JSON envelope through `op_host_dispatch`. The
 * host's `HostDispatcher` reads the envelope's `kind` and answers it. A host
 * rejection surfaces here as a thrown exception from the op call.
 */
export interface Transport {
  /** Register an MCP server with the platform under `name`. */
  register(name: string, source: ServerSource): void;
  /** Unregister a previously registered server. */
  unregister(name: string): void;
  /**
   * Resolve a dispatch `path` against `server`'s cached tool definitions and
   * issue the corresponding `tools/call`.
   */
  callPath(
    server: string,
    path: string[],
    args: Record<string, unknown>,
  ): Promise<unknown>;
}

/**
 * The concrete `Transport` backed by the `op_host_dispatch` bridge op.
 *
 * `tools/list` results are cached per server: the first call to any tool on
 * a server fetches and caches that server's whole tool list, and later calls
 * resolve against the cache. An unknown server fails at `tools/list` time
 * with `UnknownServer`.
 */
class HostBridge implements Transport {
  /**
   * Cached `tools/list` results, keyed by server name. A value of `null`
   * records that the server is known-unknown so the failure is not re-fetched.
   */
  private readonly toolCache = new Map<string, ToolDefinition[] | null>();

  /**
   * Send one JSON envelope to the host and return its JSON response.
   *
   * Throws whatever the host rejects with; the caller maps that to a typed
   * SDK error where it can.
   */
  private dispatch(payload: Record<string, unknown>): unknown {
    return Deno.core.ops.op_host_dispatch(payload);
  }

  /** {@inheritDoc Transport.register} */
  register(name: string, source: ServerSource): void {
    this.dispatch({ kind: "register", name, source });
  }

  /** {@inheritDoc Transport.unregister} */
  unregister(name: string): void {
    this.dispatch({ kind: "unregister", name });
    // A re-registered server may expose a different tool set; drop any cache.
    this.toolCache.delete(name);
  }

  /**
   * Fetch (and cache) the tool definitions a server exposes.
   *
   * Returns the cached list on a hit. On a miss it issues a `tools/list`
   * over the bridge; a host rejection (an unknown server) is cached as a
   * negative entry and re-raised as `UnknownServer`.
   */
  private tools(server: string): ToolDefinition[] {
    const cached = this.toolCache.get(server);
    if (cached === null) throw new UnknownServer(server);
    if (cached !== undefined) return cached;

    let response: unknown;
    try {
      response = this.dispatch({ kind: "toolsList", server });
    } catch {
      // The host rejected `tools/list` — treat the server as unregistered.
      this.toolCache.set(server, null);
      throw new UnknownServer(server);
    }

    const tools = (response as { tools?: ToolDefinition[] })?.tools ?? [];
    this.toolCache.set(server, tools);
    return tools;
  }

  /** {@inheritDoc Transport.callPath} */
  callPath(
    server: string,
    path: string[],
    args: Record<string, unknown>,
  ): Promise<unknown> {
    // `callPath` is async by signature so a plugin always `await`s a leaf
    // call, but the resolution and the bridge op are synchronous. Any throw
    // is funneled into the returned promise via this wrapper.
    try {
      return Promise.resolve(this.dispatchPath(server, path, args));
    } catch (error) {
      return Promise.reject(error);
    }
  }

  /**
   * Resolve a dispatch path to a `tools/call` and issue it.
   *
   * `path[0]` is the tool. A flat tool dispatches `args` verbatim; an
   * operation tool folds an `op` string — either looked up from `_meta` for
   * a `[tool, noun, verb]` path, or already present in `args` for the direct
   * `[tool]` form — in flat alongside the rest of `args`.
   */
  private dispatchPath(
    server: string,
    path: string[],
    args: Record<string, unknown>,
  ): unknown {
    if (path.length === 0) {
      throw new UnknownOperation(
        `no tool named on server '${server}' — a call needs at least a tool`,
      );
    }
    const toolName = path[0];
    const tools = this.tools(server);
    const tool = tools.find((t) => t.name === toolName);
    if (tool === undefined) {
      throw new UnknownOperation(
        `server '${server}' has no tool '${toolName}'`,
      );
    }

    const operations = operationsOf(tool);
    if (operations === undefined) {
      // Flat tool: the path is just `[tool]` and `args` go through verbatim.
      if (path.length > 1) {
        throw new UnknownOperation(
          `tool '${toolName}' on server '${server}' is a flat tool; ` +
            `it has no noun/verb path '${path.slice(1).join(".")}'`,
        );
      }
      return this.toolsCall(server, toolName, args);
    }

    // Operation tool. The direct form is `[tool]` with `op` already in args.
    if (path.length === 1) {
      return this.toolsCall(server, toolName, args);
    }

    // Path form: exactly `[tool, noun, verb]`.
    if (path.length !== 3) {
      throw new UnknownOperation(
        `operation tool '${toolName}' on server '${server}' expects a ` +
          `noun.verb path; got '${path.slice(1).join(".")}'`,
      );
    }
    const [, noun, verb] = path;
    const op = lookupOp(server, toolName, operations, noun, verb);
    return this.toolsCall(server, toolName, { op, ...args });
  }

  /** Issue one `tools/call` over the bridge. */
  private toolsCall(
    server: string,
    tool: string,
    args: Record<string, unknown>,
  ): unknown {
    return this.dispatch({
      kind: "toolsCall",
      server,
      tool,
      arguments: args,
    });
  }
}

/**
 * Read a tool's `io.swissarmyhammer/operations` `_meta` tree, if it has one.
 *
 * Returns `undefined` for a flat tool (no operations key in `_meta`).
 */
function operationsOf(tool: ToolDefinition): OperationsMeta | undefined {
  const meta = tool._meta;
  if (meta === undefined || meta === null) return undefined;
  const ops = meta[OPERATIONS_META_KEY];
  if (ops === undefined || ops === null) return undefined;
  return ops as OperationsMeta;
}

/**
 * Look up the wire `op` string for a `noun.verb` pair in an operations tree.
 *
 * Throws `UnknownOperation` — with the valid verbs (or valid nouns) listed —
 * when the noun or verb is not described by the tool's `_meta`.
 */
function lookupOp(
  server: string,
  tool: string,
  operations: OperationsMeta,
  noun: string,
  verb: string,
): string {
  const verbs = operations[noun];
  if (verbs === undefined) {
    const validNouns = Object.keys(operations).sort().join(", ");
    throw new UnknownOperation(
      `operation tool '${tool}' on server '${server}' has no noun ` +
        `'${noun}'; valid nouns: ${validNouns}`,
    );
  }
  const entry = verbs[verb];
  if (entry === undefined || typeof entry.op !== "string") {
    const validVerbs = Object.keys(verbs).sort().join(", ");
    throw new UnknownOperation(
      `operation tool '${tool}' on server '${server}' has no verb ` +
        `'${verb}' for noun '${noun}'; valid verbs: ${validVerbs}`,
    );
  }
  return entry.op;
}

// ---------------------------------------------------------------------------
// The dispatch Proxy
// ---------------------------------------------------------------------------

/**
 * SDK-handled property names that are never forwarded as path segments.
 *
 * `on`/`off`/`once`/`subscribe`/`unsubscribe` are event-API names the SDK
 * reserves for itself. `then` is included so the dispatcher Proxy is never
 * mistaken for a thenable — without it, `await`ing a dispatcher would invoke
 * `.then` as if it extended the path.
 */
const RESERVED = new Set<string>([
  "on",
  "off",
  "once",
  "subscribe",
  "unsubscribe",
  "then",
]);

/**
 * The handler returned for a `RESERVED` name accessed on a dispatcher.
 *
 * The event surface (`on`, `subscribe`, …) is not part of this SDK task, so
 * the handler is an inert no-op: it exists only to keep a `RESERVED` name
 * from being treated as a tool/noun/verb segment. The callback/event
 * primitive is wired by a later task.
 */
function reservedHandler(): () => void {
  return () => {
    /* event API not implemented in this SDK task — intentionally inert */
  };
}

/**
 * Build a server dispatcher Proxy rooted at `server`, carrying `path`.
 *
 * The Proxy wraps a function so the value is both *callable* (invoking the
 * leaf issues `transport.callPath`) and *indexable* (every property access
 * extends `path` and yields a fresh dispatcher). A `RESERVED` name yields the
 * reserved handler instead of extending the path; `then` additionally is
 * reported absent so the dispatcher is not treated as a thenable.
 */
export function makeDispatcher(
  transport: Transport,
  server: string,
  path: string[] = [],
): ServerDispatcher {
  const leaf = (input?: Record<string, unknown>): Promise<unknown> =>
    transport.callPath(server, path, input ?? {});

  return new Proxy(leaf, {
    get(_target, prop): unknown {
      if (typeof prop !== "string") return undefined;
      // `then` must read as absent: an `await` probes `.then`, and a present
      // `.then` would make the dispatcher look like a promise to resolve.
      if (prop === "then") return undefined;
      if (RESERVED.has(prop)) return reservedHandler();
      return makeDispatcher(transport, server, [...path, prop]);
    },
  }) as unknown as ServerDispatcher;
}

/**
 * A `Plugin` wrapped for dispatch: the base surface plus the dynamic server
 * index, where any property name resolves to a {@link ServerDispatcher}.
 */
export type PluginThis<T extends Plugin> = T & Record<string, ServerDispatcher>;

/**
 * Wrap a `Plugin` instance so unknown property reads become server
 * dispatchers.
 *
 * A read of an own property or inherited method of the base instance passes
 * straight through — `load`, `register`, `log`, and the rest keep working.
 * Any other string property name is treated as a server name and yields a
 * dispatcher rooted at it, which is what makes `this.<server>.<tool>...`
 * work without `<server>` being declared anywhere.
 */
export function makePluginThis<T extends Plugin>(base: T): PluginThis<T> {
  const transport = base.__transport;
  return new Proxy(base, {
    get(target, prop, receiver): unknown {
      if (typeof prop !== "string") return Reflect.get(target, prop, receiver);
      // `then` is read during promise resolution; the plugin instance is not
      // a thenable, so report it absent rather than building a dispatcher.
      if (prop === "then") return undefined;
      if (prop in target) return Reflect.get(target, prop, receiver);
      return makeDispatcher(transport, prop);
    },
  }) as PluginThis<T>;
}

// ---------------------------------------------------------------------------
// The Plugin base class
// ---------------------------------------------------------------------------

/**
 * The default `Logger`, forwarding records to the host over the bridge.
 *
 * Each level marshals a `log` envelope through `op_host_dispatch`. A host
 * that has no logging sink yet simply ignores the envelope.
 */
function makeLogger(): Logger {
  const emit = (
    level: string,
    message: string,
    fields?: Record<string, unknown>,
  ): void => {
    try {
      Deno.core.ops.op_host_dispatch({
        kind: "log",
        level,
        message,
        fields: fields ?? {},
      });
    } catch {
      // Logging must never break a plugin: swallow a host-side rejection.
    }
  };
  return {
    debug: (m, f) => emit("debug", m, f),
    info: (m, f) => emit("info", m, f),
    warn: (m, f) => emit("warn", m, f),
    error: (m, f) => emit("error", m, f),
  };
}

/**
 * The base class every plugin subclasses.
 *
 * A plugin author overrides `load` / `unload` for lifecycle work and calls
 * `register` / `unregister` to point the platform at MCP servers. Tools on a
 * registered server are reached through the dynamic `[server]` index — an
 * access such as `this.kanban` is not a declared member, it is a server
 * dispatcher produced by `makePluginThis`.
 *
 * The base instance must be wrapped by `makePluginThis` before its `load` is
 * run, so that `this.<server>...` inside the plugin resolves to a dispatcher.
 */
export abstract class Plugin {
  /**
   * The host transport this plugin's calls cross through.
   *
   * Held so `makePluginThis` and the `register`/`unregister`/dispatch
   * methods share one bridge. It is an implementation detail of the SDK, not
   * part of the plugin author's surface.
   */
  readonly __transport: Transport = new HostBridge();

  /** A scoped logger. Records are forwarded to the host. */
  readonly log: Logger = makeLogger();

  /**
   * Disposables tracked for mid-session cleanup.
   *
   * The host auto-disposes every registration a plugin makes on unload;
   * `track` is a convenience for disposables created mid-session.
   */
  private readonly tracked: Disposable[] = [];

  /**
   * Optional lifecycle hook: run once when the plugin is loaded.
   *
   * The default is a no-op so a subclass need not override it. A subclass
   * that registers servers or sets up state does so here.
   */
  load(): Promise<void> {
    return Promise.resolve();
  }

  /**
   * Optional lifecycle hook: run once when the plugin is unloaded.
   *
   * The default disposes everything passed to `track`. A subclass that
   * overrides this should call `super.unload()` to keep that behavior.
   */
  unload(): Promise<void> {
    for (const disposable of this.tracked.splice(0)) {
      try {
        disposable.dispose();
      } catch {
        // One failing disposer must not abort the rest of teardown.
      }
    }
    return Promise.resolve();
  }

  /**
   * Point the platform at an MCP server, reachable afterward as
   * `this.<name>`.
   *
   * The plugin never describes the server's tools — the platform queries
   * them from the server itself via `tools/list`.
   *
   * @param name   - the registry key the server is reachable under.
   * @param source - where the server lives; see {@link ServerSource}.
   */
  register(name: string, source: ServerSource): void {
    this.__transport.register(name, source);
  }

  /**
   * Unregister a server previously registered with {@link register}.
   *
   * @param name - the registry key passed to `register`.
   */
  unregister(name: string): void {
    this.__transport.unregister(name);
  }

  /**
   * Track a disposable for cleanup at `unload` time.
   *
   * Returns the disposable unchanged so the call can wrap an expression.
   *
   * @param disposable - the disposable to dispose on unload.
   */
  track(disposable: Disposable): Disposable {
    this.tracked.push(disposable);
    return disposable;
  }
}
