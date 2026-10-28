//! Temporal as a village source.
//!
//! A namespace is a village. Every running top-level workflow (a workstream)
//! is a house, and the running workflows beneath it (child workflows, at any
//! depth) are the people working in it. The Town Hall is the worker fleet:
//! its people are the workers polling the namespace's task queues, and it
//! raises the alarm when a queue has none, because nothing on that queue can
//! make progress.
//!
//! Data comes from the frontend's HTTP API (port 7243 by default on a
//! self-hosted server) through `curl`, so it works through `--server` too.
//! The parser accepts both payload encodings the API can return: shorthand
//! JSON values, and `{metadata, data}` objects with base64 data.
//!
//! A workflow waiting on a person can't be told apart from one waiting on a
//! timer by its history alone, so agents announce it: set the keyword search
//! attribute `AgentStatus` to `waiting_for_human` while waiting for the
//! signal, and clear it after.

use std::collections::{HashMap, HashSet};

use futures::future::BoxFuture;
use serde_json::Value;

use super::SwarmSnapshot;
use super::source::{Reading, TtlCache, VillageSource};
use super::village::{Action, House, HouseKind, HouseStatus, Note, Pipeline, Stop, Subagent, VillageSnapshot, house_kind, name_action};
use crate::transport::ServerTransport;

pub const DEFAULT_HTTP_URL: &str = "http://localhost:7243";

/// Search attribute an agent sets while it waits on a person.
pub const WAITING_ATTRIBUTE: &str = "AgentStatus";
const WAITING_VALUES: &[&str] = &["waiting_for_human", "blocked"];

/// Workflows described per refresh, houses first; the rest show no activity.
const DESCRIBE_LIMIT: usize = 60;

/// What a workflow is doing, from its first pending activity, or messaging
/// while it only waits on children it started.
fn pending_action(p: Option<&Pending>) -> Option<Action> {
    let p = p?;
    match p.activity_types.first() {
        Some(name) => Some(name_action(name)),
        None if p.children > 0 => Some(Action::Messaging),
        None => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemporalSource {
    pub namespace: String,
    pub http_url: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Workflow {
    pub id: String,
    pub type_name: String,
    pub task_queue: String,
    pub status: String,
    pub parent: Option<String>,
    pub root: Option<String>,
    pub agent_status: Option<String>,
}

impl Workflow {
    fn waiting(&self) -> bool {
        self.agent_status
            .as_deref()
            .is_some_and(|s| WAITING_VALUES.iter().any(|w| s.eq_ignore_ascii_case(w)))
    }
}

/// What a top-level workflow is doing right now, from DescribeWorkflowExecution.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pending {
    pub activities: usize,
    /// Types of the pending activities, e.g. `call_llm`.
    pub activity_types: Vec<String>,
    /// Activities on a retry after a failure.
    pub retrying: usize,
    pub children: usize,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(v, |v, key| v.get(key))
}

fn str_at(v: &Value, path: &[&str]) -> Option<String> {
    at(v, path).and_then(Value::as_str).map(str::to_string)
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in input.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// A search attribute's value as text, from either payload encoding.
fn payload_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => items.first().and_then(payload_text),
        Value::Object(map) => {
            let data = map.get("data")?.as_str()?;
            let bytes = base64_decode(data)?;
            let text = String::from_utf8(bytes).ok()?;
            // json/plain payloads hold a JSON value; anything else is raw text.
            Some(match serde_json::from_str::<Value>(&text) {
                Ok(inner) => payload_text(&inner).unwrap_or(text),
                Err(_) => text,
            })
        }
        _ => None,
    }
}

fn parse_workflow(v: &Value) -> Option<Workflow> {
    Some(Workflow {
        id: str_at(v, &["execution", "workflowId"])?,
        type_name: str_at(v, &["type", "name"]).unwrap_or_default(),
        task_queue: str_at(v, &["taskQueue"]).unwrap_or_default(),
        status: str_at(v, &["status"]).unwrap_or_default(),
        parent: str_at(v, &["parentExecution", "workflowId"]),
        root: str_at(v, &["rootExecution", "workflowId"]),
        agent_status: at(v, &["searchAttributes", "indexedFields", WAITING_ATTRIBUTE]).and_then(payload_text),
    })
}

/// ListWorkflowExecutions output: `{"executions": [...]}`, or a bare array.
pub fn parse_workflows(json: &str) -> Vec<Workflow> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let list = v.get("executions").unwrap_or(&v);
    list.as_array()
        .map(|items| items.iter().filter_map(parse_workflow).collect())
        .unwrap_or_default()
}

fn number(v: Option<&Value>) -> u64 {
    match v {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

/// DescribeWorkflowExecution output.
pub fn parse_pending(json: &str) -> Option<Pending> {
    let v: Value = serde_json::from_str(json).ok()?;
    let activities = v.get("pendingActivities").and_then(Value::as_array).cloned().unwrap_or_default();
    Some(Pending {
        activities: activities.len(),
        activity_types: activities.iter().filter_map(|a| str_at(a, &["activityType", "name"])).collect(),
        retrying: activities
            .iter()
            .filter(|a| number(a.get("attempt")) > 1 && a.get("lastFailure").is_some())
            .count(),
        children: v.get("pendingChildren").and_then(Value::as_array).map_or(0, Vec::len),
    })
}

/// DescribeTaskQueue output: the identities of the workers polling it.
pub fn parse_pollers(json: &str) -> Option<Vec<String>> {
    let v: Value = serde_json::from_str(json).ok()?;
    Some(
        v.get("pollers")
            .and_then(Value::as_array)
            .map(|p| p.iter().filter_map(|p| str_at(p, &["identity"])).collect())
            .unwrap_or_default(),
    )
}

/// DescribeTaskQueue output: the build ID each poller runs, from worker
/// versioning (`deploymentOptions.buildId`, or the older
/// `workerVersionCapabilities.buildId`). Pollers without one are left out.
pub fn parse_poller_builds(json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    v.get("pollers")
        .and_then(Value::as_array)
        .map(|p| {
            p.iter()
                .filter_map(|p| str_at(p, &["deploymentOptions", "buildId"]).or_else(|| str_at(p, &["workerVersionCapabilities", "buildId"])))
                .collect()
        })
        .unwrap_or_default()
}

/// Split a workflow type into lowercase words so labels can match it:
/// `SecurityReviewWorkflow` and `security_review` both give `security`.
fn type_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if !c.is_alphanumeric() || (c.is_uppercase() && prev_lower) {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        }
        if c.is_alphanumeric() {
            word.push(c.to_ascii_lowercase());
        }
        prev_lower = c.is_lowercase();
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// Stable small number for a workflow id, used where the village model wants
/// an issue number (crates and notes) to tell items apart between refreshes.
fn id_number(id: &str) -> u32 {
    id.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

fn is_failed(status: &str) -> bool {
    let s = status.to_ascii_uppercase();
    ["FAILED", "TIMED_OUT", "TIMEDOUT", "TERMINATED"].iter().any(|f| s.contains(f))
}

// ---------------------------------------------------------------------------
// Building the village
// ---------------------------------------------------------------------------

/// The top-level workflow a running workflow belongs to.
fn root_of<'a>(wf: &'a Workflow, by_id: &HashMap<&str, &'a Workflow>) -> &'a str {
    if let Some(root) = wf.root.as_deref() {
        return root;
    }
    let mut cur = wf;
    for _ in 0..64 {
        match cur.parent.as_deref() {
            Some(p) => match by_id.get(p) {
                Some(parent) => cur = parent,
                None => return p,
            },
            None => break,
        }
    }
    &cur.id
}

pub fn build_village(
    source: &TemporalSource,
    running: &[Workflow],
    pending: &HashMap<String, Pending>,
    pollers: &HashMap<String, Vec<String>>,
    closed: &[Workflow],
) -> VillageSnapshot {
    let by_id: HashMap<&str, &Workflow> = running.iter().map(|w| (w.id.as_str(), w)).collect();
    let mut people: HashMap<&str, Vec<&Workflow>> = HashMap::new();
    for wf in running.iter().filter(|w| w.parent.is_some()) {
        people.entry(root_of(wf, &by_id)).or_default().push(wf);
    }
    let unserved = |queue: &str| pollers.get(queue).is_some_and(Vec::is_empty);

    let houses: Vec<House> = running
        .iter()
        .filter(|w| w.parent.is_none())
        .map(|root| {
            let crew = people.get(root.id.as_str()).cloned().unwrap_or_default();
            let subagents: Vec<Subagent> = crew
                .iter()
                .map(|p| Subagent {
                    agent_type: p.type_name.clone(),
                    description: p.id.clone(),
                    status: Some(if p.waiting() { HouseStatus::Blocked } else { HouseStatus::Working }),
                    action: if p.waiting() { None } else { pending_action(pending.get(&p.id)) },
                })
                .collect();
            let status = if root.waiting() || crew.iter().any(|p| p.waiting()) {
                HouseStatus::Blocked
            } else if unserved(&root.task_queue) {
                HouseStatus::Stopped
            } else {
                match pending.get(&root.id) {
                    Some(p) if p.retrying > 0 => HouseStatus::Stalled,
                    Some(p) if p.activities > 0 || p.children > 0 => HouseStatus::Working,
                    _ if !crew.is_empty() => HouseStatus::Working,
                    Some(_) => HouseStatus::Idle,
                    None => HouseStatus::Unknown,
                }
            };
            House {
                id: root.id.clone(),
                role: root.type_name.clone(),
                kind: house_kind(&type_words(&root.type_name)),
                status,
                issue: None,
                issue_title: Some(root.id.clone()),
                priority: None,
                crew: vec![root.type_name.clone()],
                subagents,
                action: if status == HouseStatus::Working { pending_action(pending.get(&root.id)) } else { None },
            }
        })
        .collect();

    // The hall is the worker fleet.
    let mut workers: Vec<String> = pollers.values().flatten().cloned().collect::<HashSet<_>>().into_iter().collect();
    workers.sort();
    let hall_status = if pollers.values().any(Vec::is_empty) {
        HouseStatus::Stalled
    } else if houses.iter().any(|h| h.status == HouseStatus::Working) {
        HouseStatus::Working
    } else {
        HouseStatus::Idle
    };
    let manager = House {
        id: "manager".to_string(),
        role: "workers".to_string(),
        kind: HouseKind::Cottage,
        status: hall_status,
        issue: None,
        issue_title: Some(format!("{} workers polling {} task queues", workers.len(), pollers.len())),
        priority: None,
        crew: vec!["workers".to_string()],
        subagents: workers
            .into_iter()
            .map(|identity| Subagent { agent_type: "worker".to_string(), description: identity, status: Some(HouseStatus::Working), action: None })
            .collect(),
        action: None,
    };

    let top_closed = closed.iter().filter(|w| w.parent.is_none());
    let (failed, completed): (Vec<&Workflow>, Vec<&Workflow>) = top_closed.partition(|w| is_failed(&w.status));
    VillageSnapshot {
        project: format!("temporal:{}", source.namespace),
        workflow: Some("Temporal".to_string()),
        repo_path: source.http_url.clone(),
        manager,
        houses,
        backlog: Vec::new(),
        board: failed
            .iter()
            .map(|w| Note { number: id_number(&w.id), title: format!("{} {}", w.type_name, w.id), label: "failed".to_string() })
            .collect(),
        pipeline: Pipeline {
            integration: String::new(),
            sentinel: "green".to_string(),
            quay: Vec::new(),
            stops: vec![Stop { name: "completed (24h)".to_string(), issues: completed.iter().map(|w| id_number(&w.id)).collect() }],
            release: None,
        },
        builds: Vec::new(),
    }
}

/// The village shown when the server can't be read.
fn unreachable(source: &TemporalSource) -> VillageSnapshot {
    let mut v = build_village(source, &[], &HashMap::new(), &HashMap::new(), &[]);
    v.manager.status = HouseStatus::Unknown;
    v.board.push(Note {
        number: 0,
        title: format!("Can't reach Temporal's HTTP API at {}", source.http_url),
        label: "unreachable".to_string(),
    });
    v
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn get(transport: &ServerTransport, url: &str) -> Option<String> {
    let args: Vec<String> = ["-sS", "--fail", "--max-time", "10", "-H", "Accept: application/json", url]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let out = transport.output("curl", &args, None).await.ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub async fn read_village(transport: &ServerTransport, source: &TemporalSource) -> VillageSnapshot {
    let base = format!("{}/api/v1/namespaces/{}", source.http_url.trim_end_matches('/'), encode(&source.namespace));
    let list = |query: String| {
        let url = format!("{base}/workflows?pageSize=1000&query={}", encode(&query));
        async move { get(transport, &url).await }
    };

    let Some(running_json) = list("ExecutionStatus=\"Running\"".to_string()).await else {
        return unreachable(source);
    };
    let running = parse_workflows(&running_json);

    // Houses first, then their people, until the limit.
    let mut pending = HashMap::new();
    let order = running.iter().filter(|w| w.parent.is_none()).chain(running.iter().filter(|w| w.parent.is_some()));
    for wf in order.take(DESCRIBE_LIMIT) {
        let url = format!("{base}/workflows/{}", encode(&wf.id));
        if let Some(p) = get(transport, &url).await.as_deref().and_then(parse_pending) {
            pending.insert(wf.id.clone(), p);
        }
    }

    let queues: HashSet<&str> = running.iter().map(|w| w.task_queue.as_str()).filter(|q| !q.is_empty()).collect();
    let mut pollers = HashMap::new();
    let mut builds = std::collections::BTreeSet::new();
    for queue in queues {
        let url = format!("{base}/task-queues/{}?taskQueueType=TASK_QUEUE_TYPE_WORKFLOW", encode(queue));
        if let Some(json) = get(transport, &url).await {
            builds.extend(parse_poller_builds(&json));
            if let Some(p) = parse_pollers(&json) {
                pollers.insert(queue.to_string(), p);
            }
        }
    }

    let since = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let closed = list(format!("ExecutionStatus!=\"Running\" AND CloseTime>\"{since}\""))
        .await
        .map(|j| parse_workflows(&j))
        .unwrap_or_default();

    let mut village = build_village(source, &running, &pending, &pollers, &closed);
    village.builds = builds.into_iter().collect();
    village
}

/// How long a namespace's village is reused before rereading it.
const TTL: std::time::Duration = std::time::Duration::from_secs(10);

/// A Temporal namespace as a village source.
pub struct TemporalVillages {
    source: TemporalSource,
    cache: TtlCache<(), VillageSnapshot>,
}

impl TemporalVillages {
    pub fn new(source: TemporalSource) -> Self {
        Self { source, cache: TtlCache::new(TTL) }
    }
}

impl VillageSource for TemporalVillages {
    fn name(&self) -> String {
        format!("temporal {} at {}", self.source.namespace, self.source.http_url)
    }
    fn read<'a>(&'a self, transport: &'a ServerTransport, _: &'a [SwarmSnapshot]) -> BoxFuture<'a, Vec<Reading>> {
        Box::pin(async move {
            let village = self.cache.get_or((), || read_village(transport, &self.source)).await;
            vec![Reading { village, issues: Vec::new() }]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src() -> TemporalSource {
        TemporalSource { namespace: "agents".into(), http_url: DEFAULT_HTTP_URL.into() }
    }

    fn wf(id: &str, ty: &str, parent: Option<&str>, root: Option<&str>) -> Workflow {
        Workflow {
            id: id.into(),
            type_name: ty.into(),
            task_queue: "agents".into(),
            status: "WORKFLOW_EXECUTION_STATUS_RUNNING".into(),
            parent: parent.map(Into::into),
            root: root.map(Into::into),
            agent_status: None,
        }
    }

    #[test]
    fn parses_list_output_in_both_payload_encodings() {
        let json = r#"{"executions":[
          {"execution":{"workflowId":"wf-1","runId":"r1"},"type":{"name":"SecurityReview"},"taskQueue":"agents",
           "status":"WORKFLOW_EXECUTION_STATUS_RUNNING",
           "searchAttributes":{"indexedFields":{"AgentStatus":{"metadata":{"encoding":"anNvbi9wbGFpbg=="},"data":"IndhaXRpbmdfZm9yX2h1bWFuIg=="}}}},
          {"execution":{"workflowId":"wf-2"},"type":{"name":"Coder"},"parentExecution":{"workflowId":"wf-1"},
           "rootExecution":{"workflowId":"wf-1"},"searchAttributes":{"indexedFields":{"AgentStatus":"working"}}}
        ]}"#;
        let list = parse_workflows(json);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].agent_status.as_deref(), Some("waiting_for_human"));
        assert!(list[0].waiting());
        assert_eq!(list[1].parent.as_deref(), Some("wf-1"));
        assert_eq!(list[1].agent_status.as_deref(), Some("working"));
        assert!(parse_workflows("not json").is_empty());
        assert_eq!(parse_workflows(r#"[{"execution":{"workflowId":"x"}}]"#).len(), 1);
    }

    #[test]
    fn parses_pending_work_and_pollers() {
        let p = parse_pending(
            r#"{"pendingActivities":[{"state":"PENDING_ACTIVITY_STATE_STARTED","attempt":1},
                {"attempt":"4","lastFailure":{"message":"429"}}],"pendingChildren":[{}]}"#,
        )
        .unwrap();
        assert_eq!((p.activities, p.retrying, p.children), (2, 1, 1));
        assert_eq!(parse_pending("{}").unwrap(), Pending::default());
        assert_eq!(parse_pollers(r#"{"pollers":[{"identity":"w1@host"}]}"#).unwrap(), vec!["w1@host"]);
        assert_eq!(parse_pollers("{}").unwrap(), Vec::<String>::new());
        let builds = parse_poller_builds(
            r#"{"pollers":[{"identity":"a","deploymentOptions":{"buildId":"9f3c2e1"}},
                {"identity":"b","workerVersionCapabilities":{"buildId":"1a2b3c4"}},{"identity":"c"}]}"#,
        );
        assert_eq!(builds, vec!["9f3c2e1", "1a2b3c4"]);
    }

    #[test]
    fn type_words_split_camel_and_snake_case() {
        assert_eq!(type_words("SecurityReviewWorkflow"), vec!["security", "review", "workflow"]);
        assert_eq!(type_words("fix_bug-v2"), vec!["fix", "bug", "v2"]);
    }

    #[test]
    fn top_level_workflows_are_houses_and_descendants_are_people() {
        let running = vec![
            wf("plan", "FeaturePlanner", None, None),
            wf("plan/a", "Coder", Some("plan"), Some("plan")),
            // No rootExecution (older server): found by walking parents.
            wf("plan/a/1", "Tester", Some("plan/a"), None),
            wf("idle", "BugTriage", None, None),
        ];
        let mut pending = HashMap::new();
        pending.insert("idle".to_string(), Pending::default());
        let v = build_village(&src(), &running, &pending, &HashMap::new(), &[]);
        assert_eq!(v.houses.len(), 2);
        assert_eq!(v.houses[0].kind, HouseKind::Feature);
        assert_eq!(v.houses[0].status, HouseStatus::Working);
        let people: Vec<&str> = v.houses[0].subagents.iter().map(|s| s.agent_type.as_str()).collect();
        assert_eq!(people, vec!["Coder", "Tester"]);
        assert_eq!(v.houses[1].kind, HouseKind::Bug);
        assert_eq!(v.houses[1].status, HouseStatus::Idle);
        assert_eq!(v.workflow.as_deref(), Some("Temporal"));
    }

    #[test]
    fn a_waiting_person_blocks_the_house() {
        let mut child = wf("w/c", "Approver", Some("w"), Some("w"));
        child.agent_status = Some("waiting_for_human".into());
        let v = build_village(&src(), &[wf("w", "Workstream", None, None), child], &HashMap::new(), &HashMap::new(), &[]);
        assert_eq!(v.houses[0].status, HouseStatus::Blocked);
        assert_eq!(v.houses[0].subagents[0].status, Some(HouseStatus::Blocked));
    }

    #[test]
    fn retries_stall_and_unserved_queues_stop() {
        let mut pending = HashMap::new();
        pending.insert("w".to_string(), Pending { activities: 1, retrying: 1, ..Pending::default() });
        let v = build_village(&src(), &[wf("w", "Workstream", None, None)], &pending, &HashMap::new(), &[]);
        assert_eq!(v.houses[0].status, HouseStatus::Stalled);

        let mut pollers = HashMap::new();
        pollers.insert("agents".to_string(), Vec::new());
        let v = build_village(&src(), &[wf("w", "Workstream", None, None)], &pending, &pollers, &[]);
        assert_eq!(v.houses[0].status, HouseStatus::Stopped, "no worker can run it");
        assert_eq!(v.manager.status, HouseStatus::Stalled);
    }

    #[test]
    fn hall_people_are_workers_and_closed_work_is_sorted() {
        let mut pollers = HashMap::new();
        pollers.insert("agents".to_string(), vec!["w1".to_string(), "w2".to_string()]);
        let mut done = wf("ok", "Feature", None, None);
        done.status = "WORKFLOW_EXECUTION_STATUS_COMPLETED".into();
        let mut bad = wf("bad", "Feature", None, None);
        bad.status = "WORKFLOW_EXECUTION_STATUS_FAILED".into();
        let child = { let mut c = wf("ok/c", "Coder", Some("ok"), Some("ok")); c.status = "WORKFLOW_EXECUTION_STATUS_COMPLETED".into(); c };
        let v = build_village(&src(), &[], &HashMap::new(), &pollers, &[done, bad, child]);
        assert_eq!(v.manager.subagents.len(), 2);
        assert_eq!(v.pipeline.stops[0].issues.len(), 1, "only top-level work ships");
        assert_eq!(v.board.len(), 1);
        assert_eq!(v.board[0].label, "failed");
    }

    #[test]
    fn pending_activities_set_actions() {
        let p = parse_pending(r#"{"pendingActivities":[{"activityType":{"name":"call_llm"},"attempt":1}]}"#).unwrap();
        assert_eq!(p.activity_types, vec!["call_llm"]);
        let mut pending = HashMap::new();
        pending.insert("w".to_string(), p);
        pending.insert("w/c".to_string(), Pending { activities: 1, activity_types: vec!["send_email".into()], ..Pending::default() });
        let v = build_village(&src(), &[wf("w", "Workstream", None, None), wf("w/c", "Notifier", Some("w"), Some("w"))], &pending, &HashMap::new(), &[]);
        assert_eq!(v.houses[0].action, Some(Action::Thinking));
        assert_eq!(v.houses[0].subagents[0].action, Some(Action::Messaging));
    }

    #[test]
    fn unreachable_server_says_so() {
        let v = unreachable(&src());
        assert_eq!(v.manager.status, HouseStatus::Unknown);
        assert_eq!(v.board[0].label, "unreachable");
    }

    #[test]
    fn encodes_query_strings() {
        assert_eq!(encode("ExecutionStatus=\"Running\""), "ExecutionStatus%3D%22Running%22");
    }
}
