"""CLI start-up timing: alternate binaries, report median and p95 per round (BL-055).

Usage: python3 -I benches/cli_startup.py /usr/bin/true path/to/sk_a path/to/sk_b ...
Each binary except /usr/bin/true is called as `<bin> "=1+1"`. Run it from an empty
directory so no hooks/ folder is loaded. Times include Python subprocess overhead,
so compare binaries within one run, not with numbers from other tools.
Environment: RUNS (calls per binary per round, default 300), ROUNDS (default 5).
"""
import os
import statistics
import subprocess
import sys
import time

bins = sys.argv[1:]
if not bins:
    sys.exit(__doc__)
runs = int(os.environ.get("RUNS", "300"))
rounds = int(os.environ.get("ROUNDS", "5"))
results = {b: [] for b in bins}

for r in range(rounds):
    samples = {b: [] for b in bins}
    for _ in range(runs):
        for b in bins:
            args = [b] if os.path.basename(b) == "true" else [b, "=1+1"]
            t = time.perf_counter()
            subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            samples[b].append((time.perf_counter() - t) * 1000)
    for b in bins:
        v = sorted(samples[b])
        results[b].append((statistics.median(v), v[int(len(v) * 0.95)]))
    print(f"round {r}: " + " ".join(f"{b}={results[b][-1][0]:.3f}" for b in bins), flush=True)

for b in bins:
    meds = [m for m, _ in results[b]]
    p95s = [p for _, p in results[b]]
    print(
        f"{b}: median {statistics.median(meds):.3f} ms "
        f"(round medians {min(meds):.3f}-{max(meds):.3f}), p95 {statistics.median(p95s):.3f} ms"
    )
