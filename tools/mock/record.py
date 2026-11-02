#!/usr/bin/env python3
"""Record running demos' /api/village every 3 s for the replay page.

    RUNTIME=k8s PORT=7930 tools/mock/demo.sh &
    RUNTIME=temporal PORT=7931 TEMPORAL_PORT=7331 tools/mock/demo.sh &
    python3 tools/mock/record.py tools/mock/recordings/demo.json.gz 360
    python3 tools/mock/replay-build.py tools/mock/recordings/demo.json.gz replay.html
"""
import json, time, urllib.request, gzip, sys
out = sys.argv[1]; secs = int(sys.argv[2])
runs = {"k8s": ("http://127.0.0.1:7930/api/village", ["shop", "k8s:shop-prod"]),
        "temporal": ("http://127.0.0.1:7931/api/village", ["shop", "temporal:agents"])}
frames = {k: [] for k in runs}
end = time.time() + secs
while time.time() < end:
    t0 = time.time()
    for k, (url, keep) in runs.items():
        try:
            d = json.load(urllib.request.urlopen(url, timeout=5))
            frames[k].append({"villages": [v for v in d["villages"] if v["project"] in keep], "worlds": d["worlds"]})
        except Exception as e:
            print("skip", k, e)
    time.sleep(max(0, 3 - (time.time() - t0)))
raw = json.dumps(frames, separators=(",", ":")).encode()
open(out, "wb").write(gzip.compress(raw, 9))
print("frames", {k: len(v) for k, v in frames.items()}, "raw", len(raw), "gz", len(gzip.compress(raw, 9)))
