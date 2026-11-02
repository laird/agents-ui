#!/usr/bin/env python3
"""Simulated Kubernetes namespace for the village: what `kubectl get -o json` would show.

Writes <dir>/state/k8s/workloads.json and nodes.json every second; the
`kubectl` shim in tools/mock/bin serves them for the context `sim`. The
namespace runs an agentic app: a few Deployments, a StatefulSet and a
CronJob, on three nodes, and things go wrong the way they do in a cluster:

  * a release rolls out: with --release-file, every new commit the dev swarm
    promotes becomes the image tag, and pods are replaced one at a time;
  * a pod crash-loops for a while (and files an incident with --incident-file);
  * a scale-up leaves a pod unschedulable until it gives up;
  * a node goes NotReady and comes back; a node is cordoned for a while;
  * the CronJob starts a Job every 40 s; now and then one fails.

With --story it tells the incident story cleanly instead: 30 s in, the
agent-worker pods on the running image start crash-looping (a bug in that
release), and 15 s later the crash loop files one incident; every image released before the fix keeps
crashing, and pods only recover once they run the release that carries the
fix (named in <dir>/state/fixed by the swarm simulator). No other failures.
"""
import argparse
import json
import os
import random
import time

NS = "shop-prod"
NODES = ["node-a", "node-b", "node-c"]
WORKLOADS = [  # name, kind, replicas
    ("api", "Deployment", 3),
    ("agent-worker", "Deployment", 4),
    ("web", "Deployment", 2),
    ("security-scanner", "Deployment", 1),
    ("redis", "StatefulSet", 1),
]
IMAGE = "ghcr.io/larkspur-labs/{name}:{tag}"


def h(s):
    return format(abs(hash(s)) % 0xFFFFF, "x")


class Cluster:
    def __init__(self, release_file, incident_file, story=False, fixed_file=None):
        self.release_file, self.incident_file = release_file, incident_file
        self.story, self.fixed_file = story, fixed_file
        self.tags = []  # every tag seen, in order
        self.bug_from = None  # the story's broken tag: it and later ones crash, until the fixed one
        self.fixed_tag = None
        self.story_at = time.time() + 30
        self.story_filed = False
        self.tag, self.old_tag = "1f2e3d4c5b6a", None
        self.pods = {}  # name -> dict
        self.serial = 0
        self.node_down = None  # (node, until)
        self.cordoned = None
        self.crash = None  # (pod, until)
        self.pending = None  # (pod, until)
        self.jobs = []  # dicts
        self.next = {"crash": time.time() + 25, "node": time.time() + 70, "pending": time.time() + 45,
                     "job": time.time() + 5, "cordon": time.time() + 100}
        for name, kind, n in WORKLOADS:
            for i in range(n):
                self.add_pod(name, kind, i, ready_at=0)

    def add_pod(self, name, kind, i=None, ready_at=None, unschedulable=False):
        self.serial += 1
        tmpl = h(name + self.tag)[:5]
        pname = f"{name}-{i}" if kind == "StatefulSet" else f"{name}-{tmpl}-{h(str(self.serial))[:5]}"
        self.pods[pname] = dict(name=pname, workload=name, kind=kind, tag=self.tag, tmpl=tmpl,
                                node=random.choice([n for n in NODES if n != (self.node_down or (None,))[0]]),
                                created=time.time(), ready_at=time.time() + 4 if ready_at is None else ready_at,
                                restarts=0, deleting=None, unschedulable=unschedulable)
        return pname

    def read_release(self):
        if not self.release_file:
            return
        try:
            sha = open(self.release_file).read().strip()[:12]
        except OSError:
            return
        if sha and sha != self.tag:
            self.old_tag, self.tag = self.tag, sha
        if self.tag not in self.tags:
            self.tags.append(self.tag)

    def incident(self, workload, pod, reason):
        if self.incident_file:
            with open(self.incident_file, "a") as f:
                f.write(json.dumps({"type": workload, "id": pod, "reason": reason}) + "\n")

    def story_crashing(self, p):
        """In the story, agent-worker pods built from the broken image up to the fix crash-loop."""
        if not self.story or self.bug_from is None or p["workload"] != "agent-worker":
            return False
        order = {tag: i for i, tag in enumerate(self.tags)}
        i = order.get(p["tag"], 0)
        broken = i >= order[self.bug_from] and (self.fixed_tag is None or i < order.get(self.fixed_tag, 10**9))
        return broken and not p["deleting"]

    def tick(self):
        t = time.time()
        self.read_release()
        if self.story:
            if self.bug_from is None and t > self.story_at:
                self.bug_from = self.tag
            # The incident is filed once the crash loop has gone on for a while.
            if self.bug_from is not None and not self.story_filed and t > self.story_at + 15:
                self.story_filed = True
                victim = next((p for p in self.pods.values() if p["workload"] == "agent-worker"), None)
                self.incident("agent-worker", victim["name"] if victim else "agent-worker",
                              "CrashLoopBackOff: refund handler panics on an empty basket")
            if self.fixed_tag is None and self.fixed_file:
                try:
                    sha = open(self.fixed_file).read().strip()[:12]
                    self.fixed_tag = sha or None
                    if sha and sha not in self.tags:
                        self.tags.append(sha)
                except OSError:
                    pass
            for p in self.pods.values():
                if self.story_crashing(p) and random.random() < 0.15:
                    p["restarts"] += 1
        # Rolling update: one old pod per workload at a time, every 4 s.
        for name, kind, n in WORKLOADS:
            mine = [p for p in self.pods.values() if p["workload"] == name and not p["deleting"]]
            old = [p for p in mine if p["tag"] != self.tag]
            busy = any(p["tag"] == self.tag and p["ready_at"] > t for p in mine)
            if old and not busy:
                victim = old[0]
                victim["deleting"] = t
                self.add_pod(name, kind, victim["name"].rsplit("-", 1)[-1] if kind == "StatefulSet" else None)
        for p in list(self.pods.values()):
            if p["deleting"] and t - p["deleting"] > 3:
                del self.pods[p["name"]]
        # A crash loop.
        if not self.crash and t > self.next["crash"] and not self.story:
            victims = [p for p in self.pods.values() if p["workload"] == "agent-worker" and not p["deleting"]]
            if victims:
                v = random.choice(victims)
                self.crash = (v["name"], t + random.uniform(25, 40))
                if random.random() < 0.6:
                    self.incident("agent-worker", v["name"], "CrashLoopBackOff, OOMKilled")
        if self.crash:
            pod = self.pods.get(self.crash[0])
            if not pod or t > self.crash[1]:
                self.crash, self.next["crash"] = None, t + random.uniform(60, 90)
            elif random.random() < 0.15:
                pod["restarts"] += 1
        # A scale-up that doesn't fit.
        if not self.pending and t > self.next["pending"] and not self.story:
            self.pending = (self.add_pod("web", "Deployment", unschedulable=True), t + 20)
        if self.pending and t > self.pending[1]:
            self.pods.pop(self.pending[0], None)
            self.pending, self.next["pending"] = None, t + random.uniform(60, 100)
        # Nodes.
        if not self.node_down and t > self.next["node"] and not self.story:
            self.node_down = ("node-c", t + 18)
        if self.node_down and t > self.node_down[1]:
            self.node_down, self.next["node"] = None, t + random.uniform(90, 140)
        if not self.cordoned and t > self.next["cordon"] and not self.story:
            self.cordoned = ("node-b", t + 25)
        if self.cordoned and t > self.cordoned[1]:
            self.cordoned, self.next["cordon"] = None, t + random.uniform(120, 180)
        # The CronJob.
        if t > self.next["job"]:
            self.jobs.append(dict(name=f"nightly-report-{int(t)}", start=t, end=t + 12, fail=random.random() < 0.2))
            self.jobs = self.jobs[-4:]
            self.next["job"] = t + 40

    # -- what kubectl would print ------------------------------------------
    def pod_json(self, p, t):
        meta = {"name": p["name"], "namespace": NS, "labels": {"app": p["workload"]}}
        if p["kind"] == "Deployment":
            meta["labels"]["pod-template-hash"] = p["tmpl"]
            meta["ownerReferences"] = [{"kind": "ReplicaSet", "name": f"{p['workload']}-{p['tmpl']}"}]
        else:
            meta["ownerReferences"] = [{"kind": "StatefulSet", "name": p["workload"]}]
        if p["deleting"]:
            meta["deletionTimestamp"] = "2026-11-02T00:00:00Z"
        status = {"phase": "Running"}
        cs = {"name": p["workload"], "ready": True, "restartCount": p["restarts"], "state": {"running": {}}}
        if p["unschedulable"]:
            status = {"phase": "Pending", "conditions": [{"type": "PodScheduled", "status": "False", "reason": "Unschedulable",
                                                          "message": "0/3 nodes are available: 3 Insufficient memory."}]}
            cs = None
        elif (self.crash and self.crash[0] == p["name"]) or self.story_crashing(p):
            cs.update(ready=False, state={"waiting": {"reason": "CrashLoopBackOff"}}, lastState={"terminated": {"reason": "OOMKilled"}})
        elif t < p["ready_at"]:
            status["phase"] = "Pending"
            cs.update(ready=False, state={"waiting": {"reason": "ContainerCreating"}})
        if cs:
            status["containerStatuses"] = [cs]
        return {"kind": "Pod", "metadata": meta, "spec": {"nodeName": p["node"], "containers": [{"name": p["workload"], "image": IMAGE.format(name=p["workload"], tag=p["tag"])}]},
                "status": status}

    def workloads_json(self):
        t = time.time()
        items = []
        for name, kind, n in WORKLOADS:
            mine = [p for p in self.pods.values() if p["workload"] == name and not p["deleting"] and not p["unschedulable"]]
            ready = sum(1 for p in mine if t >= p["ready_at"] and not (self.crash and self.crash[0] == p["name"]) and not self.story_crashing(p))
            updated = sum(1 for p in mine if p["tag"] == self.tag)
            want = n + (1 if name == "web" and self.pending else 0)
            items.append({"kind": kind, "metadata": {"name": name, "namespace": NS}, "spec": {"replicas": want},
                          "status": {"readyReplicas": ready, "updatedReplicas": updated + (1 if name == "web" and self.pending else 0)}})
        items.append({"kind": "CronJob", "metadata": {"name": "nightly-report", "namespace": NS}})
        for j in self.jobs:
            done = t >= j["end"]
            st = {"failed": 1} if done and j["fail"] else {"succeeded": 1} if done else {"active": 1}
            items.append({"kind": "Job", "metadata": {"name": j["name"], "namespace": NS, "ownerReferences": [{"kind": "CronJob", "name": "nightly-report"}]}, "status": st})
            phase = ("Failed" if j["fail"] else "Succeeded") if done else "Running"
            items.append({"kind": "Pod", "metadata": {"name": j["name"] + "-x", "namespace": NS, "ownerReferences": [{"kind": "Job", "name": j["name"]}]},
                          "spec": {"containers": [{"name": "report", "image": IMAGE.format(name="report", tag=self.tag)}]},
                          "status": {"phase": phase, "containerStatuses": [{"ready": not done, "restartCount": 0, "state": {"running": {}} if not done else {"terminated": {"reason": "Error" if j["fail"] else "Completed"}}}]}})
        items += [self.pod_json(p, t) for p in self.pods.values()]
        return {"apiVersion": "v1", "kind": "List", "items": items}

    def nodes_json(self):
        out = []
        for n in NODES:
            down = self.node_down and self.node_down[0] == n
            node = {"kind": "Node", "metadata": {"name": n}, "status": {"conditions": [{"type": "Ready", "status": "False" if down else "True"}]}}
            if self.cordoned and self.cordoned[0] == n:
                node["spec"] = {"unschedulable": True}
            out.append(node)
        return {"apiVersion": "v1", "kind": "List", "items": out}


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dir", required=True, help="demo directory (state/k8s is written there)")
    ap.add_argument("--release-file")
    ap.add_argument("--incident-file")
    ap.add_argument("--seed", type=int)
    ap.add_argument("--story", action="store_true", help="tell the incident story cleanly (see the module docstring)")
    args = ap.parse_args()
    if args.seed is not None:
        random.seed(args.seed)
    out = os.path.join(args.dir, "state", "k8s")
    os.makedirs(out, exist_ok=True)
    c = Cluster(args.release_file, args.incident_file, args.story, os.path.join(args.dir, "state", "fixed"))
    while True:
        c.tick()
        for fname, data in (("workloads.json", c.workloads_json()), ("nodes.json", c.nodes_json())):
            tmp = os.path.join(out, fname + ".tmp")
            with open(tmp, "w") as f:
                json.dump(data, f)
            os.replace(tmp, os.path.join(out, fname))
        time.sleep(1)


if __name__ == "__main__":
    main()
