//! Features: a parent issue split into subtasks, followed from spec to
//! production (#346). Built from the parent's feature link (which subtasks
//! make it, from GitHub or a report's `Task.parent`) and where each subtask
//! is in the village: a house, the backlog, the board, the quay or a stop.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::framework::{Task, TaskState, task_number};
use super::village::{HouseStatus, Pipeline, Promotion, StopSource, VillageSnapshot};
use crate::model::issue::{Child, FeatureLink, LinkedBy};

/// How long a live feature stays after its parent closes.
pub const LIVE_KEEP: Duration = Duration::from_secs(24 * 60 * 60);

/// Where a subtask is in the village.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Where {
    Backlog,
    /// Pinned to the board by a blocking label.
    Board,
    /// Being worked on in a house.
    House,
    /// Landed, waiting for the first stop.
    Quay,
    /// At a pipeline stop, named in `stop`.
    Stop,
    /// Closed with no landed tip found (or dropped).
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subtask {
    pub number: u32,
    pub title: String,
    /// Closed as not planned: named in the dialog, out of every total.
    pub dropped: bool,
    #[serde(rename = "where")]
    pub place: Where,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<String>,
    /// Why it's stuck: its blocking label, or `waiting` when its house waits
    /// on a person.
    #[serde(default)]
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Build {
    /// Counted subtasks at the quay or any stop.
    pub landed: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageProgress {
    pub name: String,
    /// Counted subtasks whose landed work has reached this stop.
    pub reached: usize,
    pub total: usize,
    /// The village can't tell what this stop runs.
    pub unknown: bool,
}

/// A feature as the renderer reads it (the data contract in #346).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Feature {
    pub number: u32,
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_url: Option<String>,
    pub linked_by: LinkedBy,
    pub promotion: Promotion,
    pub subtasks: Vec<Subtask>,
    pub build: Build,
    pub stages: Vec<StageProgress>,
    pub live: bool,
    /// Counted subtasks that are blocked.
    pub stuck: Vec<u32>,
}

/// Stops reached per issue (0: the quay). The pipeline's untruncated map
/// wins; a report's pipeline has none, so its quay and stops fill in.
fn reached(p: &Pipeline) -> HashMap<u32, usize> {
    let mut m: HashMap<u32, usize> = p.quay.iter().map(|n| (*n, 0)).collect();
    for (i, stop) in p.stops.iter().enumerate() {
        m.extend(stop.issues.iter().map(|n| (*n, i + 1)));
    }
    m.extend(p.reached.iter().map(|(n, r)| (*n, *r)));
    m
}

/// Features for a village, one per feature link.
pub fn build(village: &VillageSnapshot, links: &[FeatureLink]) -> Vec<Feature> {
    let reached = reached(&village.pipeline);
    let houses: HashMap<u32, HouseStatus> = village
        .houses
        .iter()
        .chain(std::iter::once(&village.manager))
        .filter_map(|h| h.issue.map(|n| (n, h.status)))
        .collect();
    let board: HashMap<u32, &str> = village.board.iter().map(|n| (n.number, n.label.as_str())).collect();
    let stops = &village.pipeline.stops;

    let subtask = |c: &Child| {
        let at = reached.get(&c.number).copied();
        let house = houses.get(&c.number).filter(|_| !c.closed);
        let note = board.get(&c.number).filter(|_| !c.closed);
        let place = match (at, house, note) {
            (Some(0), _, _) => Where::Quay,
            (Some(_), _, _) => Where::Stop,
            (None, Some(_), _) => Where::House,
            (None, None, Some(_)) => Where::Board,
            _ if c.closed => Where::Closed,
            _ => Where::Backlog,
        };
        let blocked = match (note, house) {
            (Some(label), _) => Some(label.to_string()),
            (None, Some(HouseStatus::Blocked)) => Some("waiting".to_string()),
            _ => None,
        };
        Subtask {
            number: c.number,
            title: c.title.clone(),
            dropped: c.not_planned,
            place,
            stop: at.filter(|n| *n > 0).and_then(|n| stops.get(n - 1)).map(|s| s.name.clone()),
            blocked,
        }
    };

    links
        .iter()
        .map(|link| {
            let subtasks: Vec<Subtask> = link.children.iter().map(subtask).collect();
            let counted: Vec<&Child> = link.children.iter().filter(|c| !c.not_planned).collect();
            let total = counted.len();
            let landed = counted.iter().filter(|c| reached.contains_key(&c.number)).count();
            let stages: Vec<StageProgress> = stops
                .iter()
                .enumerate()
                .map(|(i, stop)| StageProgress {
                    name: stop.name.clone(),
                    reached: counted.iter().filter(|c| reached.get(&c.number).is_some_and(|r| *r > i)).count(),
                    total,
                    unknown: stop.source == StopSource::Unknown,
                })
                .collect();
            let live = total > 0 && stages.last().is_some_and(|s| s.reached == total && !s.unknown);
            let stuck = subtasks.iter().filter(|s| !s.dropped && s.blocked.is_some()).map(|s| s.number).collect();
            Feature {
                number: link.parent,
                title: link.title.clone(),
                url: link.url.clone(),
                spec_url: link.spec_url.clone(),
                linked_by: link.linked_by,
                promotion: village.pipeline.promotion,
                subtasks,
                build: Build { landed, total },
                stages,
                live,
                stuck,
            }
        })
        .collect()
}

/// Feature links from a report's tasks: every task naming a `parent` is one
/// of that parent's subtasks. The parent's title and url come from its own
/// task when the report has it.
pub fn links_from_tasks(tasks: &[Task]) -> Vec<FeatureLink> {
    let by_number: HashMap<u32, &Task> = tasks.iter().map(|t| (task_number(&t.id), t)).collect();
    let mut links: Vec<FeatureLink> = Vec::new();
    for task in tasks {
        let Some(parent) = task.parent.as_deref().filter(|p| !p.trim().is_empty()) else {
            continue;
        };
        let number = task_number(parent);
        let child = Child {
            number: task_number(&task.id),
            title: task.title.clone(),
            url: task.url.clone().unwrap_or_default(),
            closed: task.state == TaskState::Done,
            not_planned: false,
        };
        match links.iter_mut().find(|l| l.parent == number) {
            Some(link) => link.children.push(child),
            None => {
                let own = by_number.get(&number);
                links.push(FeatureLink {
                    parent: number,
                    title: own.map(|t| t.title.clone()).unwrap_or_else(|| parent.to_string()),
                    url: own.and_then(|t| t.url.clone()).unwrap_or_default(),
                    spec_url: None,
                    linked_by: LinkedBy::SubIssues,
                    children: vec![child],
                });
            }
        }
    }
    for link in &mut links {
        link.children.sort_by_key(|c| c.number);
    }
    links.sort_by_key(|l| l.parent);
    links
}

/// Live features outlive their parent by [`LIVE_KEEP`]: a feature that was
/// live when its parent stopped being listed (closed) is kept, as last seen,
/// for one more day. Keyed by village.
#[derive(Default)]
pub struct LiveMemory {
    seen: Mutex<HashMap<String, HashMap<u32, (Feature, Option<Instant>)>>>,
}

impl LiveMemory {
    pub fn keep(&self, village: &str, mut current: Vec<Feature>, now: Instant) -> Vec<Feature> {
        let Ok(mut seen) = self.seen.lock() else {
            return current;
        };
        let remembered = seen.entry(village.to_string()).or_default();
        for f in &current {
            if f.live {
                remembered.insert(f.number, (f.clone(), None));
            } else {
                remembered.remove(&f.number);
            }
        }
        remembered.retain(|number, (feature, gone)| {
            if current.iter().any(|f| f.number == *number) {
                return true;
            }
            let since = *gone.get_or_insert(now);
            let keep = now.duration_since(since) < LIVE_KEEP;
            if keep {
                current.push(feature.clone());
            }
            keep
        });
        current.sort_by_key(|f| f.number);
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::village::{House, HouseKind, Note, Stop};

    fn child(number: u32) -> Child {
        Child { number, title: format!("Sub {number}"), url: String::new(), closed: false, not_planned: false }
    }

    fn link(children: Vec<Child>) -> FeatureLink {
        FeatureLink {
            parent: 460,
            title: "Saved carts".into(),
            url: "https://github.com/acme/app/issues/460".into(),
            spec_url: Some("https://github.com/acme/app/blob/main/docs/specs/carts.md".into()),
            linked_by: LinkedBy::SubIssues,
            children,
        }
    }

    fn house(issue: Option<u32>, status: HouseStatus) -> House {
        House {
            id: format!("wt-{}", issue.unwrap_or(0)),
            role: "worker".into(),
            kind: HouseKind::Feature,
            status,
            issue,
            issue_title: None,
            priority: None,
            crew: vec![],
            subagents: vec![],
            action: None,
            links: vec![],
            question: None,
            answer_url: None,
            branch: None,
            since: None,
        }
    }

    fn stop(name: &str, issues: &[u32], source: StopSource) -> Stop {
        Stop { name: name.into(), issues: issues.to_vec(), source, ..Stop::default() }
    }

    /// A village whose pipeline has a green ref, dev, uat and prod.
    fn village(quay: &[u32], stops: [&[u32]; 4], houses: Vec<House>, board: Vec<Note>) -> VillageSnapshot {
        let names = ["master-green", "dev", "uat", "prod"];
        VillageSnapshot {
            project: "app".into(),
            plane: Default::default(),
            framework: "autocoder".into(),
            workflow: None,
            repo_path: "/repo".into(),
            manager: house(None, HouseStatus::Idle),
            houses,
            backlog: vec![],
            board,
            pipeline: Pipeline {
                quay: quay.to_vec(),
                stops: names.iter().zip(stops).map(|(n, i)| stop(n, i, StopSource::Branch)).collect(),
                ..Pipeline::default()
            },
            builds: vec![],
            features: vec![],
        }
    }

    fn four() -> FeatureLink {
        link((461..=464).map(child).collect())
    }

    #[test]
    fn build_goes_from_none_landed_to_all_landed() {
        let v = village(&[], [&[], &[], &[], &[]], vec![house(Some(461), HouseStatus::Working)], vec![]);
        let f = &build(&v, &[four()])[0];
        assert_eq!(f.build, Build { landed: 0, total: 4 });
        assert_eq!(f.subtasks[0].place, Where::House);
        assert_eq!(f.subtasks[1].place, Where::Backlog);
        assert!(!f.live);

        let v = village(&[461, 462], [&[463], &[464], &[], &[]], vec![], vec![]);
        let f = &build(&v, &[four()])[0];
        assert_eq!(f.build, Build { landed: 4, total: 4 });
        assert_eq!(f.subtasks[0].place, Where::Quay);
        assert_eq!((f.subtasks[3].place, f.subtasks[3].stop.as_deref()), (Where::Stop, Some("dev")));
        assert_eq!(f.number, 460);
        assert_eq!(f.spec_url.as_deref(), Some("https://github.com/acme/app/blob/main/docs/specs/carts.md"));
    }

    #[test]
    fn partly_promoted_reads_uat_two_of_four() {
        let v = village(&[], [&[], &[461, 462], &[463, 464], &[]], vec![], vec![]);
        let f = &build(&v, &[four()])[0];
        let by_stage: Vec<_> = f.stages.iter().map(|s| (s.name.as_str(), s.reached, s.total)).collect();
        assert_eq!(by_stage, vec![("master-green", 4, 4), ("dev", 4, 4), ("uat", 2, 4), ("prod", 0, 4)]);
        assert!(!f.live);
    }

    #[test]
    fn all_in_prod_is_live() {
        let v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        let f = &build(&v, &[four()])[0];
        assert!(f.live);
        assert_eq!(f.stages.last().map(|s| s.reached), Some(4));
    }

    #[test]
    fn a_not_planned_subtask_leaves_the_total() {
        let mut dropped = child(464);
        dropped.closed = true;
        dropped.not_planned = true;
        let l = link(vec![child(461), child(462), child(463), dropped]);
        let v = village(&[], [&[], &[], &[], &[461, 462, 463]], vec![], vec![]);
        let f = &build(&v, &[l])[0];
        assert_eq!(f.build.total, 3);
        assert!(f.live, "the dropped subtask doesn't hold the feature back");
        let d = &f.subtasks[3];
        assert!(d.dropped);
        assert_eq!(d.place, Where::Closed);
    }

    #[test]
    fn blocked_subtasks_make_the_feature_stuck() {
        let board = vec![Note { number: 463, title: "Sub 463".into(), label: "needs-design".into(), url: None }];
        let v = village(&[461], [&[], &[], &[], &[]], vec![house(Some(462), HouseStatus::Blocked)], board);
        let f = &build(&v, &[four()])[0];
        assert_eq!(f.stuck, vec![462, 463]);
        assert_eq!(f.subtasks[2].place, Where::Board);
        assert_eq!(f.subtasks[2].blocked.as_deref(), Some("needs-design"));
        assert_eq!(f.subtasks[1].blocked.as_deref(), Some("waiting"));
        assert_eq!(f.subtasks[0].blocked, None);
    }

    #[test]
    fn unknown_uat_is_flagged_and_prod_is_not_live() {
        let mut v = village(&[], [&[], &[461, 462, 463, 464], &[], &[]], vec![], vec![]);
        v.pipeline.stops[2].source = StopSource::Unknown;
        v.pipeline.stops[3].source = StopSource::Unknown;
        let f = &build(&v, &[four()])[0];
        assert!(f.stages[2].unknown);
        assert!(!f.stages[1].unknown);
        assert!(!f.live);
    }

    #[test]
    fn more_prod_issues_than_the_castle_keeps_still_count_as_live() {
        // The last stop shows only the newest crates; the untruncated map
        // still has every subtask in prod.
        let mut v = village(&[], [&[], &[], &[], &[1, 2, 3]], vec![], vec![]);
        v.pipeline.reached = (461..=464).map(|n| (n, 4)).collect();
        let f = &build(&v, &[four()])[0];
        assert!(f.live);
        assert_eq!(f.subtasks[0].stop.as_deref(), Some("prod"));
    }

    #[test]
    fn closed_subtasks_with_no_landed_tip_are_closed() {
        let mut done = child(461);
        done.closed = true;
        let v = village(&[], [&[], &[], &[], &[]], vec![house(Some(461), HouseStatus::Idle)], vec![]);
        let f = &build(&v, &[link(vec![done])])[0];
        assert_eq!(f.subtasks[0].place, Where::Closed);
    }

    #[test]
    fn report_tasks_with_a_parent_make_a_feature() {
        let task = |id: &str, parent: Option<&str>, state: TaskState| Task {
            id: id.into(),
            title: format!("Task {id}"),
            state,
            parent: parent.map(str::to_string),
            url: Some(format!("https://tracker/{id}")),
            ..Task::default()
        };
        let tasks = vec![
            task("PROJ-1", None, TaskState::Claimed),
            task("PROJ-3", Some("PROJ-1"), TaskState::Done),
            task("PROJ-2", Some("PROJ-1"), TaskState::Open),
            task("PROJ-9", None, TaskState::Open),
            task("PROJ-7", Some("PROJ-5"), TaskState::Open),
        ];
        let links = links_from_tasks(&tasks);
        assert_eq!(links.len(), 2);
        let proj1 = links.iter().find(|l| l.parent == task_number("PROJ-1")).unwrap();
        assert_eq!(proj1.title, "Task PROJ-1");
        assert_eq!(proj1.url, "https://tracker/PROJ-1");
        assert_eq!(proj1.children.len(), 2);
        assert!(proj1.children.iter().any(|c| c.number == task_number("PROJ-3") && c.closed));
        // A parent the report doesn't list is named by its id.
        assert!(links.iter().any(|l| l.title == "PROJ-5"));
    }

    #[test]
    fn live_features_stay_a_day_after_their_parent_closes() {
        let memory = LiveMemory::default();
        let t0 = Instant::now();
        let v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        let live = build(&v, &[four()]);
        assert_eq!(memory.keep("app", live, t0).len(), 1);

        // The parent closes: it's no longer listed, but stays for a day.
        assert_eq!(memory.keep("app", vec![], t0 + Duration::from_secs(60)).len(), 1);
        assert_eq!(memory.keep("app", vec![], t0 + LIVE_KEEP).len(), 1);
        assert!(memory.keep("app", vec![], t0 + LIVE_KEEP + Duration::from_secs(61)).is_empty());
        // Other villages are separate.
        assert!(memory.keep("other", vec![], t0).is_empty());
    }

    #[test]
    fn features_that_were_never_live_just_go() {
        let memory = LiveMemory::default();
        let t0 = Instant::now();
        let v = village(&[], [&[], &[], &[], &[]], vec![], vec![]);
        memory.keep("app", build(&v, &[four()]), t0);
        assert!(memory.keep("app", vec![], t0).is_empty());
    }

    #[test]
    fn serializes_per_the_contract() {
        let v = village(&[], [&[], &[], &[461], &[]], vec![], vec![]);
        let json = serde_json::to_value(&build(&v, &[link(vec![child(461)])])[0]).unwrap();
        assert_eq!(json["linked_by"], "sub-issues");
        assert_eq!(json["promotion"], "subtask");
        assert_eq!(json["subtasks"][0]["where"], "stop");
        assert_eq!(json["subtasks"][0]["stop"], "uat");
        assert!(json["subtasks"][0]["blocked"].is_null());
        assert_eq!(json["build"], serde_json::json!({"landed": 1, "total": 1}));
        assert_eq!(json["stages"][2], serde_json::json!({"name": "uat", "reached": 1, "total": 1, "unknown": false}));
        assert_eq!(json["stuck"], serde_json::json!([]));
    }
}
