/**
 * WebSocket-backed bidirectional message stream for the TypeScript ACP client.
 *
 * The kanban app runs its ACP agent in-process and serves it on a loopback
 * `ws://127.0.0.1:<port>` WebSocket (see `apps/kanban-app/src/ai/agent_ws.rs`).
 * This module is the webview's end of that wire: it opens a browser-native
 * `WebSocket` and adapts it into the {@link Stream} of ACP JSON-RPC messages
 * that `@agentclientprotocol/sdk`'s `ClientSideConnection` consumes.
 *
 * # No Tauri on the data path
 *
 * The only Tauri `invoke` involved is the one-time `ai_start_agent` call that
 * discovers the `ws://` URL — that happens elsewhere. The ACP messages
 * themselves travel exclusively over this WebSocket; Tauri IPC is never in the
 * data path.
 *
 * # Framing
 *
 * The Rust transport frames exactly one JSON-RPC message per WebSocket text
 * frame — no byte-level newline scanning. This module matches that contract:
 *
 * - **Outbound**: the SDK's {@link ndJsonStream} serializes each message to
 *   `JSON.stringify(msg) + "\n"` and hands it to the writable as one chunk;
 *   that chunk is sent verbatim as a single text frame. The trailing newline
 *   is inert — the Rust side parses the frame as one JSON value.
 * - **Inbound**: each text frame carries one complete JSON-RPC message.
 *   {@link ndJsonStream} splits the byte stream on `"\n"`, so a frame with no
 *   embedded newline yields exactly one parsed message.
 *
 * # Lifecycle
 *
 * The stream's lifetime is the socket's lifetime. A clean socket close ends
 * the readable (the SDK connection sees end-of-input); an error or unclean
 * close errors the readable so the failure surfaces instead of looking like a
 * silent end-of-stream. Closing the writable closes the socket.
 */
import { ndJsonStream, type Stream } from "@agentclientprotocol/sdk";

/**
 * The subset of the browser `WebSocket` API this module depends on.
 *
 * Declaring the dependency as a narrow interface — rather than the full
 * `WebSocket` type — lets tests inject an in-memory fake without constructing
 * a real socket. The global `WebSocket` structurally satisfies it.
 */
export interface AcpSocket {
  /** Connection state, mirroring `WebSocket.readyState` (1 === OPEN). */
  readonly readyState: number;
  /** Send one text frame. ACP traffic is always UTF-8 JSON. */
  send(data: string): void;
  /** Begin the closing handshake. */
  close(): void;
  addEventListener(type: string, listener: (event: unknown) => void): void;
  removeEventListener(type: string, listener: (event: unknown) => void): void;
}

/** Options for {@link connectAcpStream}, primarily a seam for testing. */
export interface ConnectAcpStreamOptions {
  /**
   * Factory for the underlying socket. Defaults to opening a real browser
   * `WebSocket` at `url`. Tests override this to supply an in-memory fake.
   */
  createSocket?: (url: string) => AcpSocket;
}

/**
 * A live ACP connection: the message {@link Stream} the SDK consumes plus an
 * explicit teardown handle.
 *
 * The `Stream`'s lifetime is the socket's lifetime — a socket close ends the
 * stream. {@link AcpConnection.close} drives that teardown deliberately (e.g.
 * when the AI panel is dismissed) by closing the underlying WebSocket, which
 * ends the readable so the SDK connection sees end-of-input.
 */
export interface AcpConnection {
  /** The bidirectional ACP message stream to hand to `ClientSideConnection`. */
  readonly stream: Stream;
  /** Close the underlying WebSocket, ending the stream. Idempotent. */
  close(): void;
}

/** `WebSocket.OPEN` — the numeric `readyState` of an open connection. */
const SOCKET_OPEN = 1;

/** Default socket factory: a real browser `WebSocket`. */
function openBrowserSocket(url: string): AcpSocket {
  return new WebSocket(url) as unknown as AcpSocket;
}

/**
 * Coerce one inbound `message` event's `data` to a newline-terminated UTF-8
 * byte chunk.
 *
 * The Rust transport sends exactly one JSON-RPC message per WebSocket frame
 * with no trailing delimiter, whereas {@link ndJsonStream}'s reader is a
 * *newline-delimited* JSON parser: it only emits a message once it sees a
 * `"\n"`. Appending `"\n"` to every frame here is the framing adapter — it
 * lets the SDK parser surface each frame as a message immediately instead of
 * buffering it until the socket closes.
 *
 * The agent always sends text frames, but a defensive path handles a
 * `Blob`/`ArrayBuffer` should the runtime ever deliver a binary frame.
 */
async function frameToBytes(data: unknown): Promise<Uint8Array> {
  const text =
    typeof data === "string"
      ? data
      : data instanceof ArrayBuffer
        ? new TextDecoder().decode(data)
        : data instanceof Blob
          ? new TextDecoder().decode(await data.arrayBuffer())
          : // Fall back to string coercion — keeps the parser fed rather than
            // throwing on an unexpected frame type the agent never sends.
            String(data);
  return new TextEncoder().encode(`${text}\n`);
}

/**
 * Build the readable byte stream of inbound WebSocket frames.
 *
 * Each `message` event enqueues one chunk. A clean `close` ends the stream;
 * an `error`, or a `close` with `wasClean === false`, errors it so the
 * failure is observable instead of indistinguishable from end-of-input.
 *
 * The three lifecycle listeners are paired with `removeEventListener`: they
 * are detached as soon as the stream finishes — whichever way it ends — and
 * the stream's `cancel` handler detaches them when a consumer cancels the
 * readable before the socket closes.
 */
function inboundByteStream(socket: AcpSocket): ReadableStream<Uint8Array> {
  // Shared between `start` and `cancel`. `start` runs synchronously during
  // construction and assigns `detach`; `cancel` runs later, if at all.
  let finished = false;
  let detach: () => void = () => {};

  return new ReadableStream<Uint8Array>({
    start(controller) {
      const onMessage = (event: unknown) => {
        if (finished) return;
        const data = (event as { data?: unknown }).data;
        void frameToBytes(data).then(
          (bytes) => {
            if (!finished) controller.enqueue(bytes);
          },
          (err) => {
            if (!finished) {
              finished = true;
              detach();
              controller.error(err);
            }
          },
        );
      };

      const onClose = (event: unknown) => {
        if (finished) return;
        finished = true;
        detach();
        const wasClean = (event as { wasClean?: boolean }).wasClean ?? true;
        if (wasClean) {
          controller.close();
        } else {
          controller.error(new Error("ACP WebSocket closed unexpectedly"));
        }
      };

      const onError = () => {
        if (finished) return;
        finished = true;
        detach();
        controller.error(new Error("ACP WebSocket connection error"));
      };

      socket.addEventListener("message", onMessage);
      socket.addEventListener("close", onClose);
      socket.addEventListener("error", onError);

      // Detaches all three lifecycle listeners. Invoked on every finish path
      // above, and by `cancel` when a consumer cancels the readable before
      // the socket closes — so every `addEventListener` has a matching
      // `removeEventListener`.
      detach = () => {
        socket.removeEventListener("message", onMessage);
        socket.removeEventListener("close", onClose);
        socket.removeEventListener("error", onError);
      };
    },
    cancel() {
      finished = true;
      detach();
    },
  });
}

/**
 * Build the writable byte stream that forwards chunks to the WebSocket.
 *
 * {@link ndJsonStream} writes one newline-terminated JSON-RPC message per
 * chunk; each chunk is decoded back to text and sent as a single frame,
 * matching the Rust transport's one-message-per-frame framing. The trailing
 * newline is inert: the Rust side parses each text frame as one JSON value.
 *
 * Teardown is socket-driven, not writable-driven — `ndJsonStream`'s writable
 * never forwards its own `close` here — so this stream deliberately has no
 * `close`/`abort` handler. Use {@link AcpConnection.close} to end the
 * connection.
 */
function outboundByteStream(socket: AcpSocket): WritableStream<Uint8Array> {
  const decoder = new TextDecoder();
  return new WritableStream<Uint8Array>({
    write(chunk) {
      if (socket.readyState !== SOCKET_OPEN) {
        throw new Error("ACP WebSocket is not open");
      }
      socket.send(decoder.decode(chunk));
    },
  });
}

/**
 * Open a WebSocket to the in-process ACP agent and adapt it into the
 * bidirectional {@link Stream} that `ClientSideConnection` consumes.
 *
 * The returned promise resolves once the socket is open. A WebSocket that
 * fails to connect always fires `close` (after an `error`), so a `close`
 * before the socket ever opened is the definitive "did not connect" signal:
 * the promise rejects then, rather than handing the SDK a dead stream.
 *
 * @param url - The agent's loopback `ws://127.0.0.1:<port>` URL, as returned
 *   by the `ai_start_agent` Tauri command.
 * @param options - Optional overrides; chiefly the socket factory used by
 *   tests to inject an in-memory fake.
 * @returns An {@link AcpConnection} — the ACP message `Stream` to pass to
 *   `new ClientSideConnection(toClient, stream)`, plus a `close` teardown
 *   handle.
 */
export function connectAcpStream(
  url: string,
  options: ConnectAcpStreamOptions = {},
): Promise<AcpConnection> {
  const createSocket = options.createSocket ?? openBrowserSocket;
  const socket = createSocket(url);

  return new Promise<AcpConnection>((resolve, reject) => {
    let settled = false;

    /**
     * Detach the `open`/`close` connect listeners. Both have served their
     * purpose once the promise settles — whichever fired — so every
     * `addEventListener` here is matched by a `removeEventListener`. The
     * inbound stream attaches its own fresh `close` listener afterwards.
     */
    const detachConnectListeners = () => {
      socket.removeEventListener("open", onOpen);
      socket.removeEventListener("close", onCloseBeforeOpen);
    };

    const onOpen = () => {
      if (settled) return;
      settled = true;
      detachConnectListeners();
      // ndJsonStream takes (output, input): the writable byte sink first,
      // the readable byte source second. It returns the message-level
      // Stream the SDK connection consumes.
      const stream = ndJsonStream(
        outboundByteStream(socket),
        inboundByteStream(socket),
      );
      resolve({
        stream,
        close: () => socket.close(),
      });
    };

    const onCloseBeforeOpen = () => {
      if (settled) return;
      settled = true;
      detachConnectListeners();
      reject(new Error("ACP WebSocket closed before it opened"));
    };

    socket.addEventListener("open", onOpen);
    // A failed connection fires `error` then `close`; `close` is the
    // definitive signal, so the rejection is wired there for one
    // deterministic failure path. The `error` event needs no separate
    // handler before open.
    socket.addEventListener("close", onCloseBeforeOpen);
  });
}
