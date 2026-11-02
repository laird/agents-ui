#!/usr/bin/env python3
"""Record running demos' /api/village every 3 s for the replay page.

    RUNTIME=k8s PORT=7930 tools/mock/demo.sh &
    RUNTIME=temporal PORT=7931 TEMPORAL_PORT=7331 tools/mock/demo.sh &
    python3 tools/mock/record.py tools/mock/recordings/demo.json.gz 360 \\
        k8s=7930:shop,k8s:shop-prod temporal=7931:shop,temporal:agents
    python3 tools/mock/replay-build.py tools/mock/recordings/demo.json.gz replay.html

Each run is <name>=<port>:<village>,<village>...; only those villages (and
the worlds) are kept.
"""
import gzip
import json
import sys
import time
import urllib.request

out, secs, specs = sys.argv[1], int(sys.argv[2]), sys.argv[3:]
runs = {}
for spec in specs:
    name, rest = spec.split("=", 1)
    port, keep = rest.split(":", 1)
    runs[name] = (f"http://127.0.0.1:{port}/api/village", keep.split(","))
frames = {k: [] for k in runs}
end = time.time() + secs
while time.time() < end:
    t0 = time.time()
    for k, (url, keep) in runs.items():
        try:
            d = json.load(urllib.request.urlopen(url, timeout=5))
            frames[k].append({"villages": [v for v in d["villages"] if v["project"] in keep], "worlds": d["worlds"]})
        except Exception as e:  # a slow poll skips a frame
            print("skip", k, e, file=sys.stderr)
    time.sleep(max(0, 3 - (time.time() - t0)))
raw = json.dumps(frames, separators=(",", ":")).encode()
open(out, "wb").write(gzip.compress(raw, 9))
print("frames", {k: len(v) for k, v in frames.items()}, "raw", len(raw))
