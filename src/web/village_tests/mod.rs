//! The village page's tests (#448), out of `server.rs` so parallel village
//! work doesn't share one test block: one file per feature area. They read
//! the embedded page (`VILLAGE_ASSETS`), so they need no server; the HTTP
//! tests that serve it stay in `server.rs`.

mod core;
mod envrow;
mod fit;
mod gatehouse;
mod labels_key;
mod lifecycle;
mod screensaver;
mod themes;

pub(super) use super::server::{VILLAGE_FILES as VILLAGE_ASSETS, village_asset};

/// Everything the village page is made of, for tests that look for a
/// line wherever it lives.
pub(super) fn village_source() -> String {
    VILLAGE_ASSETS.iter().map(|(_, _, body)| *body).collect::<Vec<_>>().join("\n")
}

/// The files a village module imports, as paths under `village/`:
/// `import { P } from '../palette.js';` in `themes/modern.js` is `palette.js`.
pub(super) fn village_imports(path: &str, body: &str) -> Vec<String> {
    let dir: Vec<&str> = path.split('/').collect::<Vec<_>>()[..path.matches('/').count()].to_vec();
    body.lines()
        .filter(|l| l.starts_with("import "))
        .filter_map(|l| l.split(['\'', '"']).nth(1))
        .map(|spec| {
            let mut parts = dir.clone();
            for seg in spec.split('/') {
                match seg {
                    "." => {}
                    ".." => { parts.pop(); }
                    s => parts.push(s),
                }
            }
            parts.join("/")
        })
        .collect()
}

/// The text of `function <name>(` up to its closing brace at column 0.
pub(super) fn js_fn<'a>(body: &'a str, name: &str) -> &'a str {
    let start = body.find(&format!("function {name}(")).unwrap_or_else(|| panic!("no function {name}"));
    let rest = &body[start..];
    &rest[..rest.find("\n}\n").unwrap()]
}

/// The keys of a one-line or multi-line `const NAME = { a: ..., b: ... };` table.
pub(super) fn js_keys(body: &str, name: &str) -> Vec<String> {
    let start = body.find(&format!("const {name} = {{")).unwrap_or_else(|| panic!("no {name}")) + name.len() + 10;
    let rest = &body[start..];
    let (mut depth, mut keys, mut tok, mut quote) = (0i32, Vec::new(), String::new(), None::<(char, bool)>);
    for ch in rest.chars() {
        // Commas and colons inside quotes are text, not syntax. A quoted key
        // (`'needs-design':`) is kept; a quoted value is skipped.
        if let Some((q, keep)) = quote {
            if ch == q { quote = None; } else if keep { tok.push(ch); }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => { quote = Some((ch, depth == 0 && tok.trim().is_empty())); continue; }
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' if depth == 0 => break,
            '}' | ']' | ')' => depth -= 1,
            ':' if depth == 0 => {
                let k = tok.trim().to_string();
                if !k.is_empty() { keys.push(k); }
                tok.clear();
                continue;
            }
            ',' if depth == 0 => { tok.clear(); continue; }
            _ => {}
        }
        if depth == 0 { tok.push(ch); }
    }
    keys
}

/// The text of `const <name> = {` up to its closing `};` at the same indent.
pub(super) fn js_block<'a>(body: &'a str, name: &str) -> &'a str {
    let start = body.find(&format!("const {name} = {{")).unwrap_or_else(|| panic!("no {name}"));
    let rest = &body[start..];
    &rest[..rest.find("\n};\n").unwrap()]
}

/// Method and property names at the first level of a theme object.
pub(super) fn slots(block: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in block.lines().skip(1) {
        let Some(rest) = line.strip_prefix("  ") else { continue };
        if rest.starts_with(' ') || rest.starts_with('/') || rest.starts_with('*') {
            continue;
        }
        let ident = |s: &str| s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
        let first = ident(rest);
        // A method, `name(...) { ... }`: one slot, whatever its body holds.
        if !first.is_empty() && rest[first.len()..].starts_with('(') {
            out.push(first);
            continue;
        }
        // Otherwise the line may list several: `tree, hall, gate: lock, sign: [a, b],`.
        let (mut depth, mut part) = (0i32, String::new());
        let mut parts = Vec::new();
        for ch in rest.chars() {
            match ch {
                '[' | '(' | '{' => depth += 1,
                ']' | ')' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(std::mem::take(&mut part));
                    continue;
                }
                _ => {}
            }
            part.push(ch);
        }
        parts.push(part);
        for part in parts.iter().map(|p| p.trim()) {
            let name = ident(part);
            let after = &part[name.len()..];
            if !name.is_empty() && (after.starts_with(':') || after.is_empty()) {
                out.push(name);
            }
        }
    }
    out
}
