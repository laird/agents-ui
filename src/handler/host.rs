//! Deciding whether a link's `host` names this machine, a known remote
//! machine, or neither (#576) -- pure matching against lists the caller has
//! already gathered (this machine's names, tailnet peers, saved herdr
//! machines), so it can be tested without a real network or herdr server.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    /// Matches one of this machine's own names.
    Local,
    /// Matches a tailnet peer or a saved herdr machine: a host `open` will
    /// act on remotely.
    Known,
}

/// `host`, matched case-insensitively against `local_names` first, then
/// `known_hosts`. `None` when it matches neither -- `open` must refuse to
/// run anything against a host it doesn't recognize, since every name here
/// ends up as an argument to `ssh`.
pub fn classify(host: &str, local_names: &[String], known_hosts: &[String]) -> Option<HostKind> {
    if local_names.iter().any(|n| n.eq_ignore_ascii_case(host)) {
        return Some(HostKind::Local);
    }
    if known_hosts.iter().any(|n| n.eq_ignore_ascii_case(host)) {
        return Some(HostKind::Known);
    }
    None
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
        let known = vec!["workbox".to_string()];
        assert_eq!(classify("workbox", &[], &known), Some(HostKind::Known));
    }

    #[test]
    fn local_wins_when_a_host_is_in_both_lists() {
        let v = vec!["shared".to_string()];
        assert_eq!(classify("shared", &v, &v), Some(HostKind::Local));
    }

    #[test]
    fn an_unrecognized_host_is_refused() {
        assert_eq!(classify("attacker.example.com", &["nuc-1".to_string()], &["workbox".to_string()]), None);
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
