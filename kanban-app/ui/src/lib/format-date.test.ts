/**
 * Tests for the shared date-formatting helpers.
 *
 * Every test pins `now` to a fixed `DateTime` (in UTC) so assertions are
 * deterministic regardless of when the suite runs. Two exported helpers are
 * covered:
 *
 * - `formatDateForDisplay` — full-sentence rendering used by {@link DateDisplay}
 *   (calendar-aware, falls through to a short/medium locale string for far
 *   past/future).
 * - `formatRelativeMagnitude` — magnitude-only string used by the caller to
 *   compose sentences like `"Completed 2 days ago"` /
 *   `"Scheduled in 3 weeks"` / `"Overdue by 5 days"`. Its bucket boundaries
 *   and pluralization rules must exactly match the hand-rolled helper this
 *   replaces in `status-date-display.tsx`, because the existing suite of
 *   status-date tests expects those strings unchanged.
 */

import { describe, it, expect } from "vitest";
import { DateTime } from "luxon";

import { formatDateForDisplay, formatRelativeMagnitude } from "./format-date";

/**
 * Fixed "now" for every test — 2026-04-12 15:30 UTC. Chosen to sit mid-year
 * so far-past values naturally cross year boundaries.
 */
const NOW = DateTime.fromISO("2026-04-12T15:30:00Z", { zone: "utc" });

describe("formatDateForDisplay", () => {
  describe("bare YYYY-MM-DD dates (calendar bucket)", () => {
    it('renders "today" for the same calendar day', () => {
      expect(formatDateForDisplay("2026-04-12", NOW)).toBe("today");
    });

    it('renders "yesterday" for one day earlier', () => {
      expect(formatDateForDisplay("2026-04-11", NOW)).toBe("yesterday");
    });

    it('renders "tomorrow" for one day later', () => {
      expect(formatDateForDisplay("2026-04-13", NOW)).toBe("tomorrow");
    });

    it('renders "N days ago" for a few days past', () => {
      expect(formatDateForDisplay("2026-04-09", NOW)).toBe("3 days ago");
    });

    it('renders "in N days" for a few days future', () => {
      expect(formatDateForDisplay("2026-04-15", NOW)).toBe("in 3 days");
    });

    it("never uses the sub-day relative form for date-only inputs", () => {
      // A bare date on the same day as `now` must still round up to "today",
      // never "3 hours ago" or similar. The afternoon `now` exercises the
      // case where midnight UTC is ~15 hours earlier.
      const result = formatDateForDisplay("2026-04-12", NOW);
      expect(result).toBe("today");
      expect(result).not.toMatch(/hour|minute/i);
    });
  });

  describe("ISO datetime timestamps (sub-day relative)", () => {
    it('renders "N hours ago" for a few hours past', () => {
      const now = DateTime.fromISO("2026-04-12T18:30:00Z", { zone: "utc" });
      expect(formatDateForDisplay("2026-04-12T15:30:00Z", now)).toBe(
        "3 hours ago",
      );
    });

    it('renders "in N hours" for a few hours future', () => {
      const now = DateTime.fromISO("2026-04-12T12:30:00Z", { zone: "utc" });
      expect(formatDateForDisplay("2026-04-12T15:30:00Z", now)).toBe(
        "in 3 hours",
      );
    });

    it('renders "N minutes ago" for sub-hour past', () => {
      const now = DateTime.fromISO("2026-04-12T16:15:00Z", { zone: "utc" });
      expect(formatDateForDisplay("2026-04-12T15:30:00Z", now)).toBe(
        "45 minutes ago",
      );
    });
  });

  describe("ISO datetime timestamps (calendar bucket)", () => {
    it('renders "2 days ago" for ~48h past', () => {
      const now = DateTime.fromISO("2026-04-14T15:30:00Z", { zone: "utc" });
      expect(formatDateForDisplay("2026-04-12T15:30:00Z", now)).toBe(
        "2 days ago",
      );
    });
  });

  describe("far past/future (localized date string)", () => {
    it("shows year for dates in a different year", () => {
      expect(formatDateForDisplay("2025-10-15", NOW)).toBe("Oct 15, 2025");
    });

    it("omits year for same-year dates that are far from now", () => {
      // "2026-01-01" vs NOW (2026-04-12) is ~100 days apart — well outside
      // the calendar bucket — but still within the current year.
      expect(formatDateForDisplay("2026-01-01", NOW)).toBe("Jan 1");
    });
  });

  describe("fail-safe behaviour", () => {
    it("returns the original string on unparseable input", () => {
      expect(formatDateForDisplay("not-a-date", NOW)).toBe("not-a-date");
      expect(formatDateForDisplay("", NOW)).toBe("");
      expect(formatDateForDisplay("2026-99-99", NOW)).toBe("2026-99-99");
    });
  });

  describe("default `now`", () => {
    it("uses the current clock when `now` is omitted", () => {
      // Exact output is time-dependent, but the result should always be a
      // non-empty string for a well-formed input.
      const result = formatDateForDisplay(DateTime.now().toISO()!);
      expect(typeof result).toBe("string");
      expect(result.length).toBeGreaterThan(0);
    });
  });
});

describe("formatRelativeMagnitude", () => {
  describe("bucket boundaries", () => {
    it('returns "just now" for sub-minute deltas', () => {
      // 30 seconds in the past
      const ts = NOW.minus({ seconds: 30 }).toISO()!;
      expect(formatRelativeMagnitude(ts, NOW)).toBe("just now");
    });

    it("returns singular/plural minutes", () => {
      expect(
        formatRelativeMagnitude(NOW.minus({ minutes: 1 }).toISO()!, NOW),
      ).toBe("1 minute");
      expect(
        formatRelativeMagnitude(NOW.minus({ minutes: 5 }).toISO()!, NOW),
      ).toBe("5 minutes");
      expect(
        formatRelativeMagnitude(NOW.minus({ minutes: 59 }).toISO()!, NOW),
      ).toBe("59 minutes");
    });

    it("returns singular/plural hours", () => {
      expect(
        formatRelativeMagnitude(NOW.minus({ hours: 1 }).toISO()!, NOW),
      ).toBe("1 hour");
      expect(
        formatRelativeMagnitude(NOW.minus({ hours: 3 }).toISO()!, NOW),
      ).toBe("3 hours");
    });

    it("returns singular/plural days", () => {
      expect(
        formatRelativeMagnitude(NOW.minus({ days: 1 }).toISO()!, NOW),
      ).toBe("1 day");
      expect(
        formatRelativeMagnitude(NOW.minus({ days: 3 }).toISO()!, NOW),
      ).toBe("3 days");
      expect(
        formatRelativeMagnitude(NOW.minus({ days: 5 }).toISO()!, NOW),
      ).toBe("5 days");
    });

    it("returns singular/plural weeks", () => {
      expect(
        formatRelativeMagnitude(NOW.minus({ weeks: 2 }).toISO()!, NOW),
      ).toBe("2 weeks");
      expect(
        formatRelativeMagnitude(NOW.minus({ weeks: 3 }).toISO()!, NOW),
      ).toBe("3 weeks");
    });

    it("returns singular/plural months", () => {
      // 60 days ≈ 2 months (using the 30-day month bucket)
      expect(
        formatRelativeMagnitude(NOW.minus({ days: 60 }).toISO()!, NOW),
      ).toBe("2 months");
    });

    it("returns singular/plural years", () => {
      expect(
        formatRelativeMagnitude(NOW.minus({ years: 1 }).toISO()!, NOW),
      ).toBe("1 year");
      expect(
        formatRelativeMagnitude(NOW.minus({ years: 2 }).toISO()!, NOW),
      ).toBe("2 years");
    });
  });

  describe("direction-agnostic", () => {
    it("produces the same magnitude for past and future deltas", () => {
      const past = NOW.minus({ days: 5 }).toISO()!;
      const future = NOW.plus({ days: 5 }).toISO()!;
      expect(formatRelativeMagnitude(past, NOW)).toBe("5 days");
      expect(formatRelativeMagnitude(future, NOW)).toBe("5 days");
    });
  });

  describe("bare YYYY-MM-DD timestamps", () => {
    it("treats the bare date as midnight UTC for delta math", () => {
      const now = DateTime.fromISO("2026-04-15T00:00:00Z", { zone: "utc" });
      expect(formatRelativeMagnitude("2026-04-10", now)).toBe("5 days");
    });
  });

  describe("fail-safe behaviour", () => {
    it("returns the original string on unparseable input", () => {
      expect(formatRelativeMagnitude("not-a-date", NOW)).toBe("not-a-date");
      expect(formatRelativeMagnitude("", NOW)).toBe("");
    });
  });

  describe("default `now`", () => {
    it("uses the current clock when `now` is omitted", () => {
      // Timestamp ~30s in the past should produce "just now".
      const ts = DateTime.now().minus({ seconds: 30 }).toISO()!;
      expect(formatRelativeMagnitude(ts)).toBe("just now");
    });
  });
});
