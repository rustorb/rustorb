#!/usr/bin/env python3
"""Compares two runs of run-bench.py (before and after a change) and prints
a Markdown table of the differences of their medians (docs/perf-plan.md).

    compare-bench.py BEFORE AFTER [--counters] [--all]

BEFORE and AFTER are result.json files or the directories that hold them.
Each row is one implementation (or client→server pair) and case, with the
latency, rate, throughput, CPU time and context switches of both runs and
the change in percent. The change is marked when it is better (+) or worse
(−) by more than --threshold percent (default 5): lower is better for
latency, CPU time and context switches, higher for calls/s and MB/s.

--counters adds the table of rustorb's internal counters per call
(run-bench.py --stats), the counters that changed. --all also lists the
metrics that changed less than the threshold.
"""

import argparse
import json
import os
import sys

# (key, heading, higher is better)
METRICS = [
    ("p50_us", "p50 µs", False),
    ("p99_us", "p99 µs", False),
    ("rate", "calls/s", True),
    ("mb_per_s", "MB/s", True),
    ("cpu_us_per_call", "CPU µs/call", False),
    ("client_cpu_us_per_call", "client CPU µs/call", False),
    ("server_cpu_us_per_call", "server CPU µs/call", False),
    ("csw_per_call", "ctx sw/call", False),
    ("client_csw_per_call", "client ctx sw/call", False),
    ("server_csw_per_call", "server ctx sw/call", False),
]
# Cases whose rate is the point; for the others the latency says it.
RATE_CASES = ("oneway", "threads:", "procs:")


def load(path):
    if os.path.isdir(path):
        path = os.path.join(path, "result.json")
    with open(path) as f:
        return json.load(f)


def key_of(m):
    if m["mode"] == "inproc":
        return ("%s in-process" % m["impl"], m["case"])
    who = "%s→%s" % (m["client"], m["server"])
    if m.get("transport") and m["transport"] != "tcp":
        who += " (%s)" % m["transport"]
    return (who, m["case"])


def fmt(v):
    if v is None:
        return "-"
    if abs(v) >= 100:
        return "{:,.0f}".format(v)
    if abs(v) >= 10:
        return "{:.1f}".format(v)
    return "{:.2f}".format(v)


def change(before, after, higher_better, threshold):
    """The change in percent, and its mark."""
    if before is None or after is None:
        return "-", ""
    if before == 0:
        return ("0%" if after == 0 else "new"), ""
    pct = (after - before) / before * 100
    better = pct > 0 if higher_better else pct < 0
    mark = ""
    # Below 0.05 (e.g. context switches per oneway call) a percentage
    # says nothing.
    if abs(pct) >= threshold and max(abs(before), abs(after)) >= 0.05:
        mark = "+" if better else "−"
    return "{:+.1f}%".format(pct), mark


def describe(result):
    m = result.get("machine", {})
    p = result.get("parameters", {})
    return "%s, %s (%s CPUs), rustorb %s, %s s per case, median of %s" % (
        result.get("date", "?"), m.get("cpu", "?"), m.get("logical_cpus", "?"),
        m.get("commit", "?"), p.get("seconds", "?"), p.get("repeat", "?"))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("before")
    ap.add_argument("after")
    ap.add_argument("--threshold", type=float, default=5.0)
    ap.add_argument("--counters", action="store_true")
    ap.add_argument("--all", action="store_true")
    args = ap.parse_args()
    before, after = load(args.before), load(args.after)
    b = {key_of(m): m for m in before["medians"]}
    a = {key_of(m): m for m in after["medians"]}
    keys = [k for k in b if k in a]
    out = ["Before: " + describe(before), "", "After: " + describe(after), "",
           "| | case | metric | before | after | change | |",
           "| --- | --- | --- | ---: | ---: | ---: | :---: |"]
    for k in keys:
        mb, ma = b[k], a[k]
        if not (mb.get("ok") and ma.get("ok")):
            out.append("| %s | %s | | %s | %s | | |" % (
                k[0], k[1], "ok" if mb.get("ok") else "FAIL", "ok" if ma.get("ok") else "FAIL"))
            continue
        first = True
        for key, heading, higher in METRICS:
            if key == "rate" and not k[1].startswith(RATE_CASES):
                continue
            if key == "mb_per_s" and not (mb.get(key) or ma.get(key)):
                continue
            vb, va = mb.get(key), ma.get(key)
            if vb is None and va is None:
                continue
            pct, mark = change(vb, va, higher, args.threshold)
            if not args.all and not mark and key not in ("p50_us", "rate"):
                continue
            out.append("| %s | %s | %s | %s | %s | %s | %s |" % (
                k[0] if first else "", k[1] if first else "", heading, fmt(vb), fmt(va),
                pct, mark))
            first = False
    missing = [k for k in b if k not in a] + [k for k in a if k not in b]
    if missing:
        out += ["", "Only in one of the runs: " +
                ", ".join("%s `%s`" % k for k in missing)]
    if args.counters:
        rows = []
        for k in keys:
            cb = b[k].get("counters_per_call") or {}
            ca = a[k].get("counters_per_call") or {}
            for name in list(cb) + [n for n in ca if n not in cb]:
                vb, va = cb.get(name, 0.0), ca.get(name, 0.0)
                if abs(va - vb) < 0.005 and not args.all:
                    continue
                if vb == 0 and va == 0:
                    continue
                rows.append("| %s | %s | %s | %.3f | %.3f | %+.3f |" % (
                    k[0], k[1], name, vb, va, va - vb))
        if rows:
            out += ["", "rustorb internal counters per call (rustorb_core::stats):", "",
                    "| | case | counter | before | after | change |",
                    "| --- | --- | --- | ---: | ---: | ---: |"] + rows
    print("\n".join(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
