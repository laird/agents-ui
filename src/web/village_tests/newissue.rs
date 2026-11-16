//! Filing a new issue from the village (#490): the top-bar button, the
//! board's, and the form.

use super::*;

#[test]
fn new_issue_is_offered_beside_the_repo_link_and_on_the_board() {
    let html = village_asset("index.html").unwrap().1;
    assert!(html.contains("<button id=\"new-issue\" class=\"new-issue-btn\" hidden"));
    assert!(html.contains("<div id=\"d-form\"></div>"));
    let body = &village_source();
    // Shown only while a development village is (a runtime has no backlog to file into).
    assert!(body.contains("registerOverlay(() => { topButton.hidden = !filingVillage(); });"));
    assert!(body.contains("return sc && sc.v && sc.v.plane !== 'runtime' ? sc.v : null;"));
    // The board's dialog offers it under its list.
    assert!(body.contains("const v = info.decide && state.villages.find(x => x.project === info.decide);"));
}

#[test]
fn the_form_files_through_the_server_and_a_403_links_to_github() {
    let body = &village_source();
    assert!(body.contains("<select name=\"type\">${options(ISSUE_TYPES, 'enhancement')}</select>"));
    assert!(body.contains("<select name=\"priority\">${options(ISSUE_PRIORITIES, 'P3')}</select>"));
    assert!(body.contains("export const newIssueUrl = project => `/api/swarms/${encodeURIComponent(project)}/issues`;"));
    assert!(body.contains("if (status === 403) return { words: 'This dashboard is read-only: file it on GitHub.', url: repo ? `${repo}/issues/new` : null };"));
    // Fields by name through `elements`, never `form.title` (the form's own attribute).
    assert!(body.contains("const field = n => form.elements.namedItem(n);"));
}
