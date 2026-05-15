/**
 * Unit tests for the `EntityStoreProvider` field-subscription contract.
 *
 * The store exposes a single hook — `useFieldValue(entityType, id, fieldName)`
 * — that re-renders its caller whenever that specific field's value changes
 * in the entities map handed to the provider. This is the foundation of the
 * event-driven UI: cards, inspectors, and grid cells all subscribe through
 * this hook so they redraw locally when an `entity-field-changed` event
 * patches a single field, without prop-drilling fresh entities or refetching.
 *
 * The `FieldSubscriptions.diff` class is internal — tests exercise the
 * contract through `useFieldValue`, the public surface every consumer uses.
 */

import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useState, type ReactNode } from "react";
import { EntityStoreProvider, useFieldValue } from "./entity-store-context";
import type { Entity } from "@/types/kanban";

/**
 * Wrap a hook in `EntityStoreProvider` whose `entities` prop is driven by
 * local state. Tests call `setEntities` to advance the store and assert
 * that subscribers re-render exactly when they should.
 */
function makeStoreHarness(initial: Record<string, Entity[]>) {
  let setStateRef!: (next: Record<string, Entity[]>) => void;
  function Wrapper({ children }: { children: ReactNode }) {
    const [entities, setEntities] = useState(initial);
    setStateRef = setEntities;
    return (
      <EntityStoreProvider entities={entities}>{children}</EntityStoreProvider>
    );
  }
  return {
    wrapper: Wrapper,
    setEntities: (next: Record<string, Entity[]>) => {
      // Always go through `act` so React flushes the resulting commit
      // before the test reads the hook's `result.current`.
      act(() => setStateRef(next));
    },
  };
}

/** Build a minimal entity with the given field map. */
function makeEntity(
  entityType: string,
  id: string,
  fields: Record<string, unknown>,
): Entity {
  return {
    entity_type: entityType,
    id,
    moniker: `${entityType}:${id}`,
    fields,
  };
}

describe("EntityStoreProvider field subscriptions", () => {
  it("notifies useFieldValue when a non-hardcoded field changes on a non-task entity", async () => {
    const initial = {
      tag: [makeEntity("tag", "bug", { tag_name: "bug", color: "#ff0000" })],
    };
    const { wrapper, setEntities } = makeStoreHarness(initial);

    const { result } = renderHook(() => useFieldValue("tag", "bug", "color"), {
      wrapper,
    });

    expect(result.current).toBe("#ff0000");

    // Patch just the `color` field. The new entity reference must carry the
    // patched value; other tag entries (none here) stay identical.
    setEntities({
      tag: [makeEntity("tag", "bug", { tag_name: "bug", color: "#00ff00" })],
    });

    expect(result.current).toBe("#00ff00");
  });

  it("notifies useFieldValue when a field is added to an entity that previously omitted it", async () => {
    // Mirrors the bug scenario: a schema-defined field that wasn't set on the
    // entity yet — the inspector edit produces an event that adds the field
    // for the first time. Subscribers must observe the new value.
    const initial = {
      tag: [makeEntity("tag", "new", { tag_name: "new" })],
    };
    const { wrapper, setEntities } = makeStoreHarness(initial);

    const { result } = renderHook(
      () => useFieldValue("tag", "new", "description"),
      { wrapper },
    );

    expect(result.current).toBeUndefined();

    setEntities({
      tag: [
        makeEntity("tag", "new", {
          tag_name: "new",
          description: "Just added",
        }),
      ],
    });

    expect(result.current).toBe("Just added");
  });

  it("does NOT notify subscribers of an unrelated field on the same entity", async () => {
    // The diff must be field-scoped. A change to `color` must not re-render
    // a subscriber watching `tag_name` — the snapshot for `tag_name` is
    // value-equal across the two entities so useSyncExternalStore must
    // skip the re-render.
    const initial = {
      tag: [makeEntity("tag", "bug", { tag_name: "bug", color: "#ff0000" })],
    };
    const { wrapper, setEntities } = makeStoreHarness(initial);

    let renderCount = 0;
    renderHook(
      () => {
        renderCount += 1;
        return useFieldValue("tag", "bug", "tag_name");
      },
      { wrapper },
    );

    const initialRenders = renderCount;

    setEntities({
      tag: [makeEntity("tag", "bug", { tag_name: "bug", color: "#00ff00" })],
    });

    // The store updated, but the value we subscribe to (`tag_name`) is
    // value-equal across the two snapshots. useSyncExternalStore's
    // bail-out must keep render count steady.
    expect(renderCount).toBe(initialRenders);
  });

  it("notifies subscribers across different entity types in the same diff pass", async () => {
    // A single state update can carry changes for multiple entity types
    // (e.g. an event that patches a task AND a tag in one render). The
    // diff pass must visit every entity_type present in the new map and
    // fire each relevant subscriber. Both subscribers live inside the
    // same provider tree so they share one store instance.
    const initial = {
      tag: [makeEntity("tag", "bug", { color: "#ff0000" })],
      project: [makeEntity("project", "p1", { description: "initial" })],
    };
    const { wrapper, setEntities } = makeStoreHarness(initial);

    const { result } = renderHook(
      () => ({
        color: useFieldValue("tag", "bug", "color"),
        description: useFieldValue("project", "p1", "description"),
      }),
      { wrapper },
    );

    expect(result.current.color).toBe("#ff0000");
    expect(result.current.description).toBe("initial");

    setEntities({
      tag: [makeEntity("tag", "bug", { color: "#00ff00" })],
      project: [makeEntity("project", "p1", { description: "updated" })],
    });

    expect(result.current.color).toBe("#00ff00");
    expect(result.current.description).toBe("updated");
  });

  it("notifies subscribers when a field is removed", async () => {
    // Removal flows through the diff as a value transition from "set" to
    // `undefined`. Subscribers must see the new `undefined` value so e.g.
    // a card hides the row.
    const initial = {
      tag: [
        makeEntity("tag", "bug", {
          tag_name: "bug",
          description: "to be removed",
        }),
      ],
    };
    const { wrapper, setEntities } = makeStoreHarness(initial);

    const { result } = renderHook(
      () => useFieldValue("tag", "bug", "description"),
      { wrapper },
    );

    expect(result.current).toBe("to be removed");

    setEntities({
      tag: [makeEntity("tag", "bug", { tag_name: "bug" })],
    });

    expect(result.current).toBeUndefined();
  });
});
