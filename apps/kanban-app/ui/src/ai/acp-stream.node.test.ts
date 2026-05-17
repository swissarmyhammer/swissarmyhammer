/**
 * Round-trip tests for the WebSocket-backed ACP message stream.
 *
 * `acp-stream.ts` opens a browser-native `WebSocket` to the in-process ACP
 * agent and wraps it as the bidirectional `Stream` of ACP JSON-RPC messages
 * that the `@agentclientprotocol/sdk` `ClientSideConnection` consumes.
 *
 * These tests drive the wiring against an in-memory fake WebSocket so they
 * are deterministic and need no real socket. The fake mirrors exactly the
 * surface `acp-stream.ts` uses: `addEventListener` for the four lifecycle
 * events, `send`, `close`, and `readyState`. The Rust side
 * (`apps/kanban-app/src/ai/agent_ws.rs`) frames *one* JSON-RPC message per
 * WebSocket text frame, so each fake frame here carries exactly one message.
 *
 * Node-only (no DOM, no React) — pure stream-plumbing logic. Lives under the
 * `*.node.test.ts` suffix recognized by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import type { AnyMessage } from "@agentclientprotocol/sdk";
import { connectAcpStream, type AcpSocket } from "./acp-stream";

/**
 * An in-memory stand-in for the browser `WebSocket` that `acp-stream.ts`
 * opens. It implements only the members the module touches, and exposes test
 * hooks (`emitOpen`, `emitMessage`, `emitClose`, `emitError`, `sent`) to drive
 * and observe the wire from the test side.
 */
class FakeWebSocket implements AcpSocket {
  /** Mirrors `WebSocket.readyState`; starts CONNECTING. */
  readyState = 0;
  /** Outgoing text frames captured from `send`, in order. */
  readonly sent: string[] = [];
  /** Set when `close()` was called by the module under test. */
  closedByClient = false;

  private readonly listeners = new Map<string, Set<(ev: unknown) => void>>();

  addEventListener(type: string, listener: (ev: unknown) => void): void {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(listener);
  }

  removeEventListener(type: string, listener: (ev: unknown) => void): void {
    this.listeners.get(type)?.delete(listener);
  }

  send(data: string): void {
    this.sent.push(data);
  }

  close(): void {
    this.closedByClient = true;
    this.readyState = 3; // CLOSED
    this.emitClose(true);
  }

  private dispatch(type: string, ev: unknown): void {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(ev);
    }
  }

  /** Transition to OPEN and fire the `open` event. */
  emitOpen(): void {
    this.readyState = 1; // OPEN
    this.dispatch("open", {});
  }

  /** Deliver one inbound text frame (one JSON-RPC message). */
  emitMessage(text: string): void {
    this.dispatch("message", { data: text });
  }

  /** Fire the `close` event; `wasClean` defaults to a clean close. */
  emitClose(wasClean = true): void {
    this.readyState = 3; // CLOSED
    this.dispatch("close", { wasClean, code: wasClean ? 1000 : 1006 });
  }

  /** Fire the `error` event. */
  emitError(): void {
    this.dispatch("error", {});
  }

  /** Test hook: count of currently-registered listeners for an event type. */
  listenerCount(type: string): number {
    return this.listeners.get(type)?.size ?? 0;
  }
}

/** A representative ACP JSON-RPC request frame. */
const initializeRequest: AnyMessage = {
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: { protocolVersion: 1 },
} as AnyMessage;

/** A representative ACP JSON-RPC response frame. */
const initializeResponse: AnyMessage = {
  jsonrpc: "2.0",
  id: 1,
  result: { protocolVersion: 1 },
} as AnyMessage;

describe("connectAcpStream", () => {
  it("resolves with a Stream and close handle once the socket opens", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });

    fake.emitOpen();
    const connection = await connectionPromise;

    expect(connection.stream.readable).toBeInstanceOf(ReadableStream);
    expect(connection.stream.writable).toBeInstanceOf(WritableStream);
    expect(typeof connection.close).toBe("function");
  });

  it("rejects when the socket closes before it opens", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });

    // A failed WebSocket connection fires `error` then `close`.
    fake.emitError();
    fake.emitClose(false);

    await expect(connectionPromise).rejects.toThrow(/closed before it opened/i);
  });

  it("parses inbound ndjson frames into ACP messages", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });
    fake.emitOpen();
    const { stream } = await connectionPromise;

    const reader = stream.readable.getReader();
    // The Rust side sends exactly one JSON-RPC message per text frame, with
    // no trailing delimiter — the adapter must still surface it as a message.
    fake.emitMessage(JSON.stringify(initializeResponse));

    const { value, done } = await reader.read();
    expect(done).toBe(false);
    expect(value).toEqual(initializeResponse);
  });

  it("encodes outbound ACP messages as ndjson text frames on the wire", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });
    fake.emitOpen();
    const { stream } = await connectionPromise;

    const writer = stream.writable.getWriter();
    await writer.write(initializeRequest);

    expect(fake.sent).toHaveLength(1);
    // Each frame is a single newline-terminated JSON-RPC message — the
    // framing the Rust `agent_ws.rs` transport expects.
    expect(fake.sent[0].endsWith("\n")).toBe(true);
    expect(JSON.parse(fake.sent[0])).toEqual(initializeRequest);
  });

  it("round-trips a request out and a response back in", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });
    fake.emitOpen();
    const { stream } = await connectionPromise;

    const writer = stream.writable.getWriter();
    await writer.write(initializeRequest);
    expect(JSON.parse(fake.sent[0])).toEqual(initializeRequest);

    const reader = stream.readable.getReader();
    fake.emitMessage(JSON.stringify(initializeResponse));
    const { value } = await reader.read();
    expect(value).toEqual(initializeResponse);
  });

  it("ends the readable stream when the socket closes cleanly", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });
    fake.emitOpen();
    const { stream } = await connectionPromise;

    const reader = stream.readable.getReader();
    fake.emitClose(true);

    const { done } = await reader.read();
    expect(done).toBe(true);
  });

  it("errors the readable stream when the socket closes uncleanly", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });
    fake.emitOpen();
    const { stream } = await connectionPromise;

    const reader = stream.readable.getReader();
    // A failed connection fires `error` then an unclean `close`; either one
    // must error the readable rather than letting it look like a clean end.
    fake.emitError();
    fake.emitClose(false);

    await expect(reader.read()).rejects.toThrow(/ACP WebSocket/i);
  });

  it("detaches every socket listener once the stream finishes", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });

    fake.emitOpen();
    const { stream } = await connectionPromise;
    // The `open`/`close` connect listeners served their purpose at `open`.
    expect(fake.listenerCount("open")).toBe(0);

    const reader = stream.readable.getReader();
    fake.emitClose(true);
    const { done } = await reader.read();
    expect(done).toBe(true);

    // A clean close finishes the inbound stream; all of its listeners — and
    // the connect `close` listener — are detached, so nothing remains.
    expect(fake.listenerCount("message")).toBe(0);
    expect(fake.listenerCount("close")).toBe(0);
    expect(fake.listenerCount("error")).toBe(0);
  });

  it("detaches inbound listeners when the socket closes uncleanly", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });
    fake.emitOpen();
    const { stream } = await connectionPromise;

    const reader = stream.readable.getReader();
    fake.emitError();
    fake.emitClose(false);
    await expect(reader.read()).rejects.toThrow(/ACP WebSocket/i);

    // An error finish path detaches the inbound listeners just like a clean
    // close — every `addEventListener` is matched on every finish path.
    expect(fake.listenerCount("message")).toBe(0);
    expect(fake.listenerCount("close")).toBe(0);
    expect(fake.listenerCount("error")).toBe(0);
  });

  it("closing the connection closes the socket and ends the stream", async () => {
    const fake = new FakeWebSocket();
    const connectionPromise = connectAcpStream("ws://127.0.0.1:9/", {
      createSocket: () => fake,
    });
    fake.emitOpen();
    const connection = await connectionPromise;

    const reader = connection.stream.readable.getReader();
    connection.close();

    expect(fake.closedByClient).toBe(true);
    // The socket close ends the readable: the SDK connection sees end-of-input.
    const { done } = await reader.read();
    expect(done).toBe(true);
  });
});
