//! Which version of a library a consumer pins, and the library's latest
//! release (#511, phase 2 of #494): the valley's ships.
//!
//! A small reader per ecosystem, each trying the lockfile first (the exact
//! version built) and then the manifest (the range asked for):
//!
//! - Cargo: `Cargo.lock`, then `Cargo.toml`'s dependency tables.
//! - npm: `package-lock.json` (v2/v3 `packages`, v1 `dependencies`), then
//!   `package.json`'s dependency maps.
//! - Go: `go.mod`'s `require`.
//!
//! An ecosystem with none of these files is skipped, not an error.
//!
//! A library's name is what it publishes (`Cargo.toml`'s `[package] name`,
//! `package.json`'s `name`, `go.mod`'s module path), else its repo's name.
//! Its latest release is its `release` stop's tag when its village has one,
//! else the newest version tag in its repo.

use std::cmp::Ordering;
use std::path::Path;

/// A version's numbers, from `v1.4.0`, `^1.4`, `>=1.2.3`, `1.4.0-rc.1`:
/// the leading run of dotted numbers, ignoring a leading `v` or range
/// operator. None when there are none.
pub fn version_numbers(v: &str) -> Option<Vec<u64>> {
    let rest = v.trim().trim_start_matches(|c: char| !c.is_ascii_digit());
    let lead: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let nums: Vec<u64> = lead.split('.').filter(|p| !p.is_empty()).map_while(|p| p.parse().ok()).collect();
    (!nums.is_empty()).then_some(nums)
}

/// Whether a version is a pre-release: `1.4.0-rc.1`, `v2.0.0-beta`.
pub fn is_prerelease(v: &str) -> bool {
    let rest = v.trim().trim_start_matches(|c: char| !c.is_ascii_digit());
    rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.').starts_with('-')
}

/// Compare two versions by their numbers, a missing part counting as 0
/// (`1.4` is `1.4.0`), a pre-release below its release (`1.4.0-rc.1` <
/// `1.4.0`). None when either has no numbers.
pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    let (na, nb) = (version_numbers(a)?, version_numbers(b)?);
    let n = na.len().max(nb.len());
    let at = |v: &[u64], k: usize| v.get(k).copied().unwrap_or(0);
    let by_numbers = (0..n).map(|k| at(&na, k).cmp(&at(&nb, k))).find(|o| *o != Ordering::Equal).unwrap_or(Ordering::Equal);
    Some(by_numbers.then_with(|| is_prerelease(b).cmp(&is_prerelease(a))))
}

/// What a pin is: the exact version built (a lockfile, `go.mod`), or a
/// range asked for in a manifest, read the way its ecosystem reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    Exact,
    /// `Cargo.toml`: a bare version is a caret range.
    Cargo,
    /// `package.json`: a bare version is exact.
    Npm,
}

/// Whether a consumer pinning `pinned` (read as `pin`) is behind `latest`:
/// an exact pin older than it, or a range that doesn't take it. None when
/// either can't be read.
pub fn behind(pinned: &str, pin: Pin, latest: &str) -> Option<bool> {
    let p = pinned.trim();
    let older = || compare(p, latest).map(|o| o == Ordering::Less);
    if pin == Pin::Exact {
        return older();
    }
    // An upper bound (`<2`, `<=1.9`, Cargo's `>=1.2, <2`): behind once the
    // release reaches it.
    if let Some(at) = p.find('<') {
        let (bound, inclusive) = match p[at + 1..].strip_prefix('=') {
            Some(rest) => (rest, true),
            None => (&p[at + 1..], false),
        };
        let past = compare(latest, bound)?;
        return Some(if inclusive { past == Ordering::Greater } else { past != Ordering::Less });
    }
    // `>=1.2`, `*`, `x`, `latest`: anything newer is taken.
    if p.starts_with(">=") || p.starts_with('>') || p == "*" || p.eq_ignore_ascii_case("x") || p == "latest" || p.contains("||") {
        return version_numbers(latest).map(|_| false);
    }
    let want = version_numbers(p)?;
    let have = version_numbers(latest)?;
    if is_prerelease(latest) {
        return older();
    }
    // Wildcards (`1.x`, `1.4.*`) fix only the parts before them.
    let fixed = p.trim_start_matches(|c: char| !c.is_ascii_digit()).split('.').take_while(|s| s.parse::<u64>().is_ok()).count();
    let wild = p.contains('x') || p.contains('*') || p.contains('X') || (pin == Pin::Npm && p.chars().next().is_some_and(|c| c.is_ascii_digit()) && want.len() < 3);
    // npm: a full bare version is exact; a partial one (`1`, `1.2`) is an x-range.
    let npm_exact = pin == Pin::Npm && p.chars().next().is_some_and(|c| c.is_ascii_digit()) && !wild && want.len() >= 3;
    let exact = p.starts_with('=') || npm_exact;
    if exact {
        return older();
    }
    let at = |v: &[u64], k: usize| v.get(k).copied().unwrap_or(0);
    // How many leading parts the range holds fixed.
    let hold = if wild {
        fixed
    } else if p.starts_with('~') {
        // `~1.4` / `~1.4.2`: the minor; `~1`: the major.
        if want.len() >= 2 { 2 } else { 1 }
    } else {
        // Caret (npm `^`, Cargo's default): up to the first non-zero part.
        want.iter().position(|n| *n != 0).map_or(want.len(), |k| k + 1).min(want.len().max(1))
    };
    if (0..hold).any(|k| at(&want, k) != at(&have, k)) {
        // Outside the range: behind if the release is newer.
        return older();
    }
    Some(false)
}

fn read(dir: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(file)).ok()
}

/// The names a library goes by: what it publishes, and its repo's name.
pub fn package_names(dir: &Path, repo_name: &str) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(name) = read(dir, "Cargo.toml")
        .and_then(|t| t.parse::<toml::Table>().ok())
        .and_then(|t| t.get("package")?.get("name")?.as_str().map(str::to_string))
    {
        names.push(name);
    }
    if let Some(name) = read(dir, "package.json")
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("name")?.as_str().map(str::to_string))
    {
        names.push(name);
    }
    if let Some(module) = read(dir, "go.mod").and_then(|t| t.lines().find_map(|l| l.strip_prefix("module ").map(|m| m.trim().to_string()))) {
        names.push(module);
    }
    if !names.iter().any(|n| n == repo_name) {
        names.push(repo_name.to_string());
    }
    names
}

/// The version `Cargo.lock` locks `name` at.
pub fn cargo_lock(text: &str, name: &str) -> Option<String> {
    let lock = text.parse::<toml::Table>().ok()?;
    lock.get("package")?.as_array()?.iter().find(|p| p.get("name").and_then(|n| n.as_str()) == Some(name))?.get("version")?.as_str().map(str::to_string)
}

/// The version `Cargo.toml` asks for: `name = "1.4"` or `name = { version = "1.4" }`,
/// in any dependency table (including a target's).
pub fn cargo_toml(text: &str, name: &str) -> Option<String> {
    let manifest = text.parse::<toml::Table>().ok()?;
    let from = |deps: &toml::Value| -> Option<String> {
        match deps.get(name)? {
            toml::Value::String(v) => Some(v.clone()),
            t => t.get("version")?.as_str().map(str::to_string),
        }
    };
    let tables = ["dependencies", "dev-dependencies", "build-dependencies"];
    tables.iter().find_map(|t| manifest.get(*t).and_then(from)).or_else(|| {
        manifest.get("target")?.as_table()?.values().find_map(|target| tables.iter().find_map(|t| target.get(*t).and_then(from)))
    })
}

/// The version `package-lock.json` installs `name` at.
pub fn npm_lock(text: &str, name: &str) -> Option<String> {
    let lock: serde_json::Value = serde_json::from_str(text).ok()?;
    let v2 = lock.get("packages").and_then(|p| p.get(format!("node_modules/{name}")));
    let v1 = lock.get("dependencies").and_then(|d| d.get(name));
    v2.or(v1)?.get("version")?.as_str().map(str::to_string)
}

/// The range `package.json` asks for.
pub fn npm_manifest(text: &str, name: &str) -> Option<String> {
    let manifest: serde_json::Value = serde_json::from_str(text).ok()?;
    ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"]
        .iter()
        .find_map(|k| manifest.get(*k)?.get(name)?.as_str().map(str::to_string))
}

/// The version `go.mod` requires: `require name v1.4.0`, alone or in a block.
pub fn go_mod(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let l = l.trim().strip_prefix("require ").unwrap_or(l.trim());
        let mut words = l.split_whitespace();
        (words.next() == Some(name)).then(|| words.next().map(str::to_string)).flatten()
    })
}

/// The version of a library (known by any of `names`) the repo at `dir`
/// pins, and how to read it: its lockfiles first, then its manifests. None
/// when no reader finds it.
pub fn pinned(dir: &Path, names: &[String]) -> Option<(String, Pin)> {
    type Reader = fn(&str, &str) -> Option<String>;
    let readers: [(&str, Reader, Pin); 5] = [
        ("Cargo.lock", cargo_lock, Pin::Exact),
        ("package-lock.json", npm_lock, Pin::Exact),
        ("go.mod", go_mod, Pin::Exact),
        ("Cargo.toml", cargo_toml, Pin::Cargo),
        ("package.json", npm_manifest, Pin::Npm),
    ];
    readers.iter().find_map(|(file, reader, pin)| {
        let text = read(dir, file)?;
        names.iter().find_map(|n| reader(&text, n)).map(|v| (v, *pin))
    })
}

/// The newest version tag in the repo at `dir`, with when it was made: the
/// latest release of a library with no `release` stop. Tags with no version
/// numbers are not releases.
pub fn latest_tag(dir: &Path) -> Option<(String, Option<String>)> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["for-each-ref", "refs/tags", "--format=%(refname:short) %(creatordate:iso-strict)"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut parts = l.splitn(2, ' ');
            let tag = parts.next()?.to_string();
            // A pre-release isn't the latest release.
            if version_numbers(&tag).is_none() || is_prerelease(&tag) {
                return None;
            }
            Some((tag, parts.next().map(str::to_string).filter(|d| !d.is_empty())))
        })
        .max_by(|a, b| compare(&a.0, &b.0).unwrap_or(Ordering::Equal))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("agents-ui-pins-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn versions_compare_by_their_numbers() {
        assert_eq!(version_numbers("v1.4.0"), Some(vec![1, 4, 0]));
        assert_eq!(version_numbers("^1.4"), Some(vec![1, 4]));
        assert_eq!(version_numbers(">=1.2.3, <2"), Some(vec![1, 2, 3]));
        assert_eq!(version_numbers("1.4.0-rc.1"), Some(vec![1, 4, 0]));
        assert_eq!(version_numbers("v2026.11.16.3"), Some(vec![2026, 11, 16, 3]));
        assert_eq!(version_numbers("main"), None);
        assert_eq!(compare("1.4", "v1.4.0"), Some(Ordering::Equal));
        assert_eq!(compare("1.3.9", "1.4.0"), Some(Ordering::Less));
        assert_eq!(compare("1.10.0", "1.9.0"), Some(Ordering::Greater), "numbers, not text");
        assert_eq!(compare("latest", "1.0"), None);
        assert_eq!(compare("1.4.0-rc.1", "1.4.0"), Some(Ordering::Less), "a pre-release comes before its release");
        assert_eq!(compare("v2.0.0", "v2.0.0-rc.1"), Some(Ordering::Greater));
        assert!(is_prerelease("v2.0.0-rc.1") && !is_prerelease("v2.0.0") && !is_prerelease("^1.4"));
    }

    #[test]
    fn a_range_is_behind_only_when_it_does_not_take_the_release() {
        let b = |p: &str, pin: Pin, l: &str| behind(p, pin, l);
        // Exact pins: older is behind.
        assert_eq!(b("1.3.0", Pin::Exact, "v1.4.0"), Some(true));
        assert_eq!(b("1.4.0", Pin::Exact, "v1.4.0"), Some(false));
        assert_eq!(b("1.4.0-rc.1", Pin::Exact, "v1.4.0"), Some(true));
        // Cargo: a bare version is a caret range.
        assert_eq!(b("1", Pin::Cargo, "v1.4.0"), Some(false));
        assert_eq!(b("1.2", Pin::Cargo, "v1.4.0"), Some(false));
        assert_eq!(b("1.4", Pin::Cargo, "v2.0.0"), Some(true));
        assert_eq!(b("0.3", Pin::Cargo, "v0.4.0"), Some(true), "0.x: the minor is the major");
        assert_eq!(b("0.3.1", Pin::Cargo, "v0.3.9"), Some(false));
        assert_eq!(b("=1.3.0", Pin::Cargo, "v1.4.0"), Some(true));
        assert_eq!(b("~1.3", Pin::Cargo, "v1.4.0"), Some(true));
        assert_eq!(b("~1.4", Pin::Cargo, "v1.4.7"), Some(false));
        assert_eq!(b(">=1.2", Pin::Cargo, "v9.0.0"), Some(false));
        assert_eq!(b(">=1.2, <2", Pin::Cargo, "v2.0.0"), Some(true), "an upper bound the release reached");
        assert_eq!(b(">=1.2, <2", Pin::Cargo, "v1.9.0"), Some(false));
        assert_eq!(b("<=1.9", Pin::Npm, "v1.9.0"), Some(false));
        assert_eq!(b("<2.0", Pin::Npm, "v3.0.0"), Some(true));
        // npm: a bare version is exact; ^ ~ x * as npm reads them.
        assert_eq!(b("^1.2.0", Pin::Npm, "v1.4.0"), Some(false));
        assert_eq!(b("^1.9.0", Pin::Npm, "v2.0.0"), Some(true));
        assert_eq!(b("~1.3.0", Pin::Npm, "v1.4.0"), Some(true));
        assert_eq!(b("1.x", Pin::Npm, "v1.4.0"), Some(false));
        assert_eq!(b("1.3.x", Pin::Npm, "v1.4.0"), Some(true));
        assert_eq!(b("*", Pin::Npm, "v3.0.0"), Some(false));
        assert_eq!(b("1.3.0", Pin::Npm, "v1.4.0"), Some(true), "exact in package.json");
        assert_eq!(b("1", Pin::Npm, "v1.4.0"), Some(false), "a partial bare version is an x-range to npm");
        assert_eq!(b("1.3", Pin::Npm, "v1.4.0"), Some(true));
        assert_eq!(b("1.4", Pin::Npm, "v1.4.2"), Some(false));
        assert_eq!(b("1.4.0", Pin::Npm, "v1.4.0"), Some(false));
        // A range never runs ahead of the release it is compared with.
        assert_eq!(b("^1.4.0", Pin::Npm, "v1.3.0"), Some(false));
        assert_eq!(b("main", Pin::Npm, "v1.0.0"), None);
    }

    #[test]
    fn cargo_lock_then_cargo_toml() {
        let lock = "version = 3\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.1\"\n\n[[package]]\nname = \"lib-core\"\nversion = \"1.3.2\"\n";
        assert_eq!(cargo_lock(lock, "lib-core").as_deref(), Some("1.3.2"));
        assert_eq!(cargo_lock(lock, "absent"), None);
        let toml = "[package]\nname = \"shop\"\n\n[dependencies]\nserde = \"1\"\nlib-core = { version = \"1.4\", features = [\"x\"] }\n\n[target.'cfg(unix)'.dev-dependencies]\nunixy = \"0.2\"\n";
        assert_eq!(cargo_toml(toml, "lib-core").as_deref(), Some("1.4"));
        assert_eq!(cargo_toml(toml, "serde").as_deref(), Some("1"));
        assert_eq!(cargo_toml(toml, "unixy").as_deref(), Some("0.2"), "a target's tables too");
        assert_eq!(cargo_toml("[dependencies]\nlib-core = { path = \"../lib-core\" }\n", "lib-core"), None, "a path dependency has no version");
        assert_eq!(cargo_toml("not toml [", "x"), None);
    }

    #[test]
    fn npm_lock_then_package_json() {
        let v3 = r#"{"lockfileVersion":3,"packages":{"":{"name":"web"},"node_modules/lib-core":{"version":"1.3.0"}}}"#;
        let v1 = r#"{"lockfileVersion":1,"dependencies":{"lib-core":{"version":"1.2.0"}}}"#;
        assert_eq!(npm_lock(v3, "lib-core").as_deref(), Some("1.3.0"));
        assert_eq!(npm_lock(v1, "lib-core").as_deref(), Some("1.2.0"));
        assert_eq!(npm_lock(v3, "other"), None);
        let pkg = r#"{"name":"web","dependencies":{"react":"^18"},"devDependencies":{"lib-core":"~1.3.0"}}"#;
        assert_eq!(npm_manifest(pkg, "lib-core").as_deref(), Some("~1.3.0"));
        assert_eq!(npm_manifest("{", "lib-core"), None);
    }

    #[test]
    fn go_mod_requires() {
        let m = "module example.com/web\n\ngo 1.22\n\nrequire example.com/lib-core v1.4.0\n\nrequire (\n\tgolang.org/x/text v0.14.0 // indirect\n)\n";
        assert_eq!(go_mod(m, "example.com/lib-core").as_deref(), Some("v1.4.0"));
        assert_eq!(go_mod(m, "golang.org/x/text").as_deref(), Some("v0.14.0"));
        assert_eq!(go_mod(m, "nope"), None);
    }

    #[test]
    fn a_repo_pins_from_its_lockfile_first_and_unknown_ecosystems_are_skipped() {
        let d = temp("pinned");
        let names = vec!["lib-core".to_string()];
        assert_eq!(pinned(&d, &names), None, "no files: skipped");
        std::fs::write(d.join("Cargo.toml"), "[dependencies]\nlib-core = \"1.4\"\n").unwrap();
        assert_eq!(pinned(&d, &names), Some(("1.4".into(), Pin::Cargo)));
        std::fs::write(d.join("Cargo.lock"), "[[package]]\nname = \"lib-core\"\nversion = \"1.3.2\"\n").unwrap();
        assert_eq!(pinned(&d, &names), Some(("1.3.2".into(), Pin::Exact)), "what was built, not what was asked");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_library_is_known_by_what_it_publishes_and_its_repo_name() {
        let d = temp("names");
        assert_eq!(package_names(&d, "lib-core"), vec!["lib-core"]);
        std::fs::write(d.join("Cargo.toml"), "[package]\nname = \"core-lib\"\nversion = \"1.4.0\"\n").unwrap();
        std::fs::write(d.join("package.json"), r#"{"name":"@shop/core"}"#).unwrap();
        assert_eq!(package_names(&d, "lib-core"), vec!["core-lib", "@shop/core", "lib-core"]);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn the_newest_version_tag_is_the_latest_release() {
        let d = temp("tags");
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git").arg("-C").arg(&d).args(args).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap().status.success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        assert_eq!(latest_tag(&d), None, "no tags");
        git(&["commit", "-q", "--allow-empty", "-m", "a"]);
        for t in ["v1.9.0", "v1.10.0", "v1.11.0-rc.1", "nightly", "v1.2.0"] {
            git(&["tag", t]);
        }
        let (tag, at) = latest_tag(&d).unwrap();
        assert_eq!(tag, "v1.10.0", "the newest release, not a pre-release");
        assert!(at.is_some_and(|a| a.len() >= 19));
        assert_eq!(latest_tag(&d.join("missing")), None);
        std::fs::remove_dir_all(&d).ok();
    }
}
