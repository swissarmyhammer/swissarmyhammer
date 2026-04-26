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
 */

import { describe, it, expect } from "vitest";
import {
  asLayerKey,
  asLayerName,
  asMoniker,
  asPixels,
  asSpatialKey,
  asWindowLabel,
  type LayerKey,
  type LayerName,
  type Moniker,
  type Pixels,
  type Rect,
  type SpatialKey,
  type WindowLabel,
} from "./spatial";

describe("brand helpers", () => {
  it("preserve the underlying primitive value", () => {
    expect(asWindowLabel("main")).toBe("main");
    expect(asSpatialKey("k1")).toBe("k1");
    expect(asLayerKey("L1")).toBe("L1");
    expect(asMoniker("task:01ABC")).toBe("task:01ABC");
    expect(asLayerName("window")).toBe("window");
    expect(asPixels(42)).toBe(42);
  });

  it("flow through the type system as their branded shape", () => {
    // Round-trip: a branded value can be passed straight into a function
    // that accepts that brand without re-tagging.
    const k: SpatialKey = asSpatialKey("k");
    const echo = (key: SpatialKey): SpatialKey => key;
    expect(echo(k)).toBe("k");

    const layer: LayerKey = asLayerKey("L");
    const echoLayer = (lk: LayerKey): LayerKey => lk;
    expect(echoLayer(layer)).toBe("L");

    const w: WindowLabel = asWindowLabel("main");
    const echoWindow = (wl: WindowLabel): WindowLabel => wl;
    expect(echoWindow(w)).toBe("main");

    const m: Moniker = asMoniker("task:1");
    const echoMoniker = (mk: Moniker): Moniker => mk;
    expect(echoMoniker(m)).toBe("task:1");

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

describe("brand boundaries (compile-time)", () => {
  it("rejects a raw string where a SpatialKey is expected", () => {
    const expectKey = (_: SpatialKey) => {};
    // @ts-expect-error — raw strings cannot stand in for SpatialKey
    expectKey("not-branded");
    // Wrapping in the brand helper compiles without an error.
    expectKey(asSpatialKey("ok"));
  });

  it("rejects a raw string where a LayerKey is expected", () => {
    const expectLayer = (_: LayerKey) => {};
    // @ts-expect-error — raw strings cannot stand in for LayerKey
    expectLayer("not-branded");
    expectLayer(asLayerKey("ok"));
  });

  it("rejects a raw string where a Moniker is expected", () => {
    const expectMoniker = (_: Moniker) => {};
    // @ts-expect-error — raw strings cannot stand in for Moniker
    expectMoniker("not-branded");
    expectMoniker(asMoniker("task:1"));
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

  it("rejects mixing brands of the same underlying type", () => {
    const expectKey = (_: SpatialKey) => {};
    const layer: LayerKey = asLayerKey("L");
    // @ts-expect-error — LayerKey is not assignable to SpatialKey even though
    // both are string-shaped
    expectKey(layer);
  });
});
