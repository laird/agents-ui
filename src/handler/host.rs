//! Deciding whether a link's `host` names this machine, a known remote
//! machine, or neither (#576) -- pure matching against lists the caller has
//! already gathered (this machine's names, tailnet peers, saved herdr
//! machines), so it can be tested without a real network or herdr server.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKind {
    /// Matches one of this machine's own names.
    Local,
    /// Matches a tailnet peer or a saved herdr machine: a host `open` will
    /// act on remotely, over ssh to this target (#593).
    Known(String),
}

/// A name a remote machine is known by, and the ssh target that reaches it.
/// A tailnet name is its own target; a saved herdr machine's `label` (a
/// human name such as "Build machine") reaches its `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHost {
    pub name: String,
    pub target: String,
}

impl KnownHost {
    /// A name that is also its own ssh target.
    pub fn itself(name: &str) -> Self {
        KnownHost { name: name.to_string(), target: name.to_string() }
    }
}

/// `host`, matched case-insensitively against `local_names` first, then
/// `known_hosts`. `None` when it matches neither -- `open` must refuse to
/// run anything against a host it doesn't recognize, since a known host's
/// target ends up as an argument to `ssh`. A host or target starting with
/// `-` is never accepted: ssh would read it as an option (#593).
pub fn classify(host: &str, local_names: &[String], known_hosts: &[KnownHost]) -> Option<HostKind> {
    if host.starts_with('-') {
        return None;
    }
    if local_names.iter().any(|n| n.eq_ignore_ascii_case(host)) {
        return Some(HostKind::Local);
    }
    known_hosts
        .iter()
        .find(|k| k.name.eq_ignore_ascii_case(host) && !k.target.is_empty() && !k.target.starts_with('-'))
        .map(|k| HostKind::Known(k.target.clone()))
}

/// A tailnet peer's (or this machine's own) `DNSName` from `tailscale status
/// --json`, both as given and with its trailing dot and domain suffix
/// stripped, so `host=nuc-1` matches a peer reported as
/// `nuc-1.tail04f2b7.ts.net.`.
pub fn tailnet_names(dns_name: &str) -> Vec<String> {
    let full = dns_name.trim_end_matches('.');
    let mut names = vec![full.to_string()];
    if let Some(short) = full.split('.').next()
        && short != full
        && !short.is_empty()
    {
        names.push(short.to_string());
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_a_local_name_case_insensitively() {
        let local = vec!["nuc-1".to_string(), "nuc-1.tail04f2b7.ts.net".to_string()];
        assert_eq!(classify("NUC-1", &local, &[]), Some(HostKind::Local));
    }

    #[test]
    fn matches_a_known_remote_host() {
        let known = vec![KnownHost::itself("workbox")];
        assert_eq!(classify("WorkBox", &[], &known), Some(HostKind::Known("workbox".into())));
    }

    #[test]
    fn a_herdr_label_reaches_its_machines_target() {
        let known = vec![KnownHost { name: "Build machine".into(), target: "workbox".into() }];
        assert_eq!(classify("build machine", &[], &known), Some(HostKind::Known("workbox".into())));
    }

    #[test]
    fn local_wins_when_a_host_is_in_both_lists() {
        assert_eq!(classify("shared", &["shared".to_string()], &[KnownHost::itself("shared")]), Some(HostKind::Local));
    }

    #[test]
    fn an_unrecognized_host_is_refused() {
        assert_eq!(classify("attacker.example.com", &["nuc-1".to_string()], &[KnownHost::itself("workbox")]), None);
    }

    #[test]
    fn a_host_or_target_ssh_would_read_as_an_option_is_refused() {
        let known = vec![KnownHost::itself("-oProxyCommand=x"), KnownHost { name: "box".into(), target: "-F/tmp/c".into() }];
        assert_eq!(classify("-oProxyCommand=x", &[], &known), None);
        assert_eq!(classify("box", &[], &known), None);
        assert_eq!(classify("-x", &["-x".to_string()], &[]), None, "not even as a local name");
    }

    #[test]
    fn tailnet_names_strips_the_trailing_dot_and_adds_the_short_form() {
        assert_eq!(tailnet_names("nuc-1.tail04f2b7.ts.net."), vec!["nuc-1.tail04f2b7.ts.net", "nuc-1"]);
    }

    #[test]
    fn tailnet_names_with_no_domain_has_only_the_one_form() {
        assert_eq!(tailnet_names("nuc-1"), vec!["nuc-1"]);
    }
}
