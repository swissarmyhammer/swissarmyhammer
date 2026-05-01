/**
 * Tests for the branded spatial types in `spatial.ts`.
 *
 * Brand checks fall into two camps:
 *
 *  - Runtime — the brand helpers should produce the same primitive value
 *    they were handed (brands erase to plain strings/numbers at runtime, so
 *    the wire shape stays identical to what the Rust side expects).
 *  - Compile-time — a plain `string` or `number` cannot be assigned to a
 *    branded slot. We assert that with `// @ts-expect-error` markers; if
 *    TypeScript ever stops flagging the bad call, the file will fail
 *    `tsc --noEmit` and the test build with it.
 *
 * The path-monikers refactor (card `01KQD6064G1C1RAXDFPJVT1F46`) collapsed
 * the legacy `FullyQualifiedMoniker` (UUID) and flat `SegmentMoniker` types into a single
 * fully-qualified path. Callers declare a relative `SegmentMoniker` per
 * primitive; the FQM is composed via context. The two newtypes are
 * deliberately distinct — a `SegmentMoniker` cannot be passed where a
 * `FullyQualifiedMoniker` is expected.
 */

import { describe, it, expect } from "vitest";
import {
  asFq,
  asLayerName,
  asSegment,
  asPixels,
  asWindowLabel,
  composeFq,
  fqLastSegment,
  fqRoot,
  type FullyQualifiedMoniker,
  type LayerName,
  type Pixels,
  type Rect,
  type SegmentMoniker,
  type WindowLabel,
} from "./spatial";

describe("brand helpers", () => {
  it("preserve the underlying primitive value", () => {
    expect(asWindowLabel("main")).toBe("main");
    expect(asSegment("field:T1.title")).toBe("field:T1.title");
    expect(asFq("/window/inspector/field:T1.title")).toBe(
      "/window/inspector/field:T1.title",
    );
    expect(asSegment("window")).toBe("window");
    expect(asPixels(42)).toBe(42);
  });

  it("flow through the type system as their branded shape", () => {
    // Round-trip: a branded value can be passed straight into a function
    // that accepts that brand without re-tagging.
    const seg: SegmentMoniker = asSegment("inspector");
    const echoSeg = (s: SegmentMoniker): SegmentMoniker => s;
    expect(echoSeg(seg)).toBe("inspector");

    const fq: FullyQualifiedMoniker = asFq("/window");
    const echoFq = (f: FullyQualifiedMoniker): FullyQualifiedMoniker => f;
    expect(echoFq(fq)).toBe("/window");

    const w: WindowLabel = asWindowLabel("main");
    const echoWindow = (wl: WindowLabel): WindowLabel => wl;
    expect(echoWindow(w)).toBe("main");

    const ln: LayerName = asLayerName("window");
    const echoLayerName = (n: LayerName): LayerName => n;
    expect(echoLayerName(ln)).toBe("window");

    const px: Pixels = asPixels(10);
    const echoPixels = (p: Pixels): Pixels => p;
    expect(echoPixels(px)).toBe(10);
  });

  it("build a Rect record with branded pixel values", () => {
    const rect: Rect = {
      x: asPixels(0),
      y: asPixels(0),
      width: asPixels(100),
      height: asPixels(50),
    };
    expect(rect).toEqual({ x: 0, y: 0, width: 100, height: 50 });
  });
});

describe("FQM composition", () => {
  it("fqRoot prefixes a separator", () => {
    const window = fqRoot(asSegment("window"));
    expect(window).toBe("/window");
  });

  it("composeFq appends a segment with a single separator", () => {
    const window = fqRoot(asSegment("window"));
    const inspector = composeFq(window, asSegment("inspector"));
    expect(inspector).toBe("/window/inspector");

    const field = composeFq(inspector, asSegment("field:T1.title"));
    expect(field).toBe("/window/inspector/field:T1.title");
  });

  it("two zones with the same segment under different parents have distinct FQMs", () => {
    const window = fqRoot(asSegment("window"));
    const inspector = composeFq(window, asSegment("inspector"));
    const board = composeFq(window, asSegment("board"));
    const card = composeFq(board, asSegment("card:T1"));

    const inspectorField = composeFq(inspector, asSegment("field:T1.title"));
    const boardField = composeFq(card, asSegment("field:T1.title"));

    expect(inspectorField).not.toBe(boardField);
    expect(inspectorField).toBe("/window/inspector/field:T1.title");
    expect(boardField).toBe("/window/board/card:T1/field:T1.title");
  });

  it("fqLastSegment reads the trailing path segment", () => {
    const fq = asFq("/window/inspector/field:T1.title");
    expect(fqLastSegment(fq)).toBe("field:T1.title");

    const root = asFq("/window");
    expect(fqLastSegment(root)).toBe("window");
  });
});

describe("brand boundaries (compile-time)", () => {
  it("rejects a raw string where a SegmentMoniker is expected", () => {
    const expectSeg = (_: SegmentMoniker) => {};
    // @ts-expect-error — raw strings cannot stand in for SegmentMoniker
    expectSeg("not-branded");
    // Wrapping in the brand helper compiles without an error.
    expectSeg(asSegment("ok"));
  });

  it("rejects a raw string where a FullyQualifiedMoniker is expected", () => {
    const expectFq = (_: FullyQualifiedMoniker) => {};
    // @ts-expect-error — raw strings cannot stand in for FullyQualifiedMoniker
    expectFq("/window/inspector");
    expectFq(asFq("/window/inspector"));
  });

  it("rejects a raw string where a LayerName is expected", () => {
    const expectLayerName = (_: LayerName) => {};
    // @ts-expect-error — raw strings cannot stand in for LayerName
    expectLayerName("not-branded");
    expectLayerName(asLayerName("window"));
  });

  it("rejects a raw number where Pixels is expected", () => {
    const expectPx = (_: Pixels) => {};
    // @ts-expect-error — raw numbers cannot stand in for Pixels
    expectPx(123);
    expectPx(asPixels(123));
  });

  it("rejects passing a SegmentMoniker where a FullyQualifiedMoniker is expected", () => {
    const expectFq = (_: FullyQualifiedMoniker) => {};
    const seg: SegmentMoniker = asSegment("field:T1.title");
    // @ts-expect-error — SegmentMoniker is not assignable to FullyQualifiedMoniker
    // even though both are string-shaped. This is the safety net the
    // path-monikers refactor relies on.
    expectFq(seg);
  });

  it("rejects passing a FullyQualifiedMoniker where a SegmentMoniker is expected", () => {
    const expectSeg = (_: SegmentMoniker) => {};
    const fq: FullyQualifiedMoniker = asFq("/window/inspector");
    // @ts-expect-error — FullyQualifiedMoniker is not assignable to SegmentMoniker
    expectSeg(fq);
  });
});
