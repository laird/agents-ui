//! Features: a parent issue split into subtasks, followed from spec to
//! production (#346). Built from the parent's feature link (which subtasks
//! make it, from GitHub or a report's `Task.parent`) and where each subtask
//! is in the village: a house, the backlog, the board, the quay or a stop.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::framework::{Task, TaskState, task_number};
use super::deployments::VoyageState;
use super::village::{Crate, HouseStatus, Pipeline, Promotion, StopSource, VillageSnapshot};
use crate::model::issue::{Child, FeatureLink, LinkedBy};

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

impl Feature {
    /// Whether the feature still has work in flight, and so a flag (#401):
    /// at least one counted subtask is specified (backlog, board), being
    /// worked on (a house), or landed short of the last stop. A subtask at
    /// the last stop is deployed, unless that stop is unknown; one closed
    /// with no landed tip has nothing left to deploy.
    pub fn in_flight(&self) -> bool {
        let last = self.stages.last();
        self.subtasks.iter().filter(|s| !s.dropped).any(|s| match s.place {
            Where::Backlog | Where::Board | Where::House | Where::Quay => true,
            Where::Stop => last.is_none_or(|l| l.unknown || s.stop.as_deref() != Some(l.name.as_str())),
            Where::Closed => false,
        })
    }
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

/// The features a village flags: those still in flight
/// ([`Feature::in_flight`]). One whose every subtask is deployed goes at once.
pub fn build(village: &VillageSnapshot, links: &[FeatureLink]) -> Vec<Feature> {
    follow(village, links).into_iter().filter(Feature::in_flight).collect()
}

/// Every feature link followed through the village, in flight or not.
fn follow(village: &VillageSnapshot, links: &[FeatureLink]) -> Vec<Feature> {
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

/// An epic whose last subtask reached the last stop today (#482): the village
/// sets off fireworks when it arrives, and its flag flies on the hall for the
/// rest of the day.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shipped {
    pub number: u32,
    pub title: String,
    pub url: String,
    /// When the voyage that made it live docked, RFC 3339.
    pub at: String,
}

/// Features that are live (every counted subtask at the last stop) and whose
/// subtasks rode a voyage that docked there: when the newest such voyage
/// arrived is when the feature shipped. A docked voyage is reported for an
/// hour (`DOCKED_FOR_SECS`), so this sees a shipping only that long;
/// [`remember_shipped`] keeps it for the rest of the day. A voyage's cargo
/// may be its whole history, so this alone would ship every live epic again
/// on each deploy: [`ShipMemory`] lets only an epic seen short of it ship.
pub fn shipped(village: &VillageSnapshot, links: &[FeatureLink]) -> Vec<Shipped> {
    let Some(last) = village.pipeline.stops.last() else {
        return Vec::new();
    };
    let docked: Vec<_> = village
        .pipeline
        .voyages
        .iter()
        .filter(|v| v.to == last.name && v.state == VoyageState::Docked)
        .filter_map(|v| v.finished_at.as_deref().map(|at| (v, at)))
        .collect();
    follow(village, links)
        .into_iter()
        .filter(|f| f.live)
        .filter_map(|f| {
            let counted: Vec<u32> = f.subtasks.iter().filter(|s| !s.dropped).map(|s| s.number).collect();
            let at = docked
                .iter()
                .filter(|(v, _)| v.issues.iter().any(|n| counted.contains(n)))
                .map(|(_, at)| *at)
                .max_by_key(|at| chrono::DateTime::parse_from_rfc3339(at).ok())?;
            Some(Shipped { number: f.number, title: f.title, url: f.url, at: at.to_string() })
        })
        .collect()
}

/// Per repo, what tells an epic shipping now from one that shipped long ago
/// (#482). A voyage cannot say which of its issues are new: an earlier
/// deployment marked `inactive` is not its `previous`, so it carries its whole
/// history. Only an epic this memory has seen short of the last stop can ship.
/// It is kept in memory only: after a restart, today's flags are gone, and an
/// epic that deploys before the server has seen it short of the stop never ships
/// (a missed celebration, never a false one).
#[derive(Debug, Default)]
pub struct ShipMemory {
    /// Epics seen short of the last stop, with their links: one closed and
    /// gone from the recently closed parents by the time it deploys still ships.
    pending: std::collections::BTreeMap<u32, FeatureLink>,
    /// What shipped today.
    today: Vec<Shipped>,
}

/// How many epics short of the last stop [`ShipMemory`] keeps; past it, the
/// lowest-numbered go first (each reading puts back those still short).
const PENDING_KEEP: usize = 200;

impl ShipMemory {
    /// What shipped today, after this reading of the village and its links.
    pub fn read(&mut self, village: &VillageSnapshot, links: &[FeatureLink], today: chrono::NaiveDate) -> Vec<Shipped> {
        let mut all = links.to_vec();
        for (n, link) in &self.pending {
            if !all.iter().any(|l| l.parent == *n) {
                all.push(link.clone());
            }
        }
        // A reading that can't tell what the last stop runs says nothing about
        // what is short of it: every epic would look short, and all ship next time.
        let known = village.pipeline.stops.last().is_some_and(|s| s.source != StopSource::Unknown);
        let seen = match known {
            true => shipped(village, &all).into_iter().filter(|s| self.pending.contains_key(&s.number)).collect(),
            false => Vec::new(),
        };
        for f in follow(village, &all).into_iter().filter(|_| known) {
            if f.live || f.build.total == 0 {
                self.pending.remove(&f.number);
            } else if let Some(link) = all.iter().find(|l| l.parent == f.number) {
                self.pending.insert(f.number, link.clone());
            }
        }
        while self.pending.len() > PENDING_KEEP {
            self.pending.pop_first();
        }
        remember_shipped(&mut self.today, seen, today)
    }
}

/// What shipped today, newest first: `remembered` (kept between readings)
/// with `seen` added, and anything that shipped before `today` (local) let go.
/// A feature seen again keeps when it first shipped.
pub fn remember_shipped(remembered: &mut Vec<Shipped>, seen: Vec<Shipped>, today: chrono::NaiveDate) -> Vec<Shipped> {
    for s in seen {
        if !remembered.iter().any(|r| r.number == s.number) {
            remembered.push(s);
        }
    }
    let at = |s: &Shipped| chrono::DateTime::parse_from_rfc3339(&s.at).ok();
    remembered.retain(|s| at(s).is_some_and(|t| t.with_timezone(&chrono::Local).date_naive() == today));
    remembered.sort_by_key(|s| std::cmp::Reverse(at(s)));
    remembered.clone()
}

/// Fill each backlog crate's `parent` from the feature links: the feature
/// whose subtasks include it. A crate that already names its parent (a
/// report task's `parent`) keeps it.
pub fn attach_parents(backlog: &mut [Crate], links: &[FeatureLink]) {
    let parent_of: HashMap<u32, u32> = links.iter().flat_map(|l| l.children.iter().map(move |c| (c.number, l.parent))).collect();
    for c in backlog.iter_mut().filter(|c| c.parent.is_none()) {
        c.parent = parent_of.get(&c.number).copied();
    }
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
                    closed: false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::village::{House, HouseKind, Note, Stop};

    #[test]
    fn backlog_crates_learn_their_feature() {
        let crate_n = |number: u32, parent: Option<u32>| Crate {
            number,
            kind: HouseKind::Feature,
            parent,
            ..Default::default()
        };
        let mut backlog = vec![crate_n(461, None), crate_n(462, Some(7)), crate_n(500, None)];
        attach_parents(&mut backlog, &[link(vec![child(461), child(462)])]);
        assert_eq!(backlog[0].parent, Some(460), "a subtask of #460");
        assert_eq!(backlog[1].parent, Some(7), "a parent the report named is kept");
        assert_eq!(backlog[2].parent, None, "not a subtask of anything");
    }

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
            closed: false,
        }
    }

    fn house(issue: Option<u32>, status: HouseStatus) -> House {
        House {
            id: format!("wt-{}", issue.unwrap_or(0)),
            role: "worker".into(),
            kind: HouseKind::Feature,
            status,
            issue,
            ..Default::default()
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
            framework: "autocoder".into(),
            repo_path: "/repo".into(),
            manager: house(None, HouseStatus::Idle),
            houses,
            board,
            pipeline: Pipeline {
                quay: quay.to_vec(),
                stops: names.iter().zip(stops).map(|(n, i)| stop(n, i, StopSource::Branch)).collect(),
                ..Pipeline::default()
            },
            ..Default::default()
        }
    }

    fn four() -> FeatureLink {
        link((461..=464).map(child).collect())
    }

    #[test]
    fn build_goes_from_none_landed_to_all_landed() {
        let v = village(&[], [&[], &[], &[], &[]], vec![house(Some(461), HouseStatus::Working)], vec![]);
        let f = &follow(&v, &[four()])[0];
        assert_eq!(f.build, Build { landed: 0, total: 4 });
        assert_eq!(f.subtasks[0].place, Where::House);
        assert_eq!(f.subtasks[1].place, Where::Backlog);
        assert!(!f.live);

        let v = village(&[461, 462], [&[463], &[464], &[], &[]], vec![], vec![]);
        let f = &follow(&v, &[four()])[0];
        assert_eq!(f.build, Build { landed: 4, total: 4 });
        assert_eq!(f.subtasks[0].place, Where::Quay);
        assert_eq!((f.subtasks[3].place, f.subtasks[3].stop.as_deref()), (Where::Stop, Some("dev")));
        assert_eq!(f.number, 460);
        assert_eq!(f.spec_url.as_deref(), Some("https://github.com/acme/app/blob/main/docs/specs/carts.md"));
    }

    #[test]
    fn partly_promoted_reads_uat_two_of_four() {
        let v = village(&[], [&[], &[461, 462], &[463, 464], &[]], vec![], vec![]);
        let f = &follow(&v, &[four()])[0];
        let by_stage: Vec<_> = f.stages.iter().map(|s| (s.name.as_str(), s.reached, s.total)).collect();
        assert_eq!(by_stage, vec![("master-green", 4, 4), ("dev", 4, 4), ("uat", 2, 4), ("prod", 0, 4)]);
        assert!(!f.live);
    }

    #[test]
    fn all_in_prod_is_live() {
        let v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        let f = &follow(&v, &[four()])[0];
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
        let f = &follow(&v, &[l])[0];
        assert_eq!(f.build.total, 3);
        assert!(f.live, "the dropped subtask doesn't hold the feature back");
        let d = &f.subtasks[3];
        assert!(d.dropped);
        assert_eq!(d.place, Where::Closed);
    }

    #[test]
    fn blocked_subtasks_make_the_feature_stuck() {
        let board = vec![Note { number: 463, title: "Sub 463".into(), label: "needs-design".into(), ..Default::default() }];
        let v = village(&[461], [&[], &[], &[], &[]], vec![house(Some(462), HouseStatus::Blocked)], board);
        let f = &follow(&v, &[four()])[0];
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
        let f = &follow(&v, &[four()])[0];
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
        let f = &follow(&v, &[four()])[0];
        assert!(f.live);
        assert_eq!(f.subtasks[0].stop.as_deref(), Some("prod"));
    }

    #[test]
    fn closed_subtasks_with_no_landed_tip_are_closed() {
        let mut done = child(461);
        done.closed = true;
        let v = village(&[], [&[], &[], &[], &[]], vec![house(Some(461), HouseStatus::Idle)], vec![]);
        let f = &follow(&v, &[link(vec![done])])[0];
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

    fn flagged(v: &VillageSnapshot, l: FeatureLink) -> bool {
        !build(v, &[l]).is_empty()
    }

    #[test]
    fn specified_or_worked_on_subtasks_keep_the_flag() {
        let none = village(&[], [&[], &[], &[], &[]], vec![], vec![]);
        assert!(flagged(&none, four()), "backlog");
        let board = vec![Note { number: 461, title: "Sub 461".into(), label: "needs-design".into(), ..Default::default() }];
        let pinned = village(&[], [&[], &[], &[], &[462, 463, 464]], vec![], board);
        assert!(flagged(&pinned, four()), "board");
        let worked = village(&[], [&[], &[], &[], &[462, 463, 464]], vec![house(Some(461), HouseStatus::Working)], vec![]);
        assert!(flagged(&worked, four()), "house");
    }

    #[test]
    fn landed_but_not_deployed_keeps_the_flag() {
        let closed = |n| Child { closed: true, ..child(n) };
        let all_closed = || link((461..=464).map(closed).collect());
        let quay = village(&[461], [&[], &[], &[], &[462, 463, 464]], vec![], vec![]);
        assert!(flagged(&quay, all_closed()), "one at the quay");
        let uat = village(&[], [&[], &[], &[461], &[462, 463, 464]], vec![], vec![]);
        assert!(flagged(&uat, all_closed()), "one at an earlier stop");
    }

    #[test]
    fn deployed_features_go_at_once() {
        let v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        assert!(follow(&v, &[four()])[0].live);
        assert!(build(&v, &[four()]).is_empty(), "no afterglow once every subtask is in prod");
        // The untruncated map says so too.
        let mut v = village(&[], [&[], &[], &[], &[]], vec![], vec![]);
        v.pipeline.reached = (461..=464).map(|n| (n, 4)).collect();
        assert!(build(&v, &[four()]).is_empty());
    }

    #[test]
    fn nothing_specified_has_no_flag() {
        let v = village(&[], [&[], &[], &[], &[]], vec![], vec![]);
        assert!(!flagged(&v, link(vec![])));
        let dropped = Child { closed: true, not_planned: true, ..child(461) };
        assert!(!flagged(&v, link(vec![dropped])), "only a not-planned subtask");
    }

    #[test]
    fn closed_without_landing_has_no_flag() {
        let closed = |n| Child { closed: true, ..child(n) };
        let v = village(&[], [&[], &[], &[], &[]], vec![], vec![]);
        assert!(!flagged(&v, link(vec![closed(461), closed(462)])));
        // A not-planned subtask left in the backlog doesn't hold it up either.
        let v = village(&[], [&[], &[], &[], &[461]], vec![], vec![]);
        let dropped = Child { not_planned: true, ..child(462) };
        assert!(!flagged(&v, link(vec![closed(461), dropped])));
    }

    #[test]
    fn an_unknown_last_stop_keeps_the_flag() {
        let mut v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        v.pipeline.stops[3].source = StopSource::Unknown;
        assert!(flagged(&v, four()));
    }

    #[test]
    fn serializes_per_the_contract() {
        let v = village(&[], [&[], &[], &[461], &[]], vec![], vec![]);
        let json = serde_json::to_value(&follow(&v, &[link(vec![child(461)])])[0]).unwrap();
        assert_eq!(json["linked_by"], "sub-issues");
        assert_eq!(json["promotion"], "subtask");
        assert_eq!(json["subtasks"][0]["where"], "stop");
        assert_eq!(json["subtasks"][0]["stop"], "uat");
        assert!(json["subtasks"][0]["blocked"].is_null());
        assert_eq!(json["build"], serde_json::json!({"landed": 1, "total": 1}));
        assert_eq!(json["stages"][2], serde_json::json!({"name": "uat", "reached": 1, "total": 1, "unknown": false}));
        assert_eq!(json["stuck"], serde_json::json!([]));
    }

    fn docked(to: &str, issues: &[u32], at: &str) -> crate::web::village::Voyage {
        crate::web::village::Voyage {
            from: "uat".into(),
            to: to.into(),
            state: VoyageState::Docked,
            issues: issues.to_vec(),
            finished_at: Some(at.into()),
            ..Default::default()
        }
    }

    #[test]
    fn an_epic_ships_when_the_voyage_carrying_its_last_subtasks_docks_in_prod() {
        let mut v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        v.pipeline.voyages = vec![docked("prod", &[463, 464, 999], "2026-11-16T10:00:00Z")];
        let got = shipped(&v, &[four()]);
        assert_eq!(got, vec![Shipped {
            number: 460,
            title: "Saved carts".into(),
            url: "https://github.com/acme/app/issues/460".into(),
            at: "2026-11-16T10:00:00Z".into()
        }]);
        // Its newest voyage is the one that counts.
        v.pipeline.voyages.push(docked("prod", &[461], "2026-11-16T11:30:00+00:00"));
        assert_eq!(shipped(&v, &[four()])[0].at, "2026-11-16T11:30:00+00:00");
    }

    #[test]
    fn nothing_ships_short_of_prod_or_without_a_docked_voyage() {
        // One subtask still in uat: not live.
        let mut v = village(&[], [&[], &[], &[461], &[462, 463, 464]], vec![], vec![]);
        v.pipeline.voyages = vec![docked("prod", &[462], "2026-11-16T10:00:00Z")];
        assert!(shipped(&v, &[four()]).is_empty());
        // Live, but the voyage docked in uat, or is still under sail, or carried none of it.
        let mut v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        v.pipeline.voyages = vec![docked("uat", &[461], "2026-11-16T10:00:00Z")];
        assert!(shipped(&v, &[four()]).is_empty());
        v.pipeline.voyages = vec![crate::web::village::Voyage { state: VoyageState::Sailing, ..docked("prod", &[461], "2026-11-16T10:00:00Z") }];
        assert!(shipped(&v, &[four()]).is_empty());
        v.pipeline.voyages = vec![docked("prod", &[999], "2026-11-16T10:00:00Z")];
        assert!(shipped(&v, &[four()]).is_empty());
    }

    #[test]
    fn a_closed_epic_still_ships() {
        let mut v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        v.pipeline.voyages = vec![docked("prod", &[464], "2026-11-16T10:00:00Z")];
        let closed = FeatureLink { closed: true, ..four() };
        assert_eq!(shipped(&v, &[closed]).len(), 1);
    }

    #[test]
    fn what_shipped_is_kept_for_the_rest_of_the_local_day() {
        let local = |h: u32| {
            use chrono::TimeZone;
            chrono::Local.with_ymd_and_hms(2026, 11, 16, h, 0, 0).unwrap().to_rfc3339()
        };
        let one = |n: u32, at: String| Shipped { number: n, title: format!("Epic {n}"), url: String::new(), at };
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let mut memory = Vec::new();
        let got = remember_shipped(&mut memory, vec![one(460, local(9))], today);
        assert_eq!(got.len(), 1);
        // Once its voyage is no longer reported, it is still remembered; a
        // later one goes first, and seeing 460 again keeps its first time.
        let got = remember_shipped(&mut memory, vec![one(470, local(15)), one(460, local(16))], today);
        assert_eq!(got.iter().map(|s| (s.number, s.at.clone())).collect::<Vec<_>>(), vec![(470, local(15)), (460, local(9))]);
        // The next day it is gone.
        let tomorrow = today.succ_opt().unwrap();
        assert!(remember_shipped(&mut memory, vec![], tomorrow).is_empty());
        assert!(memory.is_empty());
    }

    #[test]
    fn only_an_epic_seen_short_of_prod_ships_and_a_later_deploy_does_not_ship_it_again() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        let mut memory = ShipMemory::default();
        // Already live when first seen, its subtasks in a deploy's whole-history cargo: not new.
        let mut v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        v.pipeline.voyages = vec![docked("prod", &[461, 462, 463, 464], &now)];
        assert!(memory.read(&v, &[four()], today).is_empty());
        // Another epic in uat, then in prod: it ships, and the old one still doesn't.
        let other = FeatureLink { parent: 470, title: "Other".into(), ..link((471..=472).map(child).collect()) };
        let mut v = village(&[], [&[], &[], &[471, 472], &[461, 462, 463, 464]], vec![], vec![]);
        assert!(memory.read(&v, &[four(), other.clone()], today).is_empty());
        v.pipeline.stops[2].issues.clear();
        v.pipeline.stops[3].issues.extend([471, 472]);
        v.pipeline.voyages = vec![docked("prod", &[461, 462, 463, 464, 471, 472], &now)];
        let got = memory.read(&v, &[four(), other.clone()], today);
        assert_eq!(got.iter().map(|s| s.number).collect::<Vec<_>>(), vec![470]);
        // Remembered for the day; not shipped a second time.
        assert_eq!(memory.read(&v, &[four(), other], today).len(), 1);
    }

    #[test]
    fn an_epic_gone_from_the_links_by_the_time_it_deploys_still_ships() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let mut memory = ShipMemory::default();
        let mut v = village(&[], [&[], &[], &[461, 462, 463, 464], &[]], vec![], vec![]);
        assert!(memory.read(&v, &[four()], today).is_empty());
        v.pipeline.stops[2].issues.clear();
        v.pipeline.stops[3].issues.extend([461, 462, 463, 464]);
        v.pipeline.voyages = vec![docked("prod", &[464], &chrono::Local::now().to_rfc3339())];
        assert_eq!(memory.read(&v, &[], today).iter().map(|s| s.number).collect::<Vec<_>>(), vec![460]);
    }

    #[test]
    fn a_reading_that_cannot_tell_what_prod_runs_ships_nothing_later() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let mut memory = ShipMemory::default();
        let mut v = village(&[], [&[], &[], &[], &[461, 462, 463, 464]], vec![], vec![]);
        v.pipeline.voyages = vec![docked("prod", &[461, 462, 463, 464], &chrono::Local::now().to_rfc3339())];
        assert!(memory.read(&v, &[four()], today).is_empty());
        // The deployments can't be read: prod is unknown, and the epic looks short of it.
        let mut blind = v.clone();
        blind.pipeline.stops[3].source = StopSource::Unknown;
        assert!(memory.read(&blind, &[four()], today).is_empty());
        // Back again, with the whole-history voyage: still not shipped.
        assert!(memory.read(&v, &[four()], today).is_empty());
    }
}
