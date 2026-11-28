//! Parsing and validating an `agents-ui://attach?...` link (#576), e.g.
//! `agents-ui://attach?host=nuc-1&mux=herdr&pane=w1F:p1&project=agents-ui&role=wt-1`
//! (laird/agents-ui#549). Nothing here runs a command: `parse` only shapes
//! and rejects malformed input. `host` still needs checking against known
//! machines once the environment is available (see `handler::host::classify`).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mux {
    Herdr,
    Tmux,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachUrl {
    pub host: String,
    pub mux: Mux,
    /// The agent's pane, in its mux's own shape: `w<id>:p<n>` for herdr, or
    /// a tmux target (`session:window.pane`, or a raw pane id `%n`).
    pub pane: String,
    pub project: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    NotAnAttachUrl,
    MissingField(&'static str),
    BadMux(String),
    BadPane(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NotAnAttachUrl => write!(f, "not an agents-ui://attach link"),
            ParseError::MissingField(name) => write!(f, "missing '{name}'"),
            ParseError::BadMux(got) => write!(f, "mux must be 'herdr' or 'tmux', got '{got}'"),
            ParseError::BadPane(got) => write!(f, "'{got}' is not a valid pane for that mux"),
        }
    }
}

/// Parse `url` into its fields. Rejects anything that isn't
/// `agents-ui://attach?...`, a missing or empty `host`/`mux`/`pane`, an
/// unrecognized `mux`, or a `pane` that doesn't match its mux's shape.
pub fn parse(url: &str) -> Result<AttachUrl, ParseError> {
    let rest = url.strip_prefix("agents-ui://attach").ok_or(ParseError::NotAnAttachUrl)?;
    let query = rest.strip_prefix('?').unwrap_or(rest);

    let mut host = None;
    let mut mux_raw = None;
    let mut pane = None;
    let mut project = None;
    let mut role = None;
    for pair in query.split('&').filter(|s| !s.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = percent_decode(value);
        match key {
            "host" => host = Some(value),
            "mux" => mux_raw = Some(value),
            "pane" => pane = Some(value),
            "project" => project = Some(value),
            "role" => role = Some(value),
            // Forward-compatible: a field this binary doesn't know yet is ignored.
            _ => {}
        }
    }

    let host = non_empty(host).ok_or(ParseError::MissingField("host"))?;
    let mux_raw = non_empty(mux_raw).ok_or(ParseError::MissingField("mux"))?;
    let mux = match mux_raw.as_str() {
        "herdr" => Mux::Herdr,
        "tmux" => Mux::Tmux,
        other => return Err(ParseError::BadMux(other.to_string())),
    };
    let pane = non_empty(pane).ok_or(ParseError::MissingField("pane"))?;
    if !valid_pane(&pane, mux) {
        return Err(ParseError::BadPane(pane));
    }

    Ok(AttachUrl { host, mux, pane, project: non_empty(project), role: non_empty(role) })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// Herdr's pane ids are `w<id>:p<n>` (docs/autocoder-ctl.md "Status for
/// UIs"); tmux targets are `session:window.pane` or a raw pane id `%n`.
/// Restricted to the characters either mux ever uses, so nothing here can
/// be a shell metacharacter -- though every command built from this value
/// also passes it as its own argv entry, never through a shell string.
fn valid_pane(pane: &str, mux: Mux) -> bool {
    match mux {
        Mux::Herdr => {
            let Some((w, p)) = pane.split_once(':') else { return false };
            w.len() > 1
                && w.starts_with('w')
                && w[1..].chars().all(|c| c.is_ascii_alphanumeric())
                && p.len() > 1
                && p.starts_with('p')
                && p[1..].chars().all(|c| c.is_ascii_digit())
        }
        Mux::Tmux => {
            !pane.is_empty()
                && pane.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '%'))
        }
    }
}

/// Decode `%XX` escapes a browser-built query string may use (a pane target
/// contains `:`, which some URL builders escape as `%3A`). Any other `%`
/// (not followed by two hex digits) passes through unchanged.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_herdr_link() {
        let got = parse("agents-ui://attach?host=nuc-1&mux=herdr&pane=w1F:p1&project=agents-ui&role=wt-1").unwrap();
        assert_eq!(
            got,
            AttachUrl {
                host: "nuc-1".to_string(),
                mux: Mux::Herdr,
                pane: "w1F:p1".to_string(),
                project: Some("agents-ui".to_string()),
                role: Some("wt-1".to_string()),
            }
        );
    }

    #[test]
    fn project_and_role_are_optional() {
        let got = parse("agents-ui://attach?host=nuc-1&mux=herdr&pane=w1:p1").unwrap();
        assert_eq!(got.project, None);
        assert_eq!(got.role, None);
    }

    #[test]
    fn decodes_a_percent_encoded_colon_in_the_pane() {
        let got = parse("agents-ui://attach?host=nuc-1&mux=herdr&pane=w1%3Ap1").unwrap();
        assert_eq!(got.pane, "w1:p1");
    }

    #[test]
    fn rejects_a_different_scheme_or_action() {
        assert_eq!(parse("https://attach?host=x&mux=herdr&pane=w1:p1"), Err(ParseError::NotAnAttachUrl));
        assert_eq!(parse("agents-ui://open?host=x"), Err(ParseError::NotAnAttachUrl));
    }

    #[test]
    fn rejects_a_missing_field() {
        assert_eq!(parse("agents-ui://attach?mux=herdr&pane=w1:p1"), Err(ParseError::MissingField("host")));
        assert_eq!(parse("agents-ui://attach?host=x&pane=w1:p1"), Err(ParseError::MissingField("mux")));
        assert_eq!(parse("agents-ui://attach?host=x&mux=herdr"), Err(ParseError::MissingField("pane")));
        assert_eq!(parse("agents-ui://attach?host=&mux=herdr&pane=w1:p1"), Err(ParseError::MissingField("host")), "empty counts as missing");
    }

    #[test]
    fn rejects_an_unknown_mux() {
        assert_eq!(
            parse("agents-ui://attach?host=x&mux=cmux&pane=w1:p1"),
            Err(ParseError::BadMux("cmux".to_string()))
        );
    }

    #[test]
    fn rejects_a_herdr_pane_that_doesnt_match_the_shape() {
        for bad in ["p1", "w1", "w1:1", "1:p1", "w1:p", "; rm -rf /", "w1:p1;x"] {
            assert_eq!(parse(&format!("agents-ui://attach?host=x&mux=herdr&pane={bad}")), Err(ParseError::BadPane(bad.to_string())), "{bad}");
        }
    }

    #[test]
    fn accepts_tmux_target_shapes() {
        assert_eq!(parse("agents-ui://attach?host=x&mux=tmux&pane=claude-shop:1.0").unwrap().pane, "claude-shop:1.0");
        // A literal `%` must itself be percent-encoded (`%25`) to survive a
        // URL query string, so a raw tmux pane id `%12` arrives as `%2512`.
        assert_eq!(parse("agents-ui://attach?host=x&mux=tmux&pane=%2512").unwrap().pane, "%12");
    }

    #[test]
    fn rejects_a_tmux_pane_with_shell_metacharacters() {
        for bad in ["%12; rm -rf /", "claude-shop:1.0 && echo hi", "$(whoami)", ""] {
            assert!(parse(&format!("agents-ui://attach?host=x&mux=tmux&pane={bad}")).is_err(), "{bad}");
        }
    }
}
