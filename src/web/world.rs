//! Worlds: a dev village paired with the platform village it builds.
//!
//! The dev village (a coding swarm under herdr or tmux) writes the system;
//! the platform village (a Temporal namespace) runs it. Work moves between
//! them in a loop: issues the platform raises flow down to the dev village to
//! be built, and releases flow up from the dev village to the platform.
//!
//! * An open issue carrying the world's incident label came from the
//!   platform. Its origin is the platform building whose workflow id or type
//!   its title names, else the city gate; that building shows damage.
//! * The dev village's release is the commit at its last pipeline stop. It
//!   has landed once a platform worker reports a build ID that matches it
//!   (either is a prefix of the other, at least 7 characters). Until then the
//!   issues at that stop are on the road up to the platform.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::IssueSnapshot;
use super::village::VillageSnapshot;

pub const DEFAULT_INCIDENT_LABEL: &str = "incident";

/// `--world <dev>=<platform>`: which village builds which.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldConfig {
    /// The dev village's project, e.g. `shop`.
    pub dev: String,
    /// The platform village's project, e.g. `temporal:agents`.
    pub platform: String,
    pub incident_label: String,
}

impl WorldConfig {
    /// Parse `shop=temporal:agents`.
    pub fn parse(spec: &str, incident_label: &str) -> Option<Self> {
        let (dev, platform) = spec.split_once('=')?;
        let (dev, platform) = (dev.trim(), platform.trim());
        (!dev.is_empty() && !platform.is_empty()).then(|| Self {
            dev: dev.to_string(),
            platform: platform.to_string(),
            incident_label: incident_label.to_string(),
        })
    }
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldSnapshot {
    /// `world:<dev>`, the name the picker shows.
    pub name: String,
    pub dev: String,
    pub platform: String,
    /// Issues from the platform, still open.
    pub incidents: Vec<Incident>,
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
}

/// One build ID matches another when either is a prefix of the other and the
/// shorter has at least 7 characters (an abbreviated git sha).
pub fn same_build(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= 7 && long.starts_with(&short)
}

pub fn build_world(config: &WorldConfig, dev: &VillageSnapshot, dev_issues: &[IssueSnapshot], platform: &VillageSnapshot) -> WorldSnapshot {
    let claimed: BTreeSet<u32> = dev.houses.iter().filter_map(|h| h.issue).collect();
    let incidents: Vec<Incident> = dev_issues
        .iter()
        .filter(|i| i.state == "open" && i.labels.iter().any(|l| l.eq_ignore_ascii_case(&config.incident_label)))
        .map(|i| {
            let title = i.title.to_ascii_lowercase();
            // A workflow id is more specific than a type, so try ids first.
            let origin = platform
                .houses
                .iter()
                .find(|h| title.contains(&h.id.to_ascii_lowercase()))
                .or_else(|| platform.houses.iter().find(|h| !h.role.is_empty() && title.contains(&h.role.to_ascii_lowercase())))
                .map(|h| h.id.clone());
            Incident { number: i.number, title: i.title.clone(), origin, in_progress: claimed.contains(&i.number) }
        })
        .collect();
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
    WorldSnapshot {
        name: format!("world:{}", config.dev),
        dev: config.dev.clone(),
        platform: config.platform.clone(),
        incidents,
        damaged,
        release,
        platform_builds: platform.builds.clone(),
        landed,
        shipping,
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
            issue_title: None,
            priority: None,
            crew: vec![role.into()],
            subagents: Vec::new(),
            action: None,
        }
    }

    fn village(project: &str, houses: Vec<House>) -> VillageSnapshot {
        VillageSnapshot {
            project: project.into(),
            workflow: None,
            repo_path: String::new(),
            manager: house("manager", "manager", None),
            houses,
            backlog: Vec::new(),
            board: Vec::new(),
            pipeline: Pipeline::default(),
            builds: Vec::new(),
        }
    }

    fn issue(number: u32, title: &str, labels: &[&str]) -> IssueSnapshot {
        IssueSnapshot {
            number,
            title: title.into(),
            state: "open".into(),
            priority: "P1".into(),
            labels: labels.iter().map(|l| l.to_string()).collect(),
            is_blocked: false,
            is_working: false,
            assigned_worker: None,
            url: String::new(),
        }
    }

    fn config() -> WorldConfig {
        WorldConfig::parse("shop=temporal:agents", DEFAULT_INCIDENT_LABEL).unwrap()
    }

    #[test]
    fn parses_world_specs() {
        let c = config();
        assert_eq!((c.dev.as_str(), c.platform.as_str()), ("shop", "temporal:agents"));
        assert!(WorldConfig::parse("shop", "incident").is_none());
        assert!(WorldConfig::parse("=temporal:x", "incident").is_none());
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
        let w = build_world(&config(), &dev, &issues, &platform);
        let got: Vec<(u32, Option<&str>, bool)> = w.incidents.iter().map(|i| (i.number, i.origin.as_deref(), i.in_progress)).collect();
        assert_eq!(got, vec![(501, Some("procurement-118"), false), (502, Some("bugfix-52"), true), (503, None, false)]);
        assert_eq!(w.damaged, vec!["bugfix-52", "procurement-118"]);
        assert_eq!(w.name, "world:shop");
    }

    #[test]
    fn a_release_ships_until_a_platform_worker_runs_it() {
        let mut dev = village("shop", vec![]);
        dev.pipeline = Pipeline {
            release: Some("9f3c2e1d8a7b".into()),
            stops: vec![Stop { name: "master-green".into(), issues: vec![9] }, Stop { name: "prod".into(), issues: vec![7, 8] }],
            ..Pipeline::default()
        };
        let mut platform = village("temporal:agents", vec![]);
        let w = build_world(&config(), &dev, &[], &platform);
        assert_eq!(w.landed, None, "no build IDs reported: can't tell");
        assert!(w.shipping.is_empty());

        platform.builds = vec!["1a2b3c4".into()];
        let w = build_world(&config(), &dev, &[], &platform);
        assert_eq!(w.landed, Some(false));
        assert_eq!(w.shipping, vec![7, 8]);

        platform.builds.push("9f3c2e1".into());
        let w = build_world(&config(), &dev, &[], &platform);
        assert_eq!(w.landed, Some(true));
        assert!(w.shipping.is_empty());
    }
}
