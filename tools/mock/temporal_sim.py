#!/usr/bin/env python3
"""A stand-in Temporal frontend HTTP API with a simulated agentic namespace.

Serves the three endpoints agents-tui reads, in the shapes Temporal's HTTP
API returns (protojson, base64 payloads for search attributes):

  GET /api/v1/namespaces/{ns}/workflows?query=...     ListWorkflowExecutions
  GET /api/v1/namespaces/{ns}/workflows/{id}          DescribeWorkflowExecution
  GET /api/v1/namespaces/{ns}/task-queues/{queue}     DescribeTaskQueue

Workstreams (top-level workflows) start every few seconds, run a plan of
activities, start agents (child workflows, sometimes grandchildren), wait on
a person (AgentStatus=waiting_for_human), retry failing activities, and end
completed or failed. A "nightly-reports" queue loses its workers now and then.

Linked to the dev swarm (swarm_sim.py) through two files, so the demo plays
the whole loop: --release-file holds the commit the dev swarm last released
to prod, and workers roll over to it one at a time as their build ID;
--incident-file gets a line for every workstream that fails, which the dev
swarm files as an `incident` issue.

    python3 tools/mock/temporal_sim.py --port 7243 --namespace agents
"""

import argparse
import base64
import json
import random
import threading
import time
import urllib.parse
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

STREAMS = [
    ("ProcurementWorkstream", ["load_contract", "fetch_vendor_quotes", "call_llm"], ["VendorResearchAgent", "PriceAnalystAgent"], True),
    ("SecurityReviewWorkstream", ["fetch_advisories", "call_llm", "write_findings"], ["DependencyScanAgent", "TriageAgent"], False),
    ("BugFixWorkstream", ["load_logs", "call_llm", "run_tests", "commit_fix"], ["ReproAgent"], False),
    ("FeatureBuildWorkstream", ["plan_feature", "write_code", "run_tests"], ["CoderAgent", "TesterAgent", "ReviewerAgent"], True),
    ("DocsWorkstream", ["read_source", "draft_docs", "send_review_email"], [], False),
    ("CustomerSupportWorkstream", ["search_tickets", "call_llm", "send_reply"], ["KnowledgeBaseAgent"], True),
    ("ResearchWorkstream", ["search_papers", "summarize", "write_report"], ["LiteratureAgent", "AnalystAgent"], False),
]
AGENT_ACTIVITIES = ["call_llm", "fetch_page", "read_document", "write_draft", "run_query", "send_message", "classify"]
NIGHTLY = "nightly-reports"


def now_iso(ts):
    return datetime.fromtimestamp(ts, timezone.utc).isoformat().replace("+00:00", "Z")


def payload(value):
    """A search attribute as the API encodes it without payload shorthand."""
    return {
        "metadata": {"encoding": base64.b64encode(b"json/plain").decode()},
        "data": base64.b64encode(json.dumps(value).encode()).decode(),
    }


class Workflow:
    def __init__(self, wid, type_name, queue, parent=None, root=None, plan=None):
        self.id, self.type_name, self.queue = wid, type_name, queue
        self.parent, self.root = parent, root
        self.plan = list(plan or [])
        self.status = "RUNNING"
        self.start = time.time()
        self.close = None
        self.step_ends = time.time() + random.uniform(4, 9)
        self.attempt = 1
        self.failing = False
        self.waiting_until = 0.0
        self.children = []

    def info(self):
        e = {
            "execution": {"workflowId": self.id, "runId": "run-" + self.id.replace("/", "-")},
            "type": {"name": self.type_name},
            "startTime": now_iso(self.start),
            "status": "WORKFLOW_EXECUTION_STATUS_" + self.status,
            "taskQueue": self.queue,
        }
        if self.close:
            e["closeTime"] = now_iso(self.close)
        if self.parent:
            e["parentExecution"] = {"workflowId": self.parent.id, "runId": "run-" + self.parent.id.replace("/", "-")}
            e["rootExecution"] = {"workflowId": self.root.id, "runId": "run-" + self.root.id.replace("/", "-")}
        status = "waiting_for_human" if self.waiting_until > time.time() else "working"
        e["searchAttributes"] = {"indexedFields": {"AgentStatus": payload(status)}}
        return e

    def describe(self):
        d = {"workflowExecutionInfo": self.info(), "pendingActivities": [], "pendingChildren": []}
        if self.status == "RUNNING" and self.waiting_until <= time.time() and self.plan:
            act = {"activityId": "1", "activityType": {"name": self.plan[0]}, "state": "PENDING_ACTIVITY_STATE_STARTED", "attempt": self.attempt}
            if self.attempt > 1:
                act["lastFailure"] = {"message": "upstream returned 429 Too Many Requests"}
            d["pendingActivities"].append(act)
        d["pendingChildren"] = [{"workflowId": c.id, "workflowTypeName": c.type_name} for c in self.children if c.status == "RUNNING"]
        return d


class Namespace:
    def __init__(self, name, release_file=None, incident_file=None):
        self.name = name
        self.release_file, self.incident_file = release_file, incident_file
        self.builds = {"old": "initial", "new": "initial", "at": 0.0}
        self.lock = threading.Lock()
        self.workflows = []
        self.counter = 100
        self.next_stream = time.time()
        self.nightly_workers = True
        self.next_outage = time.time() + 40

    def start_stream(self):
        self.counter += 1
        type_name, plan, agents, waits = random.choice(STREAMS)
        queue = NIGHTLY if type_name == "ResearchWorkstream" else "agents"
        wid = f"{type_name.replace('Workstream', '').lower()}-{self.counter}"
        wf = Workflow(wid, type_name, queue, plan=plan)
        wf.agents_to_start = list(agents)
        wf.will_wait = waits and random.random() < 0.6
        wf.will_fail = random.random() < 0.15
        self.workflows.append(wf)

    def read_release(self, t):
        if not self.release_file:
            return
        try:
            with open(self.release_file) as f:
                sha = f.read().strip()[:12]
        except OSError:
            return
        if sha and sha != self.builds["new"]:
            self.builds = {"old": self.builds["new"], "new": sha, "at": t}

    def build_for(self, i):
        """Workers roll over to a new release one at a time, 5 s apart."""
        return self.builds["new"] if time.time() >= self.builds["at"] + 5 * (i + 1) else self.builds["old"]

    def tick(self):
        t = time.time()
        self.read_release(t)
        with self.lock:
            running = [w for w in self.workflows if w.status == "RUNNING"]
            if t >= self.next_stream and sum(1 for w in running if not w.parent) < 9:
                self.start_stream()
                self.next_stream = t + random.uniform(6, 12)
            if t >= self.next_outage:
                self.nightly_workers = not self.nightly_workers
                self.next_outage = t + (25 if not self.nightly_workers else 60)
            for wf in running:
                served = wf.queue != NIGHTLY or self.nightly_workers
                if not served or wf.waiting_until > t:
                    continue
                # A top-level workstream starts its agents as it goes.
                if not wf.parent and getattr(wf, "agents_to_start", None) and random.random() < 0.2:
                    name = wf.agents_to_start.pop(0)
                    child = Workflow(f"{wf.id}/{name}", name, wf.queue, parent=wf, root=wf,
                                     plan=random.sample(AGENT_ACTIVITIES, 3))
                    child.will_wait = name in ("PriceAnalystAgent", "ReviewerAgent") and random.random() < 0.7
                    wf.children.append(child)
                    self.workflows.append(child)
                    if random.random() < 0.3:
                        grandchild = Workflow(f"{child.id}/HelperAgent", "HelperAgent", wf.queue, parent=child, root=wf,
                                              plan=random.sample(AGENT_ACTIVITIES, 2))
                        child.children.append(grandchild)
                        self.workflows.append(grandchild)
                if t < wf.step_ends:
                    continue
                if getattr(wf, "will_wait", False) and len(wf.plan) == 1:
                    wf.will_wait = False
                    wf.waiting_until = t + random.uniform(12, 20)
                    continue
                if getattr(wf, "will_fail", False) and len(wf.plan) <= 2:
                    wf.attempt += 1
                    wf.step_ends = t + 3
                    if wf.attempt > 5:
                        self.finish(wf, "FAILED")
                    continue
                if wf.plan:
                    wf.plan.pop(0)
                    wf.attempt = 1
                    wf.step_ends = t + random.uniform(4, 9)
                if not wf.plan and all(c.status != "RUNNING" for c in wf.children):
                    self.finish(wf, "COMPLETED")
            # Forget work closed more than a day ago.
            self.workflows = [w for w in self.workflows if not w.close or t - w.close < 86400]

    def finish(self, wf, status):
        wf.status, wf.close = status, time.time()
        if status == "FAILED" and not wf.parent and self.incident_file:
            with open(self.incident_file, "a") as f:
                f.write(json.dumps({"id": wf.id, "type": wf.type_name, "reason": "upstream returned 429 after 5 attempts"}) + "\n")
        for c in wf.children:
            if c.status == "RUNNING":
                self.finish(c, "TERMINATED" if status == "FAILED" else "COMPLETED")

    def list(self, query):
        with self.lock:
            if "!=" in query:
                return [w.info() for w in self.workflows if w.status != "RUNNING"]
            return [w.info() for w in self.workflows if w.status == "RUNNING"]

    def describe(self, wid):
        with self.lock:
            for w in self.workflows:
                if w.id == wid:
                    return w.describe()
        return None

    def pollers(self, queue):
        if queue == NIGHTLY and not self.nightly_workers:
            return []
        n = 2 if queue == NIGHTLY else 3
        return [{"identity": f"{queue}-worker-{i}@pod-{i}", "lastAccessTime": now_iso(time.time()),
                 "deploymentOptions": {"buildId": self.build_for(i)}} for i in range(1, n + 1)]


def handler(ns):
    class H(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def send(self, code, body):
            data = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            url = urllib.parse.urlparse(self.path)
            query = urllib.parse.parse_qs(url.query)
            parts = [urllib.parse.unquote(p) for p in url.path.split("/")]
            if parts[1:4] != ["api", "v1", "namespaces"] or len(parts) < 6 or parts[4] != ns.name:
                return self.send(404, {"code": 5, "message": "namespace not found"})
            kind, rest = parts[5], "/".join(parts[6:])
            if kind == "workflows" and not rest:
                return self.send(200, {"executions": ns.list(query.get("query", [""])[0]), "nextPageToken": None})
            if kind == "workflows":
                d = ns.describe(rest)
                return self.send(200, d) if d else self.send(404, {"code": 5, "message": "workflow not found"})
            if kind == "task-queues" and rest:
                return self.send(200, {"pollers": ns.pollers(rest)})
            self.send(404, {"code": 12, "message": "unimplemented"})

    return H


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=int, default=7243)
    ap.add_argument("--namespace", default="agents")
    ap.add_argument("--seed", type=int)
    ap.add_argument("--release-file", help="file holding the commit the dev swarm last released")
    ap.add_argument("--incident-file", help="file to append failed workstreams to, one JSON line each")
    args = ap.parse_args()
    if args.seed is not None:
        random.seed(args.seed)
    ns = Namespace(args.namespace, args.release_file, args.incident_file)
    ns.read_release(0)
    ns.builds["old"] = ns.builds["new"]
    for _ in range(4):
        ns.start_stream()

    def loop():
        while True:
            ns.tick()
            time.sleep(0.5)

    threading.Thread(target=loop, daemon=True).start()
    print(f"mock Temporal HTTP API for namespace '{args.namespace}' on http://127.0.0.1:{args.port}", flush=True)
    ThreadingHTTPServer(("127.0.0.1", args.port), handler(ns)).serve_forever()


if __name__ == "__main__":
    main()
