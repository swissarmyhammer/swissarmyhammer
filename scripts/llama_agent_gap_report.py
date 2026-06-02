#!/usr/bin/env python3
"""Parse an LCOV file and emit a per-file line-coverage gap report.

Scoped to a path substring (default: ``llama-agent/src``) so the workspace-wide
LCOV produced by ``cargo llvm-cov --package llama-agent`` can be reduced to just
the crate under measurement. Computes overall and per-file line coverage and the
uncovered line ranges for each file, ranked worst-first by uncovered line count.

This is also the CI coverage gate: ``--fail-under`` enforces a crate-wide line
floor and ``--critical`` enforces per-file floors on the behaviour-critical
modules, exiting non-zero (and printing the violation) when either is breached.
Gating here rather than on ``cargo llvm-cov --fail-under-lines`` is deliberate:
the cargo flag gates the workspace-wide TOTAL (~30%, polluted by other crates
instrumented as path-deps), whereas this script's number is scoped to
``llama-agent/src`` only — the meaningful figure.

Usage:
    llama_agent_gap_report.py <lcov_path> [path_substring]
    llama_agent_gap_report.py <lcov_path> --fail-under 80 \\
        --critical generation/budget.rs:100 --critical queue.rs:90
"""

import argparse
import sys
from collections import defaultdict


def compress_ranges(lines):
    """Collapse a sorted list of ints into compact ``a-b`` / ``a`` range strings."""
    if not lines:
        return []
    lines = sorted(lines)
    ranges = []
    start = prev = lines[0]
    for n in lines[1:]:
        if n == prev + 1:
            prev = n
            continue
        ranges.append(f"{start}-{prev}" if start != prev else f"{start}")
        start = prev = n
    ranges.append(f"{start}-{prev}" if start != prev else f"{start}")
    return ranges


def parse_lcov(path, needle):
    """Return {file: {line: hits}} for files whose path contains ``needle``."""
    per_file = defaultdict(dict)
    current = None
    keep = False
    with open(path, encoding="utf-8") as fh:
        for raw in fh:
            line = raw.rstrip("\n")
            if line.startswith("SF:"):
                current = line[3:]
                keep = needle in current
            elif line.startswith("DA:") and keep:
                num, _, hits = line[3:].partition(",")
                per_file[current][int(num)] = int(hits)
            elif line == "end_of_record":
                current = None
                keep = False
    return per_file


def parse_critical(values):
    """Parse repeated ``FILE:PCT`` args into {file_suffix: float}."""
    floors = {}
    for item in values or []:
        name, _, pct = item.rpartition(":")
        if not name or not pct:
            print(f"error: --critical expects FILE:PCT, got {item!r}", file=sys.stderr)
            sys.exit(2)
        floors[name] = float(pct)
    return floors


def main():
    parser = argparse.ArgumentParser(
        description="Per-file line-coverage gap report + CI coverage gate.",
    )
    parser.add_argument("lcov_path", help="Path to the LCOV file")
    parser.add_argument(
        "needle",
        nargs="?",
        default="llama-agent/src",
        help="Path substring to scope the report (default: llama-agent/src)",
    )
    parser.add_argument(
        "--fail-under",
        type=float,
        default=None,
        metavar="PCT",
        help="Exit non-zero if overall scoped line coverage is below PCT.",
    )
    parser.add_argument(
        "--critical",
        action="append",
        default=[],
        metavar="FILE:PCT",
        help="Per-file floor (path suffix match). Repeatable.",
    )
    args = parser.parse_args()

    needle = args.needle
    critical = parse_critical(args.critical)

    per_file = parse_lcov(args.lcov_path, needle)

    rows = []
    total_inst = total_cov = 0
    for path, lines in per_file.items():
        inst = len(lines)
        cov = sum(1 for h in lines.values() if h > 0)
        total_inst += inst
        total_cov += cov
        uncovered = [n for n, h in lines.items() if h == 0]
        short = path.split("crates/", 1)[-1] if "crates/" in path else path
        rows.append((short, inst, cov, uncovered))

    # Rank worst-first: most uncovered lines, then lowest coverage %.
    rows.sort(key=lambda r: (-(r[1] - r[2]), (r[2] / r[1] if r[1] else 1.0)))

    overall = 100.0 * total_cov / total_inst if total_inst else 0.0
    print(f"# llama-agent line coverage (scope: {needle})")
    print(f"FILES: {len(rows)}")
    print(f"OVERALL LINES: {total_cov}/{total_inst} covered = {overall:.2f}%")
    print(f"OVERALL UNCOVERED LINES: {total_inst - total_cov}")
    print()
    header = f"{'FILE':<60} {'COV%':>7} {'COV':>7} {'INST':>7} {'UNCOV':>7}"
    print(header)
    print("-" * len(header))
    for short, inst, cov, uncovered in rows:
        pct = 100.0 * cov / inst if inst else 0.0
        print(f"{short:<60} {pct:>6.2f}% {cov:>7} {inst:>7} {len(uncovered):>7}")
    print()
    print("# Uncovered line ranges (worst-first)")
    for short, inst, cov, uncovered in rows:
        if not uncovered:
            continue
        ranges = ", ".join(compress_ranges(uncovered))
        print(f"\n## {short}  ({len(uncovered)} uncovered of {inst})")
        print(ranges)

    # --- Coverage gate -----------------------------------------------------
    if args.fail_under is None and not critical:
        return

    failures = []
    if args.fail_under is not None:
        status = "PASS" if overall >= args.fail_under else "FAIL"
        print(f"\nGATE crate line coverage: {overall:.2f}% >= {args.fail_under:.2f}%? {status}")
        if status == "FAIL":
            failures.append(
                f"crate line coverage {overall:.2f}% < floor {args.fail_under:.2f}%"
            )

    if critical:
        pct_by_short = {
            short: (100.0 * cov / inst if inst else 0.0)
            for short, inst, cov, _ in rows
        }
        for name, floor in sorted(critical.items()):
            matches = [s for s in pct_by_short if s.endswith(name)]
            if not matches:
                failures.append(f"critical file not found in report: {name}")
                print(f"GATE critical {name}: NOT FOUND")
                continue
            for short in matches:
                actual = pct_by_short[short]
                status = "PASS" if actual >= floor else "FAIL"
                print(f"GATE critical {short}: {actual:.2f}% >= {floor:.2f}%? {status}")
                if status == "FAIL":
                    failures.append(f"{short} {actual:.2f}% < floor {floor:.2f}%")

    if failures:
        print("\nCOVERAGE GATE FAILED:")
        for f in failures:
            print(f"  - {f}")
        sys.exit(1)
    print("\nCOVERAGE GATE PASSED")


if __name__ == "__main__":
    main()
