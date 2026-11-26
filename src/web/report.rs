//! A village as the report that draws it (#554): what `GET /api/report`
//! serves, so a village drawn where the swarm runs can feed a village drawn
//! anywhere else through `--framework http(s)://<host>/api/report`.
//!
//! [`to_report`] inverts `framework::to_village`: parsing its report draws
//! the same houses, crates, notes, gate entries and pipeline. What the
//! village holds that a report has no field for (token use, flaky tests,
//! bounces, GitHub's summary) is left out.

use std::collections::{HashMap, HashSet};

use super::IssueSnapshot;
use super::feature::Where;
use super::framework::{Agent, AgentState, Report, ReportPipeline, ReportVoyage, SCHEMA, Stage, Task, TaskState, Workstream, ids, task_number};
use super::source::Reading;
use super::village::{House, HouseStatus, Note, Pipeline, Subagent, VillageSnapshot};

/// The report that draws `reading`'s village. Tasks come from its issues,
/// which carry the labels that decide each building, and from what the
/// village drew without an issue behind it.
pub fn to_report(reading: &Reading) -> Report {
    let v = &reading.village;
    let tasks = tasks(v, &reading.issues);
    let urls: HashMap<u32, &str> = tasks.iter().filter_map(|t| Some((task_number(&t.id), t.url.as_deref()?))).collect();
    let workstream = |h: &House| workstream(h, h.issue.and_then(|n| urls.get(&n).copied()));
    Report {
        schema: SCHEMA.to_string(),
        project: v.project.clone(),
        plane: v.plane,
        framework: v.framework.clone(),
        workflow: v.workflow.clone(),
        location: v.repo_path.clone(),
        coordinator: Some(workstream(&v.manager)),
        workstreams: v.houses.iter().map(workstream).collect(),
        pipeline: pipeline(&v.pipeline),
        tasks,
        builds: v.builds.clone(),
        build: v.build.clone(),
    }
}

/// Makes the report's relative links (`/#/swarm/...`, an answer endpoint)
/// absolute at `origin` (`http://host:port`), so they still lead to this
/// server from a village drawn on another.
pub fn rebase(report: &mut Report, origin: &str) {
    let fix = |u: &mut String| {
        if u.starts_with('/') && !u.starts_with("//") {
            u.insert_str(0, origin);
        }
    };
    for ws in report.coordinator.iter_mut().chain(report.workstreams.iter_mut()) {
        ws.links.iter_mut().for_each(|l| fix(&mut l.url));
        ws.answer_url.iter_mut().for_each(fix);
    }
    report.tasks.iter_mut().filter_map(|t| t.url.as_mut()).for_each(fix);
}

fn agent_state(s: HouseStatus) -> AgentState {
    match s {
        HouseStatus::Building => AgentState::Starting,
        HouseStatus::Working => AgentState::Working,
        HouseStatus::Idle => AgentState::Idle,
        HouseStatus::Blocked => AgentState::Waiting,
        HouseStatus::Done => AgentState::Done,
        HouseStatus::Stalled => AgentState::Stalled,
        HouseStatus::Stopped => AgentState::Stopped,
        HouseStatus::Unknown => AgentState::Unknown,
    }
}

/// A house as its workstream. `task_url` is its task's link, which the task
/// carries: `to_village` adds it after the house's own links.
fn workstream(h: &House, task_url: Option<&str>) -> Workstream {
    // The house's state is final, so the report says it rather than leaving
    // the crew to decide; the house keeps no state per crew member.
    let state = agent_state(h.status);
    let mut links = h.links.clone();
    if let Some(i) = task_url.and_then(|url| links.iter().rposition(|l| l.url == url)) {
        links.remove(i);
    }
    Workstream {
        id: h.id.clone(),
        name: h.role.clone(),
        task: h.issue.map(|n| n.to_string()),
        title: h.issue_title.clone(),
        kind: Some(h.kind),
        state: Some(state),
        action: h.action,
        // The house keeps its fullest agent's context and the limit it
        // stopped on, not whose they were: the first of the crew carries them.
        agents: h
            .crew
            .iter()
            .enumerate()
            .map(|(i, name)| Agent {
                name: name.clone(),
                state,
                context_pct: h.context_pct.filter(|_| i == 0),
                context_window: h.context_window.clone().filter(|_| i == 0),
                limit: h.limit.clone().filter(|_| i == 0),
                ..Agent::default()
            })
            .collect(),
        helpers: h.subagents.iter().map(helper).collect(),
        links,
        question: h.question.clone(),
        answer_url: h.answer_url.clone(),
        branch: h.branch.clone(),
    }
}

fn helper(s: &Subagent) -> Agent {
    let name = if s.description.is_empty() { s.agent_type.clone() } else { s.description.clone() };
    Agent {
        kind: (s.agent_type != name).then(|| s.agent_type.clone()),
        // A sub-agent read from a transcript has no status: it is running.
        state: s.status.map(agent_state).unwrap_or(AgentState::Working),
        action: s.action,
        name,
        ..Agent::default()
    }
}

/// Only `P0`..`P3`: a swarm's issue without one says `—`.
fn priority(p: &str) -> Option<String> {
    p.starts_with('P').then(|| p.to_string())
}

fn note_state(n: &Note) -> TaskState {
    if n.label == "failed" || n.label == "conflict" { TaskState::Failed } else { TaskState::Blocked }
}

/// Every task the village knows, once each: its open issues, then crates
/// and notes with no issue behind them, then work it only knows by number
/// (in the gate, or a feature's landed or closed subtask) as `done`.
fn tasks(v: &VillageSnapshot, issues: &[IssueSnapshot]) -> Vec<Task> {
    let gates: HashMap<u32, _> = v.pipeline.gates.iter().map(|g| (g.issue, &g.gate)).collect();
    let mut parents: HashMap<u32, u32> = v.features.iter().flat_map(|f| f.subtasks.iter().map(move |s| (s.number, f.number))).collect();
    parents.extend(v.backlog.iter().filter_map(|c| c.parent.map(|p| (c.number, p))));
    let board: HashMap<u32, &Note> = v.board.iter().map(|n| (n.number, n)).collect();
    let backlog: HashSet<u32> = v.backlog.iter().map(|c| c.number).collect();
    // Drawn in a house or on the canal, so neither a crate nor a note, whatever its state.
    let elsewhere: HashSet<u32> = v
        .houses
        .iter()
        .chain(std::iter::once(&v.manager))
        .filter_map(|h| h.issue)
        .chain(v.pipeline.quay.iter().copied())
        .chain(v.pipeline.stops.iter().flat_map(|s| s.issues.iter().copied()))
        .collect();

    let mut out: Vec<Task> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut push = |n: u32, t: Task| {
        if seen.insert(n) {
            out.push(Task { id: n.to_string(), parent: parents.get(&n).map(u32::to_string), gate: gates.get(&n).map(|g| (*g).clone()), ..t });
        }
    };
    for i in issues.iter().filter(|i| i.state == "open") {
        let state = match board.get(&i.number) {
            Some(note) => note_state(note),
            None if backlog.contains(&i.number) => TaskState::Open,
            None if elsewhere.contains(&i.number) && i.is_blocked => TaskState::Blocked,
            None if elsewhere.contains(&i.number) && !i.is_working => TaskState::Open,
            // Not drawn at all: taken (decomposed, or still in the gate).
            None => TaskState::Claimed,
        };
        let url = Some(i.url.clone()).filter(|u| !u.is_empty());
        let depends_on = ids(&i.depends_on);
        push(i.number, Task { title: i.title.clone(), labels: i.labels.clone(), priority: priority(&i.priority), state, url, depends_on, ..Task::default() });
    }
    for c in &v.backlog {
        let t = Task { title: c.title.clone(), priority: c.priority.clone(), url: c.url.clone(), depends_on: ids(&c.waits_on), ..Task::default() };
        push(c.number, t);
    }
    for n in &v.board {
        let labels = if crate::model::issue::BLOCKING_LABELS.contains(&n.label.as_str()) { vec![n.label.clone()] } else { Vec::new() };
        let t = Task { title: n.title.clone(), labels, priority: n.priority.clone(), url: n.url.clone(), state: note_state(n), ..Task::default() };
        push(n.number, t);
    }
    let done = |title: &str| Task { title: title.to_string(), state: TaskState::Done, ..Task::default() };
    for g in &v.pipeline.gates {
        push(g.issue, done(""));
    }
    for f in &v.features {
        for s in f.subtasks.iter().filter(|s| !s.dropped && matches!(s.place, Where::Quay | Where::Stop | Where::Closed)) {
            push(s.number, done(&s.title));
        }
        push(f.number, Task { url: Some(f.url.clone()).filter(|u| !u.is_empty()), ..done(&f.title) });
    }
    out
}

fn pipeline(p: &Pipeline) -> ReportPipeline {
    ReportPipeline {
        integration: p.integration.clone(),
        gate: p.sentinel.clone(),
        landed: ids(&p.quay),
        stages: p.stops.iter().map(Stage::from).collect(),
        release: p.release.clone(),
        promotion: p.promotion,
        merge: p.merge.clone(),
        voyages: p.voyages.iter().map(ReportVoyage::from).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::framework::{parse_reports, to_village};
    use crate::web::village::{Stop, StopKind};
    use crate::web::{AgentSnapshot, SwarmSnapshot};

    const EXAMPLE: &str = include_str!("../../tools/adapters/example-report.json");

    /// A development report with what the example lacks: tasks in the merge
    /// gate, a feature, a dependency, a train, a waiting question, a house's
    /// links, its context and a usage limit.
    const GATED: &str = r#"{
      "schema": "agents-village/v1", "project": "orchard", "framework": "autocoder", "location": "/src/orchard",
      "coordinator": { "id": "manager", "name": "manager", "agents": [{ "name": "manager", "state": "idle" }] },
      "workstreams": [
        { "id": "/src/orchard-wt-1", "task": "12", "branch": "feature/issue-12",
          "agents": [{ "name": "worker-1", "state": "waiting", "context_pct": 91, "context_window": "200k" }],
          "links": [{ "label": "Agent session", "url": "/#/swarm/orchard/agent/worker-1" }],
          "question": { "prompt": "Run it?", "kind": "yes_no", "options": [{ "key": "y", "label": "Yes" }, { "key": "n", "label": "No" }] },
          "answer_url": "/api/swarms/orchard/agents/worker-1/answer" },
        { "id": "/src/orchard-wt-2", "agents": [{ "name": "worker-2", "state": "idle", "limit": { "resets": "3pm" } }] },
        { "id": "/src/orchard-wt-3", "agents": [{ "name": "worker-3", "state": "idle" }],
          "helpers": [{ "name": "scan the tree", "kind": "Explore", "state": "running", "action": "searching" }] }
      ],
      "tasks": [
        { "id": "10", "title": "Pick fruit", "labels": ["feature"] },
        { "id": "12", "title": "Ladder wobbles", "labels": ["bug"], "priority": "P1", "parent": "10", "url": "https://tracker.example.com/12" },
        { "id": "13", "title": "Paint the ladder", "labels": ["chore"], "parent": "10", "depends_on": ["12"] },
        { "id": "14", "title": "Sort the crates", "labels": ["enhancement"], "state": "claimed",
          "gate": { "state": "gating", "landed": false, "slot": 1, "attempt": 1, "train": "train/1" } },
        { "id": "15", "title": "Fence the field", "state": "failed", "gate": { "state": "conflict", "failure_url": "https://tracker.example.com/15" } },
        { "id": "16", "title": "Which tractor?", "labels": ["needs-design"], "state": "blocked", "priority": "P2" },
        { "id": "9", "title": "Water the trees", "parent": "10", "state": "done" }
      ],
      "pipeline": {
        "integration": "main", "gate": "pending", "landed": ["9", "14"],
        "stages": [{ "name": "main-green", "kind": "gate", "tasks": [] }, { "name": "prod", "tasks": [], "source": "unknown" }],
        "merge": { "slots": 2, "busy": [1], "trains": [{ "id": "train/1", "task": "14", "members": ["14"], "state": "gating" }] },
        "promotion": "feature"
      }
    }"#;

    fn readings(json: &str) -> Vec<Reading> {
        parse_reports(json).unwrap().iter().map(to_village).collect()
    }

    fn reparse(r: &Reading) -> Reading {
        let json = serde_json::to_string(&to_report(r)).unwrap();
        let mut again = readings(&json);
        assert_eq!(again.len(), 1);
        again.remove(0)
    }

    /// #554: a village exported and read back through `--framework` draws
    /// the same houses, crates, notes, gate entries and pipeline.
    #[test]
    fn an_exported_village_reads_back_as_the_same_village() {
        for first in readings(EXAMPLE).into_iter().chain(readings(GATED)) {
            let (a, b) = (&first.village, &reparse(&first).village);
            let name = &a.project;
            assert_eq!(b.manager, a.manager, "{name}: the hall");
            assert_eq!(b.houses, a.houses, "{name}: houses, their crews and helpers");
            assert_eq!(b.backlog, a.backlog, "{name}: crates");
            assert_eq!(b.board, a.board, "{name}: notes");
            assert_eq!(b.pipeline, a.pipeline, "{name}: gates, stops, voyages, merge");
            assert_eq!(b.features, a.features, "{name}: features");
            assert_eq!((&b.builds, &b.build), (&a.builds, &a.build), "{name}: builds");
            assert_eq!(b, a, "{name}: the whole village");
        }
        // The round trip is not empty: each part above has something in it.
        let gated = &readings(GATED)[0].village;
        assert!(gated.houses.iter().any(|h| h.question.is_some() && h.context_pct == Some(91)));
        assert!(gated.houses.iter().any(|h| h.limit.is_some()));
        assert!(gated.houses.iter().any(|h| !h.subagents.is_empty()));
        assert_eq!(gated.pipeline.gates.len(), 2);
        assert!(gated.pipeline.merge.is_some() && !gated.features.is_empty());
        assert!(gated.backlog.iter().any(|c| c.parent == Some(10) && c.waits_on == vec![12]));
        assert_eq!(gated.board.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(), ["conflict", "needs-design"]);
        let larkspur = &readings(EXAMPLE)[0].village;
        assert!(!larkspur.pipeline.stops.is_empty() && !larkspur.pipeline.voyages.is_empty() && larkspur.build.is_some());
    }

    /// A swarm's village, drawn from its agents and issues, exports as an
    /// `agents-village/v1` report that draws it again.
    #[test]
    fn a_swarm_village_exports_a_versioned_report() {
        let agent = |role: &str, state: &str, worktree: &str| AgentSnapshot {
            id: format!("orchard/{role}"),
            role: role.to_string(),
            state: state.to_string(),
            is_manager: role == "manager",
            health: "Healthy".to_string(),
            worktree_path: worktree.to_string(),
            ..Default::default()
        };
        let swarm = SwarmSnapshot {
            project_name: "orchard".to_string(),
            repo_path: "/repos/orchard".to_string(),
            manager: agent("manager", "Idle", "/repos/orchard"),
            workers: vec![agent("worker-1", "Working #7", "/repos/orchard-wt-1"), agent("worker-2", "Idle", "/repos/orchard-wt-2")],
            ..Default::default()
        };
        let issue = |number: u32, labels: &[&str]| IssueSnapshot {
            number,
            title: format!("Issue {number}"),
            state: "open".to_string(),
            priority: "—".to_string(),
            labels: labels.iter().map(|l| l.to_string()).collect(),
            is_blocked: labels.contains(&"needs-design"),
            url: format!("https://tracker.example.com/{number}"),
            ..Default::default()
        };
        let issues = vec![issue(7, &["bug"]), issue(8, &["needs-design"]), issue(9, &["enhancement"]), issue(11, &["merging"])];
        let pipeline = Pipeline {
            integration: "main".to_string(),
            sentinel: "green".to_string(),
            quay: vec![11],
            stops: vec![Stop { name: "main-green".to_string(), kind: StopKind::Gate, issues: vec![5], ..Default::default() }],
            ..Default::default()
        };
        let first = Reading { village: crate::web::village::build(&swarm, &issues, pipeline), issues };

        let json = serde_json::to_value(to_report(&first)).unwrap();
        assert_eq!(json["schema"], "agents-village/v1");
        assert_eq!(json["project"], "orchard");
        assert_eq!(json["workstreams"][0]["task"], "7");
        assert!(json["tasks"].as_array().unwrap().iter().all(|t| t.get("priority").is_none()), "`—` is no priority");
        assert!(!json.to_string().contains("\"\""), "no empty strings standing in for what it can't say: {json}");

        let (a, b) = (&first.village, &reparse(&first).village);
        assert_eq!((&b.manager, &b.houses), (&a.manager, &a.houses));
        assert_eq!(b.houses[0].kind, crate::web::village::HouseKind::Bug, "its issue's labels still decide the building");
        assert_eq!((&b.backlog, &b.board, &b.pipeline.quay, &b.pipeline.stops), (&a.backlog, &a.board, &a.pipeline.quay, &a.pipeline.stops));
    }

    #[test]
    fn relative_links_lead_back_to_the_exporting_server() {
        let mut report = to_report(&readings(GATED)[0]);
        rebase(&mut report, "http://host:7914");
        let house = &report.workstreams[0];
        assert_eq!(house.links[0].url, "http://host:7914/#/swarm/orchard/agent/worker-1");
        assert_eq!(house.answer_url.as_deref(), Some("http://host:7914/api/swarms/orchard/agents/worker-1/answer"));
        let task = report.tasks.iter().find(|t| t.id == "12").unwrap();
        assert_eq!(task.url.as_deref(), Some("https://tracker.example.com/12"), "absolute links are left alone");
    }
}
