#!/usr/bin/env python3
"""Parse an LCOV file and emit a per-file line-coverage gap report.

Scoped to a path substring (default: ``llama-agent/src``) so the workspace-wide
LCOV produced by ``cargo llvm-cov --package llama-agent`` can be reduced to just
the crate under measurement. Computes overall and per-file line coverage and the
uncovered line ranges for each file, ranked worst-first by uncovered line count.

Usage:
    llama_agent_gap_report.py <lcov_path> [path_substring]
"""

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


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    lcov_path = sys.argv[1]
    needle = sys.argv[2] if len(sys.argv) > 2 else "llama-agent/src"

    per_file = parse_lcov(lcov_path, needle)

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


if __name__ == "__main__":
    main()
