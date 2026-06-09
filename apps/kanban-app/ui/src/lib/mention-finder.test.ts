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

describe("findMentionsInText \u2014 ^ short-id shape matching", () => {
  // The `^` prefix matches task references by SHAPE \u2014 `^` followed by exactly
  // 7 or exactly 26 Crockford-base32 chars \u2014 rather than by enumerating known
  // slugs. A 26-char full ULID normalizes to its lowercased last-7 slug; the
  // returned `length` always spans the full matched token. The `slugs`
  // argument is unused for `^` (shape, not enumeration), so an empty list
  // still produces hits.

  // Full ULID and its true canonical short id (lowercased last-7).
  const ULID = "01KT4CNAYW7JG0X8F8W28RFP1R";
  const SHORT = "28rfp1r"; // ULID.slice(-7).toLowerCase()

  it("matches a bare 7-char short id by shape", () => {
    const hits = findMentionsInText(`see ^${SHORT} now`, "^", []);
    expect(hits).toEqual([{ index: 4, length: 8, slug: SHORT }]);
  });

  it("matches a full 26-char ULID and normalizes the slug to its last 7", () => {
    const hits = findMentionsInText(`ref ^${ULID} end`, "^", []);
    expect(hits).toEqual([{ index: 4, length: 27, slug: SHORT }]);
  });

  it("lowercases the normalized slug from an uppercase ULID", () => {
    const hits = findMentionsInText(`^${ULID}`, "^", []);
    expect(hits).toHaveLength(1);
    expect(hits[0].slug).toBe(SHORT);
    expect(hits[0].length).toBe(27);
  });

  it("lowercases the slug from an uppercase 7-char short id", () => {
    const hits = findMentionsInText(`^${SHORT.toUpperCase()}`, "^", []);
    expect(hits).toEqual([{ index: 0, length: 8, slug: SHORT }]);
  });

  it("prefers the 26-char form over a 7-char prefix (longest-first)", () => {
    // The 26-char ULID must be consumed as one token, not as `^` + first 7.
    const hits = findMentionsInText(`^${ULID}`, "^", []);
    expect(hits).toHaveLength(1);
    expect(hits[0].length).toBe(27);
    expect(hits[0].slug).toBe(SHORT);
  });

  it("does not match Crockford runs of other lengths", () => {
    // 6 and 8 chars are neither a short id (7) nor a full ULID (26).
    expect(findMentionsInText("^abcdef", "^", [])).toEqual([]);
    expect(findMentionsInText("^abcdefgh", "^", [])).toEqual([]);
  });

  it("does not match non-Crockford characters (I, L, O, U excluded)", () => {
    // `i`, `l`, `o`, `u` are not in the Crockford alphabet.
    expect(findMentionsInText("^abcdeio", "^", [])).toEqual([]);
    expect(findMentionsInText("^iloiloU", "^", [])).toEqual([]);
  });

  it("respects the word boundary before the caret", () => {
    const hits = findMentionsInText(`foo^${SHORT}`, "^", []);
    expect(hits).toEqual([]);
  });

  it("respects the boundary after the short id", () => {
    // A trailing word char extends the run past 7 chars \u2192 no 7-char match.
    const hits = findMentionsInText(`^${SHORT}x`, "^", []);
    expect(hits).toEqual([]);
  });

  it("matches a short id at the end of the string", () => {
    const hits = findMentionsInText(`done ^${SHORT}`, "^", []);
    expect(hits).toEqual([{ index: 5, length: 8, slug: SHORT }]);
  });

  it("finds multiple short-id references sorted by position", () => {
    const hits = findMentionsInText("^aaaaaaa and ^bbbbbbb", "^", []);
    expect(hits).toHaveLength(2);
    expect(hits[0].slug).toBe("aaaaaaa");
    expect(hits[1].slug).toBe("bbbbbbb");
    expect(hits[0].index).toBeLessThan(hits[1].index);
  });
});
