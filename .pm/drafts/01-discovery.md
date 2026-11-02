# Discovery brief: agents-ui · Attention and answering

| Field | Value |
|---|---|
| Status | draft |
| Product · focus area | agents-ui · attention-and-answering |
| User you design for · your brief | Solo developer · Never miss a blocked agent |
| Code snapshot | laird/agents-ui @ 0019fe8 (primary; read in place, working tree clean) |
| Research window | 2026-11-02T00:29Z to (open) · fetches 15/20 · searches 0/8 |
| Chosen opportunity | (pending priority check-in) |
| Evidence review | (pending) |
| Run summary | (written at close) |

## 1. The decision

(Written last, after the priority check-in.)

## 2. What we learned

People who run several coding agents at once (in tmux panes, herdr, or editor tabs) mostly
learn that an agent is waiting on them by going to look. An agent that stops for a permission
prompt or a question (a "blocked" agent: it can do nothing until a person answers) sits silent
until someone checks, and the people who notice this build their own fixes: notification
hooks (small scripts Claude Code runs on events), phone apps, Telegram bots, kanban boards. The
platform vendors are closing the single-session case (Claude Code shipped tmux-aware terminal
notifications and phone pushes for its Remote Control mode in August 2026), but nothing in the
public record covers a swarm: several agents across several hosts, where the question is which
one needs you and whether you can answer it from where you are. When notifications do arrive
they turn into noise fast for people with five or more sessions, and detecting "waiting" from
screen text is itself unreliable.

Themes:
- Theme A, nobody tells you an agent is waiting: one sentence: agents block silently and people
  check back by hand or build their own alerts · who says so: 6 GitHub issues on Claude Code's
  tracker (largest 59 and 56 thumbs-up), 2 Hacker News comments, 1 Lemmy post · fixed or open:
  fixed for a single Claude Code session (tmux passthrough, Remote Control pushes, both August
  2026), open for anything spanning several agents or tools · why it matters: a solo developer
  running a swarm loses agent-hours every time a prompt waits unseen.
- Theme B, tell me which one and let me answer from here: once alerted, people want the alert to
  name the agent, take them straight to it, and let them reply without going back to the desk ·
  who says so: 3 GitHub issues (Claude Code), 2 Hacker News comments, 3 agents-ui issues filed by
  its maintainer · fixed or open: agents-ui already names the blocked role and deep-links from
  its Omarchy alert (#344, #340, both closed); answering herdr and Temporal agents from agents-ui
  is not possible today (CON-002, CON-005); counter-evidence: alerts become noise beyond five
  sessions (SIG-009) · why it matters: the brief is "never miss", and an alert that is ignored or
  leads two screens away is a miss.
- Theme C, is it waiting or just thinking: detecting a waiting agent from screen text or silence
  is guesswork, and hook-based signals miss some prompts · who says so: 1 Hacker News comment,
  2 GitHub issues, herdr's own README · fixed or open: sources disagree; herdr reports good
  results from pattern matching but says "there will be rough edges" · why it matters: a missed
  detection is a missed agent, and a false one trains the developer to ignore alerts.

Who we heard from: GitHub issue trackers sample power users who file bugs and feature requests,
here almost entirely Claude Code users, with Anthropic staff replying; Hacker News and Lemmy
sample tool builders and comparison shoppers, several of them makers of competing tools. Nobody
here is an agents-ui user other than its maintainer: the product has no public community yet. Missing
entirely: herdr's own users (its issue tracker could not be reached, GAP), Temporal or AgentEx
platform users, and anyone who supervises agents without being a heavy terminal user.

## 3. The three opportunities in detail

(Drafted after the research check-in.)

## 4. Evidence

### 4a. Coverage
| Kind | Target | Logged | Channels used | Who those channels mostly hear from |
|---|---|---|---|---|
| Customer or community signals | 3 | 13 | GitHub issues (rung 1), Hacker News, Lemmy (rung 2) | power users who file bugs; tool builders and comparison shoppers |
| Market or competitor observations | 2 | 2 (SIG-011 herdr, SIG-015 claude-squad), no Wayback or adoption number | competitor README and issue list (rung 1, read as community) | what makers choose to publish |
| Official product or release signal | 1 | 2 (vendor replies in SIG-002, SIG-003) | Anthropic staff replies on GitHub | the maker's own framing |
| Repository constraints | 2 | 5 | agents-ui source at 0019fe8 | the code |

Queries used:
- GitHub issue search: `notification` (Claude Code), `in:title notification` (Claude Code), no term (claude-squad, herdr), all issues (agents-ui)
- Hacker News comments: `agents waiting for input tmux`; `claude code parallel agents permission prompt`
- Lemmy posts: `coding agents notification`

Sources reached: GitHub issues for Claude Code, claude-squad, agents-ui; herdr README; Hacker
News comments; Lemmy. Empty or unreachable: herdr issues (GAP), GitHub API for any repository but
agents-ui (refused from this machine; HTML pages used instead), laird/agents issues (not read; budget).
Dropped by the PM: app reviews, numbers and competitors, official and search (rungs 3 to 7).
Added by the PM: herdr, claude-squad, Claude Code, laird/agents issue trackers as community sources.

Triangulation (at most three decision-relevant themes):
| Theme | Forum, GitHub | Lemmy, Mastodon, HN | Reviews | Numbers, competitors, official | Recurrence | Same crowd? | Counter-evidence |
|---|---|---|---|---|---|---|---|
| A. Nobody tells you an agent is waiting | SIG-002 (59 👍), SIG-001, SIG-003, SIG-004, SIG-014 | SIG-005, SIG-012 (HN), SIG-008 (Lemmy) | none | SIG-002 vendor reply: shipped for Remote Control | recurring | no: GitHub filers and HN tool builders differ | SIG-002 (shipped Aug 2026 for one session), looked (F-011) |
| B. Tell me which one and let me answer from here | SIG-002 (comment), SIG-001 (comment), SIG-010 (own) | SIG-005, SIG-006 (HN) | none | none | recurring | partly | SIG-009 (alerts become noise), looked (F-012) |
| C. Is it waiting or thinking | SIG-013, SIG-003 | SIG-007 (HN) | none | SIG-011 (herdr README) | recurring, sources disagree | no | SIG-011 and SIG-006 (herdr detection praised), looked (F-013) |

Gaps:
- GAP: herdr issues, maxjendrall/herdr has issues disabled and appears to be a fork of ogulcancelik/herdr (F-004, F-013).
- GAP: GitHub API refused for repositories other than laird/agents-ui from this cloud machine; reaction counts taken from HTML pages.
- GAP: laird/agents issues, not read (budget kept for the counter-evidence pass).
- GAP: AgentEx users, no public source searched. Temporal users: one forum search (SIG-016), blurbs only.
- GAP: Mastodon, not searched.

Contradictions:
- SIG-007 (silence polling cannot tell waiting from thinking) and SIG-011 (herdr's pattern matching, no hooks needed) against SIG-006 (a user switched to herdr for its waiting indication): detection works well enough for some, is guesswork for others.
- SIG-001 to SIG-004 (people want more notifications) against SIG-009 (notifications become noise with five or more sessions).

### 4b. Quote bank

> "I kept losing context running Claude Code, Codex, and a bunch of dev servers in parallel — six tmux panes, missed permission prompts, no idea which agent was waiting on me."

QUO-001 · a Hacker News commenter (maker of a competing dashboard) · 2026-04-25 · supports SIG-005 · comment, no points shown · https://news.ycombinator.com/item?id=47901927 · accessed 2026-11-02 · from F-008

> "Is Claude waiting for my answer? Did Gemini finish 10 minutes ago and I didn't notice?"

QUO-002 · a Lemmy poster (maker of a competing tool) · 2026-02-22 · supports SIG-008 · score 1, 0 comments · https://lemmy.durstig.online/post/6108 · accessed 2026-11-02 · from F-009

> "The session silently blocks until you happen to check the app, which can waste significant time if you're away from your phone."

QUO-003 · a GitHub issue author · 2026-02-27 · supports SIG-002 · 59 thumbs-up · https://github.com/anthropics/claude-code/issues/29438 · accessed 2026-11-02 · from F-011

> "being alerted is half of it, the other half is answering from the phone so the session unblocks without you returning to the desk."

QUO-004 · a GitHub commenter (maker of a competing tool) · 2026-06-21 · supports SIG-002, theme B · no reactions shown · https://github.com/anthropics/claude-code/issues/29438 · accessed 2026-11-02 · from F-011

> "Would love to see unread markers in the sessions list though. I am heavily using parallel sessions."

QUO-005 · a GitHub commenter · 2026-06-17 · supports SIG-001, theme B · no reactions shown · https://github.com/anthropics/claude-code/issues/57230 · accessed 2026-11-02 · from F-006

> "It makes notification center completely useless because it's flooded with stale "Ready for input" alerts."

QUO-006 · a GitHub commenter · 2026-02-14 · supports SIG-009 · 4 thumbs-up · https://github.com/anthropics/claude-code/issues/12046 · accessed 2026-11-02 · from F-012

> "And, silence for N seconds is just that, the poll doesn't know if that really means waiting for input or something else."

QUO-007 · a Hacker News commenter · 2026-06-29 · supports SIG-007 · comment, no points shown · https://news.ycombinator.com/item?id=48717413 · accessed 2026-11-02 · from F-007 · only voice for this point outside GitHub

> "One: visual indication which agent is working and which is waiting for an input."

QUO-008 · a Hacker News commenter (herdr user) · 2026-10-30 · supports SIG-006 · comment, no points shown · https://news.ycombinator.com/item?id=50302116 · accessed 2026-11-02 · from F-007

> "But without push notifications to know when Claude Code needs permissions or input, I end up "wasting" time a lot."

QUO-009 · a Hacker News commenter (contributor to a competing app) · 2025-08-28 · supports SIG-012 · comment, no points shown · https://news.ycombinator.com/item?id=45047168 · accessed 2026-11-02 · from F-008

> "Both are easy to miss when focus is on another window or monitor."

QUO-010 · a GitHub issue author · 2026-05-08 · supports SIG-001 · 56 thumbs-up · https://github.com/anthropics/claude-code/issues/57230 · accessed 2026-11-02 · from F-006

### 4c. Evidence log

```text
SIG-001 · Evidence · customer/community · GitHub issue, Claude Code (rung 1)
  Claim: The VS Code extension has no notification when Claude needs attention; its tab dot and status-bar text are easy to miss. Open; commenters run parallel sessions and want unread markers.
  Source: https://github.com/anthropics/claude-code/issues/57230 · published 2026-05-08 · accessed 2026-11-02
  Weight: 56 thumbs-up, 18 comments
  Saved: F-006

SIG-002 · Evidence · customer/community · GitHub issue, Claude Code (rung 1)
  Claim: Remote Control users had no push when a session waited on a permission prompt; the session blocked silently. Anthropic shipped "Push when actions required" on 2026-08-17. A commenter adds that answering from the phone is the other half.
  Source: https://github.com/anthropics/claude-code/issues/29438 · published 2026-02-27 · accessed 2026-11-02
  Weight: 59 thumbs-up (a follow-up comment has 20)
  Saved: F-011

SIG-003 · Evidence · customer/community · GitHub issue, Claude Code (rung 1)
  Claim: Claude Code's notifications did not reach the terminal from inside tmux; users built hook workarounds; a fix shipped in 2.1.78 but several commenters report it broke or did not work; closed as completed 2026-08-17.
  Source: https://github.com/anthropics/claude-code/issues/19976 · published 2026-01-22 · accessed 2026-11-02
  Weight: 26 thumbs-up, 12 comments
  Saved: F-005

SIG-004 · Evidence · customer/community · GitHub issue search, Claude Code (rung 1)
  Claim: 989 Claude Code issues carry "notification" in the title; page 1 includes requests for system notifications when Claude needs attention (#26581), a configurable idle-prompt timeout (#13922), and a 10-second Notification hook delay (#5186, open).
  Source: https://github.com/anthropics/claude-code/issues?q=is%3Aissue%20in%3Atitle%20notification%20sort%3Areactions-%2B1-desc · accessed 2026-11-02
  Weight: 989 results in a tracker of 5,000+ issues; top items 9 to 49 reactions
  Saved: F-002

SIG-005 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: A maker built a dashboard after missing permission prompts across six tmux panes, with no idea which agent was waiting; wanted to approve from a phone.
  Source: https://news.ycombinator.com/item?id=47901927 · published 2026-04-25 · accessed 2026-11-02
  Weight: comment on a Show HN; points not shown
  Saved: F-008

SIG-006 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: A long-time tmux user switched to herdr first for its visual indication of which agent is working and which is waiting.
  Source: https://news.ycombinator.com/item?id=50302116 · published 2026-10-30 · accessed 2026-11-02
  Weight: comment; points not shown
  Saved: F-007

SIG-007 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: Telling a waiting agent from a thinking one needs hooks or polling for silence, and silence does not reliably mean waiting.
  Source: https://news.ycombinator.com/item?id=48717413 · published 2026-06-29 · accessed 2026-11-02
  Weight: comment; points not shown
  Saved: F-007

SIG-008 · Evidence · customer/community · Lemmy post (rung 2)
  Claim: A developer running Claude Code and Gemini CLI in parallel says the hardest part was tracking which agent waits for an answer; built a kanban that moves a task to PENDING when the agent asks.
  Source: https://lemmy.durstig.online/post/6108 · published 2026-02-22 · accessed 2026-11-02
  Weight: score 1, 0 comments (cross-posted from Reddit)
  Saved: F-009

SIG-009 · Evidence · customer/community · GitHub issue, Claude Code (rung 1) · counter-evidence
  Claim: With five or more sessions, notifications become noise (stale "Ready for input" from idle tabs, pings for the focused tab); people want only "needs input" and "finished", focus awareness, and debounce. Disabling shipped 2026-08-17.
  Source: https://github.com/anthropics/claude-code/issues/12046 · published 2025-11-20 · accessed 2026-11-02
  Weight: 41 thumbs-up; top comment 23
  Saved: F-012

SIG-010 · Evidence · customer/community (maintainer) · GitHub issues, agents-ui (rung 1)
  Claim: The maintainer filed and closed: alerts should name the blocked role (#344), alert click should deep-link to the blocked agent (#340), and the web view's text field cannot answer arrow-key pickers or numbered prompts (#345, #343).
  Source: https://github.com/laird/agents-ui/issues?q=is%3Aissue · published 2026-09-01 to 2026-09-02 · accessed 2026-11-02
  Weight: 0 reactions each; 80 issues in the repository, all states
  Saved: F-010
  Note: the product's own maintainer, the only agents-ui user in the record

SIG-011 · Evidence · market/competitor · herdr README (rung 1, read as community)
  Claim: herdr detects blocked/done/working/idle by matching terminal output every 300 to 500 ms with no hooks; partial for some agents, "rough edges"; toasts and sounds opt-in; OS-level notifications planned, not available; no approve or answer feature.
  Source: https://github.com/maxjendrall/herdr · accessed 2026-11-02
  Weight: 0 stars on this repository (appears to be a fork)
  Saved: F-013

SIG-012 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: Without push notifications for permissions or input, a developer running parallel Claude Code sessions wastes time.
  Source: https://news.ycombinator.com/item?id=45047168 · published 2025-08-28 · accessed 2026-11-02
  Weight: comment; points not shown
  Saved: F-008

SIG-013 · Evidence · customer/community · GitHub comment, Claude Code (rung 1)
  Claim: No hook fires for AskUserQuestion (Claude asking a multiple-choice question), so hook-based alerts miss it.
  Source: https://github.com/anthropics/claude-code/issues/19976 · comment 2026-05-14 · accessed 2026-11-02
  Weight: 1 thumbs-up
  Saved: F-005

SIG-014 · Inference (title only) · customer/community · GitHub issue, Claude Code (rung 1)
  Claim: An AskUserQuestion left unanswered for 60 seconds lets the agent continue without the answer (title only; body not read).
  Source: https://github.com/anthropics/claude-code/issues/73125 · 2026-07-04 · accessed 2026-11-02
  Weight: 144 thumbs-up, closed
  Saved: F-001

SIG-016 · Evidence · customer/community · Temporal community forum search (rung 1, PM's extra fetch)
  Claim: Temporal users model a human wait as a workflow paused for a signal (or async activity completion) that carries the person's decision; several build their own UI for the people who approve, because those people will not use the Temporal Web UI. 18 matching posts since 2020, none about being notified that a workflow is waiting; engagement is low (0 to 1 likes per post).
  Source: https://community.temporal.io/search.json?q=human%20approval · posts 2020-10-30 to 2026-07-08 · accessed 2026-11-02
  Weight: 18 posts, top 1 like
  Saved: F-015 (F-014 was the same search with a longer query and returned nothing)

SIG-015 · Evidence · market/competitor · GitHub issues, claude-squad (rung 1)
  Claim: claude-squad's top 25 issues by reactions are about pane capture, worktrees and sessions; none is about noticing a waiting agent; one asks to fix auto-approve ("yolo mode").
  Source: https://github.com/smtg-ai/claude-squad/issues?q=is%3Aissue%20sort%3Areactions-%2B1-desc · accessed 2026-11-02
  Weight: 134 issues; top item 18 thumbs-up
  Saved: F-003
```

### 4d. Repository constraints

```text
CON-001 · Constraint · repository · Status: to confirm with engineering
  Claim: The only desktop alert (the Omarchy bar widget) reads /api/swarms, which holds tmux and herdr swarms only; Temporal villages are served on /api/village, so a Temporal workflow waiting on a person raises no alert.
  Where: contrib/omarchy/plugins/agents.swarm/Panel.qml, line 232; src/web/server.rs, lines 61 to 69 and 1094, commit 0019fe8
  Quote: `var swarms = parsed && Array.isArray(parsed.swarms) ? parsed.swarms : []`
  Implication: the PM's source of truth (Temporal) is invisible to the one push signal that exists.

CON-002 · Constraint · repository · Status: to confirm with engineering
  Claim: A herdr agent's target is herdr's pane id, but answering from the web sends tmux keys to that target, so a waiting herdr agent can be seen but not answered from agents-ui.
  Where: src/web/herdr.rs, line 108; src/web/server.rs, lines 520 to 535, commit 0019fe8
  Quote: `tmux_target: agent.pane_id.clone(),`
  Implication: "answer from here" for herdr needs a herdr input path (herdr's CLI or socket can send keys, per SIG-011).

CON-003 · Constraint · repository · Status: to confirm with engineering
  Claim: For tmux panes, waiting is detected from fixed phrases in the last 15 screen lines; a numbered-choice question is caught only after an interruption.
  Where: src/model/swarm.rs, lines 287 to 313, commit 0019fe8
  Quote: `|| tail_text.contains("Allow?")`
  Implication: tmux detection misses prompt types the phrase list does not know; herdr and Temporal report state directly and avoid this.

CON-004 · Constraint · repository · Status: to confirm with engineering
  Claim: The only push alert is Omarchy-specific (a Linux Hyprland desktop); the web dashboard and village send no browser notification, sound, or title change.
  Where: contrib/omarchy/plugins/agents.swarm/Panel.qml, line 300; src/web/ui.html and src/web/village.html (no Notification use found), commit 0019fe8
  Quote: `var cmd = ["omarchy-notification-send",`
  Implication: on macOS, any other Linux desktop, or a phone, nothing tells the developer an agent is waiting.

CON-005 · Constraint · repository · Status: to confirm with engineering
  Claim: With --read-only every non-GET request is refused, and Temporal is read only (agents-ui sends no signal), so a shared or in-cluster dashboard can show a waiting agent but not answer it.
  Where: src/web/server.rs, lines 1079 to 1085; src/web/temporal.rs, lines 15 to 18, commit 0019fe8
  Quote: `if req.method() == axum::http::Method::GET || req.method() == axum::http::Method::HEAD {`
  Implication: answering a Temporal workflow needs a signal path that does not exist; answering in a pod needs a non-read-only mode or a separate authenticated route.
```

## 5. Assumptions and open questions
- ASSUMPTION: the solo developer runs agents-ui on a machine they are not always looking at (laptop asleep, other window, away from desk).
- OPEN-001: what does "answering" a waiting Temporal workflow mean: a signal with a typed reply, an approve/deny, or opening AgentEx? (PM and engineering; changes scope.)

## 6. Decision record
| Check-in | Recommendation | Your decision | Reason |
|---|---|---|---|
| Case check | Confirm the case, web surface fed by herdr and Temporal waiting states | Confirm | "Source of truth is herdr sessions and temporal/agentex" |
| Research check | | | |
| Evidence review (only if a finding needed your call) | | | |
| Priority check | | | |
