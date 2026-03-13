import { describe, it, expect } from "vitest";
import { findMentionsInText } from "./mention-finder";

describe("findMentionsInText", () => {
  it("finds #tag mentions", () => {
    const hits = findMentionsInText("hello #bug world", "#", ["bug"]);
    expect(hits).toEqual([{ index: 6, length: 4, slug: "bug" }]);
  });

  it("finds @actor mentions", () => {
    const hits = findMentionsInText("assigned to @alice", "@", ["alice"]);
    expect(hits).toEqual([{ index: 12, length: 6, slug: "alice" }]);
  });

  it("finds multiple mentions with different prefixes independently", () => {
    const text = "fix #bug cc @alice";
    const tags = findMentionsInText(text, "#", ["bug"]);
    const actors = findMentionsInText(text, "@", ["alice"]);
    expect(tags).toEqual([{ index: 4, length: 4, slug: "bug" }]);
    expect(actors).toEqual([{ index: 12, length: 6, slug: "alice" }]);
  });

  it("respects word boundary before prefix", () => {
    const hits = findMentionsInText("foo#bug", "#", ["bug"]);
    expect(hits).toEqual([]);
  });

  it("respects boundary after slug", () => {
    const hits = findMentionsInText("#bugfix", "#", ["bug"]);
    expect(hits).toEqual([]);
  });

  it("matches at start of string", () => {
    const hits = findMentionsInText("#bug is bad", "#", ["bug"]);
    expect(hits).toEqual([{ index: 0, length: 4, slug: "bug" }]);
  });

  it("matches at end of string", () => {
    const hits = findMentionsInText("see #bug", "#", ["bug"]);
    expect(hits).toEqual([{ index: 4, length: 4, slug: "bug" }]);
  });

  it("allows prefix chars as boundary after slug", () => {
    // #bug #feature should match both (space-separated)
    const hits = findMentionsInText("#bug #feature", "#", ["bug", "feature"]);
    expect(hits).toHaveLength(2);
    expect(hits[0].slug).toBe("bug");
    expect(hits[1].slug).toBe("feature");
  });

  it("does not match when prefix is preceded by word char", () => {
    // #bug#feature — the second # is preceded by word char 'g'
    const hits = findMentionsInText("#bug#feature", "#", ["bug", "feature"]);
    expect(hits).toHaveLength(1);
    expect(hits[0].slug).toBe("bug");
  });

  it("allows @ as boundary after # slug", () => {
    const hits = findMentionsInText("#bug@alice", "#", ["bug"]);
    expect(hits).toEqual([{ index: 0, length: 4, slug: "bug" }]);
  });

  it("returns empty for no matches", () => {
    const hits = findMentionsInText("no mentions here", "@", ["alice"]);
    expect(hits).toEqual([]);
  });

  it("returns sorted by position", () => {
    const hits = findMentionsInText("#b then #a", "#", ["a", "b"]);
    expect(hits[0].slug).toBe("b");
    expect(hits[1].slug).toBe("a");
  });

  it("detects mention after Unicode punctuation (CJK bracket)", () => {
    const hits = findMentionsInText("\u300C#bug\u300D", "#", ["bug"]);
    expect(hits).toEqual([{ index: 1, length: 4, slug: "bug" }]);
  });

  it("detects mention after em-dash", () => {
    const hits = findMentionsInText("issue\u2014#bug", "#", ["bug"]);
    expect(hits).toEqual([{ index: 6, length: 4, slug: "bug" }]);
  });

  it("detects mention followed by Unicode punctuation", () => {
    const hits = findMentionsInText("#bug\u3002", "#", ["bug"]);
    expect(hits).toEqual([{ index: 0, length: 4, slug: "bug" }]);
  });

  it("detects mention followed by emoji", () => {
    const hits = findMentionsInText("#bug\uD83D\uDC1B", "#", ["bug"]);
    expect(hits).toEqual([{ index: 0, length: 4, slug: "bug" }]);
  });
});
