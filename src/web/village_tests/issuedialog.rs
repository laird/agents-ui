//! An issue's own dialog in the village (#564): the board's rows (and any
//! other list naming an issue) get `#N` as a button, and a single-subject
//! dialog about one issue (a crate's, or a house's claimed one) gets an
//! "Open issue" button; either reads the issue in full from the server
//! (issue.rs) and shows it here. The words are tested under node
//! (issuewords.test.mjs, markdown.test.mjs), the server read in `issue.rs`.

use super::*;

#[test]
fn dialogs_offer_an_issues_own_dialog() {
    assert!(village_asset("index.html").unwrap().1.contains("<div id=\"d-issue\"></div>"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './issuedialog.js';"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './markdown.js';"));
    assert!(village_asset("modules.js").unwrap().1.contains("import './issuewords.js';"));
    // A list item with a URL gets #N as a button, not a direct GitHub link;
    // the GitHub link stays, now secondary. One with none (a runtime
    // village's workstream) stays plain text: no fake affordance.
    let dialogs = village_asset("dialogs.js").unwrap().1;
    assert!(dialogs.contains("<button class=\"issue-open\" type=\"button\" data-n=\"${esc(i.number)}\">#${i.number}</button>"));
    assert!(dialogs.contains("i.url ? `<button"));
    assert!(dialogs.contains("on GitHub ↗"));
    let issuedialog = village_asset("issuedialog.js").unwrap().1;
    // Any list row's button opens the issue dialog on the server's detail.
    assert!(issuedialog.contains("document.querySelectorAll('#d-meta li[data-n] button.issue-open')"));
    assert!(issuedialog.contains("const res = await fetch(issueUrl(project, n));"));
    // A single-subject dialog's own issue, the same number travel.js already finds.
    assert!(issuedialog.contains("const n = info.openIssue || info.travel || (info.house && info.house.issue) || null;"));
    // A platform's workloads aren't issues.
    assert!(issuedialog.contains("if (!v || v.plane === 'runtime') return;"));
    // A slow answer for an earlier click is dropped, as travel.js's own fetch is.
    assert!(issuedialog.contains("if (mine !== issueToken) return;"));
}

/// Chat with the manager or any agent in a panel over the map (#566, for
/// #563): "Discuss with manager" on an issue's own dialog, prefilled, posting
/// to the manager's /input and then Enter; "Chat" on every agent's dialog; a
/// read-only server shows the output only. The words and requests are tested
/// under node (chatwords.test.mjs).
#[test]
fn discuss_with_manager_opens_a_chat_panel_with_the_prefill() {
    let chat = village_asset("chat.js").unwrap().1;
    assert!(chat.contains("buttons.push(['Discuss with manager', () => openChat(project, v.manager.role, `Chat · manager · about #${info.issueDetail.number}`, discussPrefill(info.issueDetail))]);"));
    assert!(chat.contains("input.value = text;"), "the line starts with the prefill");
    assert!(chat.contains("await post(sendSteps(who.project, who.role, text));"), "it types into that agent's session");
    let words = village_asset("chatwords.js").unwrap().1;
    assert!(words.contains("{ url: inputUrl(project, role), body: { text, enter: false } },"));
    assert!(words.contains("{ url: keysUrl(project, role), body: { items: [{ type: 'key', key: 'Enter' }] } },"));
    // Live output, falling back to polling; never answering a trust prompt.
    assert!(chat.contains("chatStream = new EventSource(streamUrl(who.project, who.role));"));
    assert!(chat.contains("chatPoll = setInterval(() => pollPane(who), 2000);"));
    assert!(chat.contains("if (chatTrust) { msg.textContent = 'Not sent: answer the trust prompt in the agent\\'s own session first.'; return; }"));
    // Read-only: output only, no input.
    assert!(chat.contains("form.hidden = !ok;"));
    assert!(chat.contains("fetch('api/config')"));
    // Every agent's dialog offers "Chat"; the panel is in the page, not a new tab.
    let lifecycle = village_asset("lifecycle.js").unwrap().1;
    assert!(lifecycle.contains("chat: s ? { role: m.role, name: 'manager', label: 'Chat with manager' } : null };"));
    assert!(lifecycle.contains("chat: sessionOf(h) ? { role: h.role, name: houseName(v, h) } : null,"));
    assert!(lifecycle.contains("chat: sessionOf(h) ? { role: h.role, name: o.who || h.role } : null };"));
    assert!(village_asset("index.html").unwrap().1.contains("<div id=\"d-chat\"></div>"));
    assert!(village_asset("village.css").unwrap().1.contains(".chat pre {"));
}
