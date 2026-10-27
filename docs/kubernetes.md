# Running the village in Kubernetes

agents-tui runs as a single small pod that serves the village dashboard
read-only. It is a good fit for **Temporal**: the pod reads each namespace
through the Temporal frontend's HTTP API, so it sees every workstream in the
cluster wherever its workers run.

```bash
docker build -t ghcr.io/<you>/agents-ui:latest . && docker push ghcr.io/<you>/agents-ui:latest
# edit image, --temporal and --temporal-url in the manifest, then:
kubectl apply -n agents -f deploy/k8s/agents-ui.yaml
kubectl port-forward -n agents svc/agents-ui 7878:80
open http://localhost:7878/village
```

What the image sets up:

- `--headless`: no terminal, just the web server.
- `--web-bind 0.0.0.0`: listen on the pod IP (the default is 127.0.0.1).
- `--read-only`: every request but GET/HEAD gets 403, so nobody who can
  reach the dashboard can launch, stop or type into agents.
- `GET /healthz` returns `ok` for the probes.

The pod has no login of its own. Keep the Service cluster-internal and reach
it with `kubectl port-forward`, or put it behind an ingress that
authenticates (for example oauth2-proxy).

## Coding swarms

tmux/herdr swarms and Claude Code sub-agents are read from the machine the
agents run on, so a pod only shows them if the agents run in that pod (or
the pod mounts the same tmux socket, `~/.claude` and repos). For a coding
swarm on a workstation or VM, run `agents-tui --headless` there instead and
tunnel to it: `ssh -L 7878:localhost:7878 host`.
