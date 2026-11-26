//! Worlds: a dev village paired with the platform village it builds.
//!
//! The dev village (a coding swarm under herdr or tmux) writes the system;
//! the platform village (a Temporal namespace) runs it. Work moves between
//! them in a loop: issues the platform raises flow down to the dev village to
//! be built, and releases flow up from the dev village to the platform.
//!
//! * An open issue carrying one of the world's incident labels came from the
//!   platform, as did a platform task carrying one. Its origin is the
//!   platform building its task's `raised_by` names, else the one whose
//!   workflow id or type its title names, else the city gate; that building
//!   shows damage. A label value ending in `:` matches every label it begins,
//!   and the matched label's part after its last `:` is the crate's channel.
//! * An incident whose task carries a `verified_at` within the window has
//!   been confirmed fixed by the platform: it no longer floats, and its
//!   building sparkles.
//! * The dev village's release is the commit at its last pipeline stop. It
//!   has landed once a platform worker reports a build ID that matches it
//!   (either is a prefix of the other, at least 7 characters). Until then the
//!   issues at that stop are on the road up to the platform.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::IssueSnapshot;
use super::village::{Health, HealthState, StopKind, VillageSnapshot};

pub const DEFAULT_INCIDENT_LABEL: &str = "incident";

/// How far back the world's counts and its verified incidents reach.
const WINDOW_HOURS: i64 = 24;

/// `--world <dev>=<platform>`: which village builds which.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldConfig {
    /// The dev village's project, e.g. `shop`.
    pub dev: String,
    /// The platform village's project, e.g. `temporal:agents`.
    pub platform: String,
    /// Labels marking an issue the platform raised; one ending in `:` is a
    /// prefix (`channel:` matches `channel:billing`).
    pub incident_labels: Vec<String>,
    /// The environment the platform runs (`shop=temporal:agents@deploy`):
    /// its village in the environment row. `None`: the last environment.
    pub environment: Option<String>,
}

impl WorldConfig {
    /// Parse `shop=temporal:agents`.
    pub fn parse(spec: &str, incident_labels: &[String]) -> Option<Self> {
        let (dev, platform) = spec.split_once('=')?;
        let (dev, platform) = (dev.trim(), platform.trim());
        let (platform, environment) = match platform.rsplit_once('@') {
            Some((p, env)) if !env.trim().is_empty() => (p.trim(), Some(env.trim().to_string())),
            _ => (platform, None),
        };
        (!dev.is_empty() && !platform.is_empty()).then(|| Self {
            dev: dev.to_string(),
            platform: platform.to_string(),
            incident_labels: incident_labels.to_vec(),
            environment,
        })
    }

    /// The label among `labels` that makes an issue an incident, trying the
    /// incident labels in the order given.
    pub fn incident_label<'a>(&self, labels: &'a [String]) -> Option<&'a str> {
        self.incident_labels.iter().find_map(|want| labels.iter().find(|l| label_matches(l, want))).map(String::as_str)
    }

    /// The channel an incident's `label` names: its part after the last `:`,
    /// or all of it. A single exact incident label names one channel for
    /// every incident, so it gives none and the crate is drawn as it always was.
    pub fn channel(&self, label: &str) -> Option<String> {
        let single = matches!(self.incident_labels.as_slice(), [one] if !one.ends_with(':'));
        (!single).then(|| label.rsplit(':').next().unwrap_or(label).to_string())
    }
}

/// `label` is `want`, or begins with it when `want` ends in `:` (and has more after it).
fn label_matches(label: &str, want: &str) -> bool {
    let (label, want) = (label.to_ascii_lowercase(), want.to_ascii_lowercase());
    if want.ends_with(':') { label.len() > want.len() && label.starts_with(&want) } else { label == want }
}

/// An issue the platform raised, on its way to (or in) the dev village.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Incident {
    pub number: u32,
    pub title: String,
    /// The platform house it came from; `None` = the city gate.
    pub origin: Option<String>,
    /// Being built: a dev house has claimed it.
    pub in_progress: bool,
    /// The part of the platform asking: its incident label after the last `:`.
    #[serde(default)]
    pub channel: Option<String>,
    /// The platform workstream that raised it, as its task says.
    #[serde(default)]
    pub raised_by: Option<String>,
    /// When the platform confirmed the fix, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
}

/// The loop within the window: incidents raised, their fixes landed and
/// deployed, and the platform's confirmations.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LoopCounts {
    pub raised: usize,
    pub landed: usize,
    pub deployed: usize,
    pub verified: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldSnapshot {
    /// `world:<dev>`, the name the picker shows.
    pub name: String,
    pub dev: String,
    pub platform: String,
    /// Issues from the platform, still open.
    pub incidents: Vec<Incident>,
    /// Incidents the platform confirmed fixed within the window.
    #[serde(default)]
    pub verified: Vec<Incident>,
    #[serde(default)]
    pub counts: LoopCounts,
    /// Platform houses with an open incident against them.
    pub damaged: Vec<String>,
    /// The commit at the dev village's last pipeline stop.
    pub release: Option<String>,
    /// Build IDs the platform's workers report.
    pub platform_builds: Vec<String>,
    /// Whether any platform worker runs the release; `None` when the platform
    /// reports no build IDs, so it can't be told.
    pub landed: Option<bool>,
    /// Issues released but not yet running on the platform: on the road up.
    pub shipping: Vec<u32>,
    /// The dev village's environment the platform is: its village in the
    /// environment row, with the platform's workloads as its houses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
}

/// Mark the environment a world's platform runs: open incidents against the
/// platform are its health.
pub fn mark_incidents(dev: &mut VillageSnapshot, world: &WorldSnapshot) {
    let (Some(env), n) = (&world.environment, world.incidents.len()) else { return };
    if n == 0 {
        return;
    }
    if let Some(stop) = dev.pipeline.stops.iter_mut().find(|s| &s.name == env) {
        stop.health = Some(Health {
            state: HealthState::Incident,
            reason: Some(format!("{n} open {}", if n == 1 { "incident" } else { "incidents" })),
            url: None,
        });
    }
}

/// One build ID matches another when either is a prefix of the other and the
/// shorter has at least 7 characters (an abbreviated git sha).
pub fn same_build(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= 7 && long.starts_with(&short)
}

/// Every issue either village lists, once by number: the dev village's,
/// with the platform's labels, `raised_by` and `verified_at` where its own
/// say nothing.
fn merged(dev: &[IssueSnapshot], platform: &[IssueSnapshot]) -> Vec<IssueSnapshot> {
    let mut out = dev.to_vec();
    for p in platform {
        match out.iter_mut().find(|i| i.number == p.number) {
            Some(i) => {
                i.labels.extend(p.labels.iter().filter(|l| !i.labels.contains(l)).cloned().collect::<Vec<_>>());
                i.raised_by = i.raised_by.take().or_else(|| p.raised_by.clone());
                i.verified_at = i.verified_at.take().or_else(|| p.verified_at.clone());
            }
            None => out.push(p.clone()),
        }
    }
    out
}

pub fn build_world(
    config: &WorldConfig,
    dev: &VillageSnapshot,
    dev_issues: &[IssueSnapshot],
    platform: &VillageSnapshot,
    platform_issues: &[IssueSnapshot],
    now: DateTime<Utc>,
) -> WorldSnapshot {
    let claimed: BTreeSet<u32> = dev.houses.iter().filter_map(|h| h.issue).collect();
    let since = now - Duration::hours(WINDOW_HOURS);
    let (mut incidents, mut verified) = (Vec::new(), Vec::new());
    for i in merged(dev_issues, platform_issues) {
        let Some(label) = config.incident_label(&i.labels) else { continue };
        let title = i.title.to_ascii_lowercase();
        // The house its task names; else a workflow id, more specific than a type.
        let origin = i
            .raised_by
            .as_deref()
            .and_then(|r| platform.houses.iter().find(|h| h.id == r))
            .or_else(|| platform.houses.iter().find(|h| title.contains(&h.id.to_ascii_lowercase())))
            .or_else(|| platform.houses.iter().find(|h| !h.role.is_empty() && title.contains(&h.role.to_ascii_lowercase())))
            .map(|h| h.id.clone());
        let at = i.verified_at.as_deref().and_then(|v| DateTime::parse_from_rfc3339(v).ok());
        let incident = Incident {
            number: i.number,
            title: i.title.clone(),
            origin,
            in_progress: claimed.contains(&i.number),
            channel: config.channel(label),
            raised_by: i.raised_by.clone(),
            verified_at: i.verified_at.clone(),
        };
        match at {
            // Confirmed before the window: done, and no longer shown.
            Some(at) if at < since => {}
            Some(_) => verified.push(incident),
            None if i.state == "open" => incidents.push(incident),
            None => {}
        }
    }
    let damaged: Vec<String> = incidents.iter().filter_map(|i| i.origin.clone()).collect::<BTreeSet<_>>().into_iter().collect();

    let release = dev.pipeline.release.clone();
    let landed = match (&release, platform.builds.is_empty()) {
        (_, true) => None,
        (None, false) => Some(false),
        (Some(r), false) => Some(platform.builds.iter().any(|b| same_build(b, r))),
    };
    let shipping = match landed {
        Some(false) => dev.pipeline.stops.last().map(|s| s.issues.clone()).unwrap_or_default(),
        _ => Vec::new(),
    };
    let stops = &dev.pipeline.stops;
    let landed_issues: BTreeSet<u32> = dev.pipeline.quay.iter().chain(stops.iter().flat_map(|s| &s.issues)).copied().collect();
    let deployed_issues: BTreeSet<u32> = stops.iter().filter(|s| s.kind == StopKind::Environment).flat_map(|s| s.issues.iter().copied()).collect();
    // A verified incident was landed and deployed too.
    let open_in = |at: &BTreeSet<u32>| incidents.iter().filter(|i| at.contains(&i.number)).count() + verified.len();
    let counts = LoopCounts {
        raised: incidents.len() + verified.len(),
        landed: open_in(&landed_issues),
        deployed: open_in(&deployed_issues),
        verified: verified.len(),
    };
    WorldSnapshot {
        name: format!("world:{}", config.dev),
        dev: config.dev.clone(),
        platform: config.platform.clone(),
        incidents,
        verified,
        counts,
        damaged,
        release,
        platform_builds: platform.builds.clone(),
        landed,
        shipping,
        environment: config
            .environment
            .clone()
            .or_else(|| dev.pipeline.stops.iter().rev().find(|s| s.kind == StopKind::Environment).map(|s| s.name.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::village::{House, HouseKind, HouseStatus, Pipeline, Stop};

    fn house(id: &str, role: &str, issue: Option<u32>) -> House {
        House {
            id: id.into(),
            role: role.into(),
            kind: HouseKind::Workshop,
            status: HouseStatus::Working,
            issue,
            crew: vec![role.into()],
            ..Default::default()
        }
    }

    fn village(project: &str, houses: Vec<House>) -> VillageSnapshot {
        VillageSnapshot {
            project: project.into(),
            manager: house("manager", "manager", None),
            houses,
            ..Default::default()
        }
    }

    fn issue(number: u32, title: &str, labels: &[&str]) -> IssueSnapshot {
        IssueSnapshot {
            number,
            title: title.into(),
            state: "open".into(),
            priority: "P1".into(),
            labels: labels.iter().map(|l| l.to_string()).collect(),
            ..Default::default()
        }
    }

    fn labels(ls: &[&str]) -> Vec<String> {
        ls.iter().map(|l| l.to_string()).collect()
    }

    /// The clock the tests read: the window ends here.
    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-20T12:00:00Z").unwrap().with_timezone(&Utc)
    }

    fn config() -> WorldConfig {
        WorldConfig::parse("shop=temporal:agents", &labels(&[DEFAULT_INCIDENT_LABEL])).unwrap()
    }

    #[test]
    fn parses_world_specs() {
        let c = config();
        assert_eq!((c.dev.as_str(), c.platform.as_str()), ("shop", "temporal:agents"));
        assert!(WorldConfig::parse("shop", &[]).is_none());
        assert!(WorldConfig::parse("=temporal:x", &[]).is_none());
    }

    #[test]
    fn build_ids_match_by_prefix() {
        assert!(same_build("9f3c2e1", "9f3c2e1d8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d"));
        assert!(same_build("9F3C2E1D", "9f3c2e1"));
        assert!(!same_build("9f3c", "9f3c2e1"), "too short to trust");
        assert!(!same_build("1a2b3c4", "9f3c2e1"));
    }

    #[test]
    fn incidents_come_from_the_platform_building_their_title_names() {
        let platform = village(
            "temporal:agents",
            vec![house("procurement-118", "ProcurementWorkstream", None), house("bugfix-52", "BugFixWorkstream", None)],
        );
        let dev = village("shop", vec![house("/src/shop-wt-1", "worker-1", Some(502))]);
        let issues = vec![
            issue(501, "procurement-118 failed: vendor API 429", &["incident", "bug"]),
            issue(502, "BugFixWorkstream bugfix-50 timed out", &["Incident"]),
            issue(503, "Payments retry storm", &["incident"]),
            issue(504, "Saved carts", &["enhancement"]),
        ];
        let w = build_world(&config(), &dev, &issues, &platform, &[], now());
        let got: Vec<(u32, Option<&str>, bool)> = w.incidents.iter().map(|i| (i.number, i.origin.as_deref(), i.in_progress)).collect();
        assert_eq!(got, vec![(501, Some("procurement-118"), false), (502, Some("bugfix-52"), true), (503, None, false)]);
        assert_eq!(w.damaged, vec!["bugfix-52", "procurement-118"]);
        assert_eq!(w.name, "world:shop");
    }

    #[test]
    fn a_platform_is_the_environment_it_runs() {
        use crate::web::village::{HealthState, StopKind};
        let mut dev = village("shop", vec![]);
        dev.pipeline.stops = vec![
            Stop { name: "master-green".into(), kind: StopKind::Gate, ..Stop::default() },
            Stop { name: "deploy".into(), ..Stop::default() },
            Stop { name: "release".into(), ..Stop::default() },
        ];
        let platform = village("temporal:agents", vec![house("procurement-118", "ProcurementWorkstream", None)]);
        // By default the platform is the production village: the last environment.
        let w = build_world(&config(), &dev, &[], &platform, &[], now());
        assert_eq!(w.environment.as_deref(), Some("release"));
        // `@env` attaches it to another, and the platform name is what comes before it.
        let at = WorldConfig::parse("shop=temporal:agents@deploy", &labels(&[DEFAULT_INCIDENT_LABEL])).unwrap();
        assert_eq!((at.platform.as_str(), at.environment.as_deref()), ("temporal:agents", Some("deploy")));
        assert_eq!(WorldConfig::parse("shop=k8s:shop-prod@", &[]).unwrap().environment, None);
        // Open incidents are that environment's health; none leaves it alone.
        let issues = vec![issue(501, "procurement-118 failed", &["incident"]), issue(502, "BugFix timed out", &["incident"])];
        let w = build_world(&at, &dev, &issues, &platform, &[], now());
        mark_incidents(&mut dev, &w);
        let h = dev.pipeline.stops[1].health.clone().unwrap();
        assert_eq!((h.state, h.reason.as_deref()), (HealthState::Incident, Some("2 open incidents")));
        assert!(dev.pipeline.stops[2].health.is_none());
        let quiet = build_world(&config(), &dev, &[], &platform, &[], now());
        let before = dev.clone();
        mark_incidents(&mut dev, &quiet);
        assert_eq!(dev, before);
        // The snapshot says so.
        assert_eq!(serde_json::to_value(&w).unwrap()["environment"], "deploy");
    }

    #[test]
    fn a_release_ships_until_a_platform_worker_runs_it() {
        let mut dev = village("shop", vec![]);
        dev.pipeline = Pipeline {
            release: Some("9f3c2e1d8a7b".into()),
            stops: vec![
                Stop { name: "master-green".into(), issues: vec![9], ..Stop::default() },
                Stop { name: "prod".into(), issues: vec![7, 8], ..Stop::default() },
            ],
            ..Pipeline::default()
        };
        let mut platform = village("temporal:agents", vec![]);
        let w = build_world(&config(), &dev, &[], &platform, &[], now());
        assert_eq!(w.landed, None, "no build IDs reported: can't tell");
        assert!(w.shipping.is_empty());

        platform.builds = vec!["1a2b3c4".into()];
        let w = build_world(&config(), &dev, &[], &platform, &[], now());
        assert_eq!(w.landed, Some(false));
        assert_eq!(w.shipping, vec![7, 8]);

        platform.builds.push("9f3c2e1".into());
        let w = build_world(&config(), &dev, &[], &platform, &[], now());
        assert_eq!(w.landed, Some(true));
        assert!(w.shipping.is_empty());
    }

    #[test]
    fn incident_labels_match_exactly_from_a_list_or_by_prefix() {
        let world = |ls: &[&str]| WorldConfig::parse("shop=temporal:agents", &labels(ls)).unwrap();
        let matched = |w: &WorldConfig, ls: &[&str]| w.incident_label(&labels(ls)).map(|l| (l.to_string(), w.channel(l)));
        // The single exact label: as before, and no channel, so the crate is drawn as it was.
        let single = world(&["incident"]);
        assert_eq!(matched(&single, &["bug", "Incident"]), Some(("Incident".into(), None)));
        assert_eq!(matched(&single, &["incidental"]), None);
        // A list: any of them, each its own channel.
        let list = world(&["sre", "ops"]);
        assert_eq!(matched(&list, &["ops"]), Some(("ops".into(), Some("ops".into()))));
        assert_eq!(matched(&list, &["bug"]), None);
        // A prefix: every label it begins, the channel after the last colon.
        let prefix = world(&["channel:"]);
        assert_eq!(matched(&prefix, &["bug", "channel:log-analyzer"]), Some(("channel:log-analyzer".into(), Some("log-analyzer".into()))));
        assert_eq!(matched(&prefix, &["team:channel:user-feedback"]), None, "a prefix starts the label");
        assert_eq!(matched(&prefix, &["channel:"]), None, "nothing after the prefix names no channel");
        // The first incident label given decides which of an issue's labels counts.
        let both = world(&["channel:", "incident"]);
        assert_eq!(matched(&both, &["incident", "channel:billing"]), Some(("channel:billing".into(), Some("billing".into()))));
        assert_eq!(matched(&both, &["incident"]), Some(("incident".into(), Some("incident".into()))));
    }

    #[test]
    fn a_task_raised_by_a_house_floats_from_it_until_the_platform_verifies_it() {
        let platform = village(
            "temporal:agents",
            vec![house("task-8812", "SupportAgent", None), house("task-8815", "ResearchAgent", None)],
        );
        let mut dev = village("shop", vec![]);
        dev.pipeline = Pipeline {
            quay: vec![61],
            stops: vec![
                Stop { name: "main-green".into(), kind: StopKind::Gate, issues: vec![62], ..Stop::default() },
                Stop { name: "prod".into(), issues: vec![63], ..Stop::default() },
            ],
            ..Pipeline::default()
        };
        let at = |hours: i64| (now() - Duration::hours(hours)).to_rfc3339();
        // The dev village's tracker has the issues; the platform's report says who raised them.
        let dev_issues = vec![
            issue(60, "Summaries too long", &["channel:log-analyzer"]),
            issue(61, "Feedback form drops replies", &["channel:user-feedback"]),
            issue(62, "Slow search", &["channel:log-analyzer"]),
            issue(63, "Retry storm", &["channel:log-analyzer"]),
            issue(64, "Saved carts", &["enhancement"]),
        ];
        // The platform's own tasks carry their channel too.
        let raised = |n: u32, by: &str, verified: Option<String>| IssueSnapshot {
            raised_by: Some(by.into()),
            verified_at: verified,
            state: "closed".into(),
            ..issue(n, "", &["channel:user-feedback"])
        };
        let platform_issues = vec![
            IssueSnapshot { state: "open".into(), labels: vec![], ..raised(60, "task-8815", None) },
            raised(65, "task-8812", Some(at(2))),
            raised(66, "task-8812", Some(at(30))),
            raised(67, "task-8815", Some("soon".into())),
        ];
        let w = build_world(&WorldConfig::parse("shop=temporal:agents", &labels(&["channel:"])).unwrap(), &dev, &dev_issues, &platform, &platform_issues, now());
        let open: Vec<(u32, Option<&str>, Option<&str>)> = w.incidents.iter().map(|i| (i.number, i.origin.as_deref(), i.channel.as_deref())).collect();
        assert_eq!(
            open,
            vec![(60, Some("task-8815"), Some("log-analyzer")), (61, None, Some("user-feedback")), (62, None, Some("log-analyzer")), (63, None, Some("log-analyzer"))]
        );
        assert_eq!(w.incidents[0].raised_by.as_deref(), Some("task-8815"));
        // Verified in the window: it sparkles its house and no longer floats; before the window, gone.
        // A closed task whose verified_at doesn't parse is neither.
        assert_eq!(w.verified.iter().map(|i| (i.number, i.origin.as_deref())).collect::<Vec<_>>(), vec![(65, Some("task-8812"))]);
        assert_eq!(w.damaged, vec!["task-8815"], "a verified incident's house is no longer on fire");
        assert_eq!(w.counts, LoopCounts { raised: 5, landed: 4, deployed: 2, verified: 1 });
        let json = serde_json::to_value(&w).unwrap();
        assert_eq!(json["counts"], serde_json::json!({"raised": 5, "landed": 4, "deployed": 2, "verified": 1}));
        assert_eq!((&json["incidents"][0]["channel"], &json["incidents"][0]["raised_by"]), (&serde_json::json!("log-analyzer"), &serde_json::json!("task-8815")));
    }
}
