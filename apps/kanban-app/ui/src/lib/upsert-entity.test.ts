// @vitest-environment node
import { describe, it, expect } from "vitest";
import type { Entity } from "@/types/kanban";
import { upsertEntity } from "./upsert-entity";

describe("upsertEntity", () => {
  const existing: Entity[] = [
    {
      entity_type: "task",
      id: "aaa",
      moniker: "task:aaa",
      fields: { title: "A" },
    },
    {
      entity_type: "task",
      id: "bbb",
      moniker: "task:bbb",
      fields: { title: "B" },
    },
  ];

  it("replaces an entity already in the list", () => {
    const updated: Entity = {
      entity_type: "task",
      id: "aaa",
      moniker: "task:aaa",
      fields: { title: "A-updated" },
    };
    const result = upsertEntity(existing, updated);
    expect(result).toHaveLength(2);
    expect(result.find((e) => e.id === "aaa")?.fields.title).toBe("A-updated");
  });

  it("appends an entity not yet in the list (race condition recovery)", () => {
    const newEntity: Entity = {
      entity_type: "task",
      id: "ccc",
      moniker: "task:ccc",
      fields: { title: "C" },
    };
    const result = upsertEntity(existing, newEntity);
    expect(result).toHaveLength(3);
    expect(result[2]).toEqual(newEntity);
  });

  it("does not mutate the original array", () => {
    const updated: Entity = {
      entity_type: "task",
      id: "aaa",
      moniker: "task:aaa",
      fields: { title: "A-updated" },
    };
    const result = upsertEntity(existing, updated);
    expect(result).not.toBe(existing);
    expect(existing[0].fields.title).toBe("A");
  });
});
