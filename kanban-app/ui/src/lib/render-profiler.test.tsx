import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import { useState } from "react";

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { warn } from "@tauri-apps/plugin-log";
import { RenderProfiler } from "./render-profiler";

const mockedWarn = warn as unknown as ReturnType<typeof vi.fn>;

beforeEach(() => {
  mockedWarn.mockClear();
});

/* ---------- children rendering / transparency ---------- */

describe("RenderProfiler children", () => {
  it("renders children unchanged", () => {
    const { container } = render(
      <RenderProfiler id="t">
        <div data-testid="child">hello</div>
      </RenderProfiler>,
    );
    const el = container.querySelector('[data-testid="child"]');
    expect(el).not.toBeNull();
    expect(el?.textContent).toBe("hello");
  });

  it("produces identical DOM with and without the wrapper", () => {
    const tree = (
      <section>
        <h1>Title</h1>
        <p>Body</p>
      </section>
    );
    const { container: plain } = render(tree);
    const { container: wrapped } = render(
      <RenderProfiler id="t">{tree}</RenderProfiler>,
    );
    expect(wrapped.innerHTML).toBe(plain.innerHTML);
  });
});

/* ---------- mount + update logging ---------- */

describe("RenderProfiler logging", () => {
  it("logs a mount line with counters starting at m=1 u=0", () => {
    render(
      <RenderProfiler id="t">
        <div>hi</div>
      </RenderProfiler>,
    );

    // At least one mount-phase call should have been made
    const mountCalls = mockedWarn.mock.calls.filter(
      (args) =>
        typeof args[0] === "string" && args[0].startsWith("[profile] t mount"),
    );
    expect(mountCalls.length).toBeGreaterThanOrEqual(1);
    // First mount call should carry m=1 u=0 n=0
    expect(mountCalls[0][0]).toMatch(/m=1 u=0 n=0/);
  });

  it("logs an update line after a re-render with u incremented", () => {
    // Drive a state change on a child so React commits an update inside the wrapped subtree.
    let setN: (n: number) => void = () => {};
    function Counter() {
      const [n, s] = useState(0);
      setN = s;
      return <div>{n}</div>;
    }

    render(
      <RenderProfiler id="t">
        <Counter />
      </RenderProfiler>,
    );

    mockedWarn.mockClear();

    act(() => {
      setN(1);
    });

    const updateCalls = mockedWarn.mock.calls.filter(
      (args) =>
        typeof args[0] === "string" && args[0].startsWith("[profile] t update"),
    );
    expect(updateCalls.length).toBeGreaterThanOrEqual(1);
    // Counter advanced: m=1, u>=1
    expect(updateCalls[0][0]).toMatch(/m=1 u=[1-9]\d* n=/);
  });

  it("logged string contains id, phase, ms, and aggregate counters", () => {
    render(
      <RenderProfiler id="my-id">
        <div>hi</div>
      </RenderProfiler>,
    );
    const mountCall = mockedWarn.mock.calls.find(
      (args) =>
        typeof args[0] === "string" &&
        args[0].startsWith("[profile] my-id mount"),
    );
    expect(mountCall).toBeDefined();
    const line = mountCall![0] as string;
    // Format: [profile] <id> <phase> <ms>ms (m=<n> u=<n> n=<n> total=<n>ms max=<n>ms)
    expect(line).toMatch(
      /^\[profile\] my-id mount \d+(\.\d+)?ms \(m=\d+ u=\d+ n=\d+ total=\d+ms max=\d+(\.\d+)?ms\)$/,
    );
  });
});

/* ---------- minDurationMs suppression ---------- */

describe("RenderProfiler minDurationMs", () => {
  /**
   * React.Profiler supplies `actualDuration` itself; we cannot force it to a
   * specific value from the outside. To verify the threshold gate, we set a
   * very large `minDurationMs` that any realistic render stays under — no
   * log calls should be emitted.
   */
  it("suppresses log calls when commit duration is below the threshold", () => {
    render(
      <RenderProfiler id="t" minDurationMs={10_000}>
        <div>hi</div>
      </RenderProfiler>,
    );
    const tCalls = mockedWarn.mock.calls.filter(
      (args) =>
        typeof args[0] === "string" && args[0].startsWith("[profile] t "),
    );
    expect(tCalls.length).toBe(0);
  });
});

/* ---------- mountsOnly gate ---------- */

describe("RenderProfiler mountsOnly", () => {
  it("logs mount but not update commits when mountsOnly is true", () => {
    let setN: (n: number) => void = () => {};
    function Counter() {
      const [n, s] = useState(0);
      setN = s;
      return <div>{n}</div>;
    }

    render(
      <RenderProfiler id="t" mountsOnly>
        <Counter />
      </RenderProfiler>,
    );

    // Mount was logged
    const mountCalls = mockedWarn.mock.calls.filter(
      (args) =>
        typeof args[0] === "string" && args[0].startsWith("[profile] t mount"),
    );
    expect(mountCalls.length).toBeGreaterThanOrEqual(1);

    mockedWarn.mockClear();

    act(() => {
      setN(1);
    });

    // Update must NOT be logged — but the counter still advanced internally,
    // so subsequent mount/nested commits (if any) would carry u>=1. We only
    // assert no update line was emitted.
    const updateCalls = mockedWarn.mock.calls.filter(
      (args) =>
        typeof args[0] === "string" && args[0].startsWith("[profile] t update"),
    );
    expect(updateCalls.length).toBe(0);
  });
});

/* ---------- counters accumulate across commits ---------- */

describe("RenderProfiler counter accumulation", () => {
  it("u counter advances monotonically across updates", () => {
    let setN: (n: number) => void = () => {};
    function Counter() {
      const [n, s] = useState(0);
      setN = s;
      return <div>{n}</div>;
    }

    render(
      <RenderProfiler id="t">
        <Counter />
      </RenderProfiler>,
    );

    mockedWarn.mockClear();

    act(() => {
      setN(1);
    });
    act(() => {
      setN(2);
    });

    const updateLines = mockedWarn.mock.calls
      .map((args) => args[0])
      .filter(
        (s): s is string =>
          typeof s === "string" && s.startsWith("[profile] t update"),
      );

    expect(updateLines.length).toBeGreaterThanOrEqual(2);

    const extractU = (s: string) => {
      const m = s.match(/u=(\d+)/);
      return m ? Number(m[1]) : -1;
    };
    const uValues = updateLines.map(extractU);
    // Strictly increasing
    for (let i = 1; i < uValues.length; i++) {
      expect(uValues[i]).toBeGreaterThan(uValues[i - 1]);
    }
  });
});
