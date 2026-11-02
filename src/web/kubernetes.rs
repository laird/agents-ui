//! Kubernetes as a runtime source: an alternative to Temporal for drawing the
//! working town of an agentic app (one source at a time per village).
//!
//! A namespace is the town. Each workload (Deployment, StatefulSet,
//! DaemonSet, CronJob, or a Job of its own) is a house, its kind shaping the
//! building; its pods are the people working there. The nodes are the worker
//! fleet in the hall. Kubernetes has no "waiting for a person" signal, so a
//! pod needs you when it is stuck: CrashLoopBackOff, ImagePullBackOff,
//! unschedulable, OOMKilled, a rollout past its deadline. The version a world
//! matches a release against is the image tag the pods run.
//!
//! Read-only, through `kubectl ... -o json` (run through the transport, so a
//! `--server` host's kubeconfig is used there); a read-only ServiceAccount is
//! enough: get and list on pods, workloads, jobs, events and nodes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use futures::future::BoxFuture;
use serde_json::Value;

use super::SwarmSnapshot;
use super::framework::{Agent, AgentState, Plane, Report, ReportPipeline, Stage, Task, TaskState, Workstream, to_village};
use super::source::{Reading, TtlCache, VillageSource};
use super::village::{HouseKind, HouseStatus, Note, house_kind};
use crate::transport::ServerTransport;

/// `--runtime k8s:[<context>/]<namespace>`.
#[derive(Debug, Clone, PartialEq)]
pub struct KubeSource {
    pub context: Option<String>,
    pub namespace: String,
}

impl KubeSource {
    /// `prod/shop`, or `shop` for the current context.
    pub fn parse(spec: &str) -> Option<Self> {
        let spec = spec.trim();
        let (context, namespace) = match spec.split_once('/') {
            Some((c, n)) => (Some(c.trim().to_string()).filter(|c| !c.is_empty()), n.trim()),
            None => (None, spec),
        };
        (!namespace.is_empty()).then(|| Self { context, namespace: namespace.to_string() })
    }

    pub fn project(&self) -> String {
        format!("k8s:{}", self.namespace)
    }

    fn label(&self) -> String {
        match &self.context {
            Some(c) => format!("{c}/{}", self.namespace),
            None => self.namespace.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Reading kubectl's JSON
// ---------------------------------------------------------------------------

fn s<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter().try_fold(v, |v, k| v.get(k)).and_then(Value::as_str)
}

fn n(v: &Value, path: &[&str]) -> i64 {
    path.iter().try_fold(v, |v, k| v.get(k)).and_then(Value::as_i64).unwrap_or(0)
}

fn items(list: &Value) -> &[Value] {
    list.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default()
}

/// The workload a pod belongs to: a ReplicaSet's Deployment (its name minus
/// the pod-template hash), a Job's CronJob, or the owner itself.
fn owner_of(pod: &Value, jobs: &HashMap<String, String>) -> Option<(String, String)> {
    let owner = pod.get("metadata")?.get("ownerReferences")?.as_array()?.first()?;
    let (kind, name) = (s(owner, &["kind"])?, s(owner, &["name"])?);
    Some(match kind {
        "ReplicaSet" => {
            let hash = s(pod, &["metadata", "labels", "pod-template-hash"]).unwrap_or_default();
            let dep = name.strip_suffix(&format!("-{hash}")).filter(|_| !hash.is_empty()).unwrap_or(name);
            ("Deployment".to_string(), dep.to_string())
        }
        "Job" => match jobs.get(name) {
            Some(cron) => ("CronJob".to_string(), cron.clone()),
            None => ("Job".to_string(), name.to_string()),
        },
        k => (k.to_string(), name.to_string()),
    })
}

/// What a pod is doing, and why it needs a person if it does.
pub fn pod_state(pod: &Value) -> (AgentState, Option<String>) {
    if pod.get("metadata").and_then(|m| m.get("deletionTimestamp")).is_some() {
        return (AgentState::Stopped, Some("Terminating".into()));
    }
    let statuses = pod.get("status").and_then(|s| s.get("containerStatuses")).and_then(Value::as_array).cloned().unwrap_or_default();
    let restarts: i64 = statuses.iter().map(|c| n(c, &["restartCount"])).sum();
    for c in &statuses {
        if let Some(reason) = s(c, &["state", "waiting", "reason"]) {
            if ["CrashLoopBackOff", "ImagePullBackOff", "ErrImagePull", "CreateContainerConfigError", "InvalidImageName"].contains(&reason) {
                let last = s(c, &["lastState", "terminated", "reason"]).map(|r| format!(", last {r}")).unwrap_or_default();
                return (AgentState::Waiting, Some(format!("{reason} · {restarts} restarts{last}")));
            }
            return (AgentState::Starting, Some(reason.to_string()));
        }
    }
    let phase = s(pod, &["status", "phase"]).unwrap_or("Unknown");
    if phase == "Pending" {
        let unschedulable = pod
            .get("status")
            .and_then(|s| s.get("conditions"))
            .and_then(Value::as_array)
            .and_then(|cs| cs.iter().find(|c| s(c, &["type"]) == Some("PodScheduled")))
            .filter(|c| s(c, &["status"]) == Some("False"));
        if let Some(c) = unschedulable {
            let msg = s(c, &["message"]).unwrap_or("no node fits");
            return (AgentState::Waiting, Some(format!("Unschedulable: {msg}")));
        }
        return (AgentState::Starting, Some("Pending".into()));
    }
    match phase {
        "Running" => {
            let ready = !statuses.is_empty() && statuses.iter().all(|c| c.get("ready").and_then(Value::as_bool) == Some(true));
            if ready {
                (AgentState::Working, None)
            } else {
                (AgentState::Stalled, Some("Running, not ready".into()))
            }
        }
        "Succeeded" => (AgentState::Done, None),
        "Failed" => {
            let oom = statuses.iter().any(|c| s(c, &["state", "terminated", "reason"]) == Some("OOMKilled"));
            (AgentState::Stopped, Some(if oom { "OOMKilled".into() } else { "Failed".into() }))
        }
        _ => (AgentState::Unknown, None),
    }
}

/// The version a pod runs: its first container's image tag, or the start of
/// its digest.
pub fn image_version(image: &str) -> Option<String> {
    if let Some((_, digest)) = image.split_once("@sha256:") {
        return Some(digest.chars().take(12).collect());
    }
    let last = image.rsplit('/').next().unwrap_or(image);
    last.split_once(':').map(|(_, tag)| tag.to_string()).filter(|t| !t.is_empty() && t != "latest")
}

/// The items of one kind in a mixed `kubectl get` list.
fn of<'a>(all: &'a [Value], kind: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
    all.iter().filter(move |i| s(i, &["kind"]) == Some(kind))
}

fn kind_house(kind: &str, name: &str) -> HouseKind {
    match kind {
        "Job" | "CronJob" => HouseKind::Chores,
        // Otherwise the name's words decide, as for issue labels: `security-scanner` is a watchtower.
        _ => house_kind(&name.split(['-', '_']).map(str::to_string).collect::<Vec<_>>()),
    }
}

/// A namespace's report from `kubectl get <workloads>,pods -o json` and
/// `kubectl get nodes -o json`.
pub fn report(source: &KubeSource, workloads: &Value, nodes: &Value) -> Report {
    let all = items(workloads);
    let name = |i: &Value| s(i, &["metadata", "name"]).unwrap_or_default().to_string();

    // Jobs a CronJob made.
    let job_cron: HashMap<String, String> = of(all, "Job")
        .filter_map(|j| {
            let owner = j.get("metadata")?.get("ownerReferences")?.as_array()?.first()?;
            (s(owner, &["kind"]) == Some("CronJob")).then(|| (name(j), s(owner, &["name"]).unwrap_or_default().to_string()))
        })
        .collect();

    // Workloads in a stable order: long-running first, then jobs.
    let mut order: Vec<(String, String)> = Vec::new();
    for kind in ["Deployment", "StatefulSet", "DaemonSet", "CronJob"] {
        order.extend(of(all, kind).map(|w| (kind.to_string(), name(w))));
    }
    order.extend(of(all, "Job").filter(|j| !job_cron.contains_key(&name(j))).map(|j| ("Job".to_string(), name(j))));

    let mut pods: BTreeMap<(String, String), Vec<&Value>> = BTreeMap::new();
    for pod in of(all, "Pod") {
        if let Some(owner) = owner_of(pod, &job_cron) {
            pods.entry(owner).or_default().push(pod);
        }
    }

    let mut builds = BTreeSet::new();
    let workstreams: Vec<Workstream> = order
        .iter()
        .map(|(kind, wname)| {
            let mine = pods.get(&(kind.clone(), wname.clone())).cloned().unwrap_or_default();
            let helpers: Vec<Agent> = mine
                .iter()
                .map(|p| {
                    let (state, why) = pod_state(p);
                    let image = p.get("spec").and_then(|s| s.get("containers")).and_then(Value::as_array).and_then(|c| c.first()).and_then(|c| s(c, &["image"])).unwrap_or_default();
                    if state == AgentState::Working {
                        if let Some(v) = image_version(image) {
                            builds.insert(v);
                        }
                    }
                    Agent {
                        name: name(p),
                        kind: Some("pod".into()),
                        state,
                        action: (state == AgentState::Working).then_some(super::village::Action::Running),
                        detail: Some(match (why, image_version(image)) {
                            (Some(w), _) => format!("{} · {w}", name(p)),
                            (None, Some(v)) => format!("{} · {v}", name(p)),
                            (None, None) => name(p),
                        }),
                    }
                })
                .collect();
            let w = of(all, kind).find(|w| name(w) == *wname).cloned().unwrap_or(Value::Null);
            let (want, ready, updated) = (n(&w, &["spec", "replicas"]), n(&w, &["status", "readyReplicas"]), n(&w, &["status", "updatedReplicas"]));
            let stuck_rollout = w
                .get("status")
                .and_then(|s| s.get("conditions"))
                .and_then(Value::as_array)
                .is_some_and(|cs| cs.iter().any(|c| s(c, &["reason"]) == Some("ProgressDeadlineExceeded")));
            let state = if stuck_rollout {
                AgentState::Waiting
            } else if helpers.iter().any(|h| h.state == AgentState::Waiting) {
                AgentState::Waiting
            } else if matches!(kind.as_str(), "Deployment" | "StatefulSet") && want == 0 {
                AgentState::Idle
            } else if matches!(kind.as_str(), "Deployment" | "StatefulSet") && updated < want {
                AgentState::Starting
            } else if helpers.iter().any(|h| h.state == AgentState::Working) {
                AgentState::Working
            } else if helpers.iter().any(|h| h.state == AgentState::Starting) {
                AgentState::Starting
            } else if helpers.is_empty() {
                AgentState::Idle
            } else if helpers.iter().all(|h| h.state == AgentState::Done) {
                AgentState::Done
            } else {
                AgentState::Stalled
            };
            let title = match kind.as_str() {
                "Deployment" | "StatefulSet" => format!("{kind} {wname} · {ready}/{want} ready{}", if stuck_rollout { " · rollout stuck" } else { "" }),
                _ => format!("{kind} {wname} · {} pods", helpers.len()),
            };
            Workstream {
                id: format!("{}/{wname}", kind.to_ascii_lowercase()),
                name: wname.clone(),
                title: Some(title),
                kind: Some(kind_house(kind, wname)),
                state: Some(state),
                agents: vec![Agent { name: wname.clone(), state, ..Agent::default() }],
                helpers,
                ..Workstream::default()
            }
        })
        .collect();

    // The hall is the node fleet.
    let node_agents: Vec<Agent> = items(nodes)
        .iter()
        .map(|nd| {
            let ready = nd
                .get("status")
                .and_then(|s| s.get("conditions"))
                .and_then(Value::as_array)
                .and_then(|cs| cs.iter().find(|c| s(c, &["type"]) == Some("Ready")))
                .and_then(|c| s(c, &["status"]));
            let cordoned = nd.get("spec").and_then(|s| s.get("unschedulable")).and_then(Value::as_bool) == Some(true);
            let (state, detail) = match (ready, cordoned) {
                (Some("True"), false) => (AgentState::Working, "Ready"),
                (Some("True"), true) => (AgentState::Idle, "Ready, cordoned"),
                (Some("False"), _) => (AgentState::Waiting, "NotReady"),
                _ => (AgentState::Unknown, "status unknown"),
            };
            Agent { name: name(nd), kind: Some("node".into()), state, detail: Some(format!("{} · {detail}", name(nd))), ..Agent::default() }
        })
        .collect();
    let ready_nodes = node_agents.iter().filter(|a| matches!(a.state, AgentState::Working | AgentState::Idle)).count();
    let coordinator = Workstream {
        id: "manager".into(),
        name: "nodes".into(),
        title: Some(format!("{ready_nodes} of {} nodes ready", node_agents.len())),
        kind: Some(HouseKind::Cottage),
        state: Some(if node_agents.iter().any(|a| a.state == AgentState::Waiting) {
            AgentState::Waiting
        } else if workstreams.iter().any(|w| w.state == Some(AgentState::Working)) {
            AgentState::Working
        } else {
            AgentState::Idle
        }),
        agents: vec![Agent { name: "nodes".into(), ..Agent::default() }],
        helpers: node_agents,
        ..Workstream::default()
    };

    // Failed and completed runs: Jobs (and pods of their own that failed).
    let jobs: Vec<&Value> = of(all, "Job").collect();
    let failed: Vec<Task> = jobs
        .iter()
        .filter(|j| n(j, &["status", "failed"]) > 0 && n(j, &["status", "succeeded"]) == 0)
        .map(|j| Task { id: format!("job/{}", name(j)), title: format!("Job {} failed", name(j)), state: TaskState::Failed, ..Task::default() })
        .chain(
            of(all, "Pod")
                .filter(|p| s(p, &["status", "phase"]) == Some("Failed") && !jobs.iter().any(|j| owner_of(p, &HashMap::new()).is_some_and(|(_, o)| o == name(j))))
                .map(|p| Task { id: format!("pod/{}", name(p)), title: format!("Pod {} {}", name(p), pod_state(p).1.unwrap_or_default()), state: TaskState::Failed, ..Task::default() }),
        )
        .collect();
    let completed: Vec<String> = jobs.iter().filter(|j| n(j, &["status", "succeeded"]) > 0).map(|j| format!("job/{}", name(j))).collect();

    Report {
        project: source.project(),
        plane: Plane::Runtime,
        framework: "kubernetes".into(),
        workflow: Some("Kubernetes".into()),
        location: source.label(),
        coordinator: Some(coordinator),
        workstreams,
        tasks: failed,
        pipeline: ReportPipeline { gate: "green".into(), stages: vec![Stage { name: "jobs completed".into(), tasks: completed }], ..ReportPipeline::default() },
        builds: builds.into_iter().collect(),
        ..Report::default()
    }
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

const TTL: Duration = Duration::from_secs(5);

pub struct KubeVillages {
    source: KubeSource,
    cache: TtlCache<(), Reading>,
}

impl KubeVillages {
    pub fn new(source: KubeSource) -> Self {
        Self { source, cache: TtlCache::new(TTL) }
    }

    async fn kubectl(&self, transport: &ServerTransport, args: &[&str]) -> Result<Value, String> {
        let mut all: Vec<String> = Vec::new();
        if let Some(c) = &self.source.context {
            all.extend(["--context".to_string(), c.clone()]);
        }
        all.extend(args.iter().map(|s| s.to_string()));
        let out = transport.output("kubectl", &all, None).await.map_err(|e| format!("{e:#}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("kubectl failed").trim().to_string());
        }
        serde_json::from_slice(&out.stdout).map_err(|e| format!("kubectl printed no JSON: {e}"))
    }

    async fn read(&self, transport: &ServerTransport) -> Reading {
        let ns = self.source.namespace.as_str();
        let workloads = self.kubectl(transport, &["get", "deployments,statefulsets,daemonsets,cronjobs,jobs,pods", "-n", ns, "-o", "json"]).await;
        let nodes = self.kubectl(transport, &["get", "nodes", "-o", "json"]).await.unwrap_or(Value::Null);
        match workloads {
            Ok(w) => to_village(&report(&self.source, &w, &nodes)),
            Err(why) => {
                let mut r = to_village(&report(&self.source, &Value::Null, &Value::Null));
                r.village.manager.status = HouseStatus::Unknown;
                r.village.board.push(Note { number: 0, title: format!("Can't read namespace {}: {why}", self.source.label()), label: "unreachable".into(), url: None });
                r
            }
        }
    }
}

impl VillageSource for KubeVillages {
    fn name(&self) -> String {
        format!("kubernetes {}", self.source.label())
    }
    fn read<'a>(&'a self, transport: &'a ServerTransport, _: &'a [SwarmSnapshot]) -> BoxFuture<'a, Vec<Reading>> {
        Box::pin(async move { vec![self.cache.get_or((), || self.read(transport)).await] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pod(name: &str, owner: (&str, &str), hash: &str, phase: &str, container: Value) -> Value {
        json!({"kind": "Pod", "metadata": {"name": name, "labels": {"pod-template-hash": hash}, "ownerReferences": [{"kind": owner.0, "name": owner.1}]},
               "spec": {"containers": [{"image": "ghcr.io/acme/api:9f3c2e1d8a7b"}]},
               "status": {"phase": phase, "containerStatuses": [container]}})
    }

    fn cluster() -> (Value, Value) {
        let ready = json!({"ready": true, "restartCount": 0, "state": {"running": {}}});
        let crash = json!({"ready": false, "restartCount": 7, "state": {"waiting": {"reason": "CrashLoopBackOff"}}, "lastState": {"terminated": {"reason": "OOMKilled"}}});
        let workloads = json!({"items": [
            {"kind": "Deployment", "metadata": {"name": "api"}, "spec": {"replicas": 2}, "status": {"readyReplicas": 1, "updatedReplicas": 2}},
            {"kind": "Deployment", "metadata": {"name": "web"}, "spec": {"replicas": 1}, "status": {"readyReplicas": 0, "updatedReplicas": 1}},
            {"kind": "CronJob", "metadata": {"name": "nightly-report"}},
            {"kind": "Job", "metadata": {"name": "nightly-report-1", "ownerReferences": [{"kind": "CronJob", "name": "nightly-report"}]}, "status": {"succeeded": 1}},
            {"kind": "Job", "metadata": {"name": "migrate"}, "status": {"failed": 2}},
            pod("api-7d9f-a", ("ReplicaSet", "api-7d9f"), "7d9f", "Running", ready.clone()),
            pod("api-7d9f-b", ("ReplicaSet", "api-7d9f"), "7d9f", "Running", crash),
            {"kind": "Pod", "metadata": {"name": "web-55c-x", "labels": {"pod-template-hash": "55c"}, "ownerReferences": [{"kind": "ReplicaSet", "name": "web-55c"}]},
             "spec": {"containers": [{"image": "web:1.2"}]},
             "status": {"phase": "Pending", "conditions": [{"type": "PodScheduled", "status": "False", "reason": "Unschedulable", "message": "0/3 nodes are available: insufficient memory"}]}},
            pod("nightly-report-1-q", ("Job", "nightly-report-1"), "", "Succeeded", json!({"ready": false, "state": {"terminated": {"reason": "Completed"}}}))
        ]});
        let nodes = json!({"items": [
            {"metadata": {"name": "node-a"}, "status": {"conditions": [{"type": "Ready", "status": "True"}]}},
            {"metadata": {"name": "node-b"}, "spec": {"unschedulable": true}, "status": {"conditions": [{"type": "Ready", "status": "True"}]}},
            {"metadata": {"name": "node-c"}, "status": {"conditions": [{"type": "Ready", "status": "False"}]}}
        ]});
        (workloads, nodes)
    }

    #[test]
    fn parses_runtime_specs() {
        assert_eq!(KubeSource::parse("prod/shop"), Some(KubeSource { context: Some("prod".into()), namespace: "shop".into() }));
        assert_eq!(KubeSource::parse("shop"), Some(KubeSource { context: None, namespace: "shop".into() }));
        assert_eq!(KubeSource::parse("prod/"), None);
        assert_eq!(KubeSource::parse("shop").unwrap().project(), "k8s:shop");
    }

    #[test]
    fn image_versions() {
        assert_eq!(image_version("ghcr.io/acme/api:9f3c2e1d8a7b").as_deref(), Some("9f3c2e1d8a7b"));
        assert_eq!(image_version("localhost:5000/api:v2").as_deref(), Some("v2"));
        assert_eq!(image_version("api@sha256:0123456789abcdef0123").as_deref(), Some("0123456789ab"));
        assert_eq!(image_version("api"), None);
        assert_eq!(image_version("api:latest"), None);
    }

    #[test]
    fn workloads_are_houses_and_stuck_pods_wave() {
        let (w, nodes) = cluster();
        let v = to_village(&report(&KubeSource::parse("shop").unwrap(), &w, &nodes)).village;
        let houses: Vec<(&str, HouseStatus, usize)> = v.houses.iter().map(|h| (h.role.as_str(), h.status, h.subagents.len())).collect();
        assert_eq!(houses, vec![("api", HouseStatus::Blocked, 2), ("web", HouseStatus::Blocked, 1), ("nightly-report", HouseStatus::Done, 1), ("migrate", HouseStatus::Idle, 0)]);
        let crash = &v.houses[0].subagents[1];
        assert_eq!(crash.description, "api-7d9f-b · CrashLoopBackOff · 7 restarts, last OOMKilled");
        assert!(v.houses[1].subagents[0].description.contains("Unschedulable: 0/3 nodes are available"));
        assert_eq!(v.houses[2].kind, HouseKind::Chores, "a CronJob is a barn");
        assert_eq!(v.builds, vec!["9f3c2e1d8a7b"], "only running pods report a version");
        assert_eq!(v.board.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(), vec!["Job migrate failed"]);
        assert_eq!(v.pipeline.stops[0].issues.len(), 1, "the CronJob's completed run");
        assert_eq!(v.manager.status, HouseStatus::Blocked, "a NotReady node needs you");
        assert_eq!(v.manager.issue_title.as_deref(), Some("2 of 3 nodes ready"));
    }

    #[test]
    fn pod_states() {
        let p = |phase: &str, extra: Value| { let mut v = json!({"status": {"phase": phase}}); if let Value::Object(m) = extra { for (k, x) in m { v["status"][k] = x; } } v };
        assert_eq!(pod_state(&p("Running", json!({"containerStatuses": [{"ready": true}]}))).0, AgentState::Working);
        assert_eq!(pod_state(&p("Running", json!({"containerStatuses": [{"ready": false}]}))).0, AgentState::Stalled);
        assert_eq!(pod_state(&p("Pending", json!({"containerStatuses": [{"state": {"waiting": {"reason": "ContainerCreating"}}}]}))).0, AgentState::Starting);
        let (st, why) = pod_state(&p("Pending", json!({"containerStatuses": [{"restartCount": 0, "state": {"waiting": {"reason": "ImagePullBackOff"}}}]})));
        assert_eq!((st, why.as_deref()), (AgentState::Waiting, Some("ImagePullBackOff · 0 restarts")));
        assert_eq!(pod_state(&json!({"metadata": {"deletionTimestamp": "x"}, "status": {"phase": "Running"}})).0, AgentState::Stopped);
    }
}
