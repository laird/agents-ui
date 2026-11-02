# Discovery brief: agents-ui · Attention and answering

| Field | Value |
|---|---|
| Status | approved |
| Product · focus area | agents-ui · attention-and-answering |
| User you design for · your brief | Solo developer · Never miss a blocked agent |
| Code snapshot | laird/agents-ui @ 0019fe8 (primary; read in place, working tree clean) |
| Research window | 2026-11-02T00:29Z to 2026-11-02T01:46Z · fetches 15/20 · searches 0/8 |
| Chosen opportunity | OPP-003 Trust the waiting signal |
| Evidence review | 13 findings · 0 blocking · 8 major and 5 minor, all dispositioned · quotes 10/10 verified · floor met · outputs/01-discovery-review.md |
| Run summary | Discovery complete · 77 min · fetches 15/20 · searches 0/8 · items 16 · quotes 10/10 verified · gaps 5 · findings 13, blocking 0, all resolved · approved |

## 1. The decision

### Chosen: OPP-003 Trust the waiting signal
- What it is: today agents-ui guesses that a tmux agent is waiting from a fixed list of phrases on its screen, which misses some prompts and raises false alarms. Reading the state each source reports itself (herdr's `blocked`, Temporal's `waiting_for_human`, GitHub's blocking labels) would make the "needs you" marker one the solo developer can believe.
- Why you chose it: "Cleanest more reliable signals."
- What tipped the recommendation: fit with the brief put OPP-001 first in Claude's recommendation; you chose OPP-003 instead, putting signal quality first, so that a waiting list or an alert built later rests on states the sources report rather than guesses.
- Evidence: 4 signals (3 Evidence), 2 source types · strongest: 26-thumbs-up issue (SIG-003) · counter-evidence: SIG-006 and SIG-011 (herdr's pattern matching works well enough for some users, looked F-013) · constraints: CON-003 hurts

### Considered and placed later
- **Next, OPP-001 One waiting list across herdr, Temporal and GitHub.** Everything waiting on the developer in one list, oldest first. Evidence: 6 signals (5 Evidence), 2 source types · strongest: 56-thumbs-up issue (SIG-001) · counter-evidence: SIG-002, looked F-011 · constraints: CON-001, CON-004. Why not now: it is only as good as the states it lists, which OPP-003 makes trustworthy first.
- **Later, OPP-002 An alert that names the agent and lets you answer from where you are.** Evidence: 5 signals (4 Evidence), 2 source types · strongest: 59-thumbs-up issue (SIG-002) · counter-evidence: SIG-009, looked F-012 · constraints: CON-002, CON-004, CON-005. Why not now: it needs answer paths into herdr and Temporal that do not exist, and an alert on unreliable signals becomes noise.

Main trade-off between the top two: OPP-001 delivers the brief directly and visibly; OPP-003 is invisible when it works but makes everything built on it believable.

| Slot | Opportunity | Reasoning (one sentence) |
|---|---|---|
| Now | OPP-003 Trust the waiting signal | The PM's choice: reliable signals first, so later features rest on reported states. |
| Next | OPP-001 One waiting list | Delivers "never miss" directly once the states it lists are trustworthy. |
| Later | OPP-002 Alert and answer from anywhere | Highest single signal but needs write paths and risks noise on weak signals. |

## 2. What we learned

People who run several coding agents at once (in tmux panes, herdr, or editor tabs) mostly
learn that an agent is waiting on them by going to look. An agent that stops for a permission
prompt or a question (a "blocked" agent: it can do nothing until a person answers) sits silent
until someone checks, and the people who notice this build their own fixes: notification
hooks (small scripts Claude Code runs on events), phone apps, Telegram bots, kanban boards. The
platform vendors are closing the single-session case (Claude Code released tmux-aware terminal
notifications by March 2026, with mixed reports that they work, and phone pushes for its Remote
Control mode in August 2026), but nothing in the
public record covers a swarm: several agents across several hosts, where the question is which
one needs you and whether you can answer it from where you are. When notifications do arrive
they turn into noise fast for people with five or more sessions, and detecting "waiting" from
screen text is itself unreliable.

Themes:
- Theme A, nobody tells you an agent is waiting: one sentence: agents block silently and people
  check back by hand or build their own alerts · who says so: 3 Claude Code issues read in full (59, 56 and 26 thumbs-up) and a
  title listing with more like them, 2 Hacker News comments, 1 Lemmy post · fixed or open:
  partly fixed for a single Claude Code session (tmux passthrough released by March 2026 with mixed
  reports, issue closed August 2026; Remote Control pushes August 2026), open for anything spanning several agents or tools · why it matters: a solo developer
  running a swarm loses agent-hours every time a prompt waits unseen.
- Theme B, tell me which one and let me answer from here: once alerted, people want the alert to
  name the agent, take them straight to it, and let them reply without going back to the desk ·
  who says so: 2 Claude Code issues, 2 Hacker News comments, 4 agents-ui issues filed by
  its maintainer (#340, #343, #344, #345) · fixed or open: agents-ui already names the blocked role and deep-links from
  its Omarchy alert (#344, #340, both closed); answering herdr and Temporal agents from agents-ui
  is not possible today (CON-002, CON-005); counter-evidence: alerts become noise beyond five
  sessions (SIG-009) · why it matters: the brief is "never miss", and an alert that is ignored or
  leads two screens away is a miss.
- Theme C, is it waiting or just thinking: detecting a waiting agent from screen text or silence
  is guesswork, and hook-based signals miss some prompts · who says so: 1 Hacker News comment,
  1 Claude Code issue (#19976, two of its comments), herdr's own README · fixed or open: sources disagree; herdr reports good
  results from pattern matching but says "there will be rough edges" · why it matters: a missed
  detection is a missed agent, and a false one trains the developer to ignore alerts.

Who we heard from: GitHub issue trackers sample power users who file bugs and feature requests,
here almost entirely Claude Code users, with Anthropic staff replying; Hacker News and Lemmy
sample tool builders and comparison shoppers, several of them makers of competing tools. Nobody
here is an agents-ui user other than its maintainer: the product has no public community yet. Missing
entirely: herdr's own users (its issue tracker could not be reached, GAP), Temporal or AgentEx
platform users, and anyone who supervises agents without being a heavy terminal user.

## 3. The three opportunities in detail

### OPP-001: One waiting list across herdr, Temporal and GitHub
- For: solo developer · Job: see everything that is waiting on me, wherever it waits, oldest first
- Supported by: SIG-001, SIG-005, SIG-006, SIG-008, SIG-010, SIG-016 · CON-001, CON-004
- In their words: QUO-001, QUO-002, QUO-005
- Evidence: 6 signals (5 Evidence), 2 source types · strongest: 56-thumbs-up issue (SIG-001) · counter-evidence: SIG-002 (Claude Code shipped pushes for one session, looked F-011) · constraints: CON-001 hurts, CON-004 hurts
- Evidence strength: good across GitHub, Hacker News, Lemmy and the Temporal forum, but no agents-ui user besides its maintainer / user impact: high, it answers "which agent is waiting on me" in one look / fit with the brief: direct, and matches the PM's three sources / delivery constraints: the alert path reads tmux and herdr only, Temporal waits never reach it (CON-001), and no alert outside Omarchy (CON-004) / uncertainty: whether blocked issues belong in the same list as blocked agents
- Exclusions check: tested against all seven case exclusions: no install, build or test command run (none); paid or feature-flagged functionality (none); billing or seats (none); third-party key or OAuth integrations (allowed by the PM (D-003): GitHub issues are read through the PM's own signed-in `gh`, which agents-ui already uses); legal or compliance claims (none); PM: billing and paid tiers (none); PM: third-party key integrations (allowed by the PM (D-003): GitHub issues are read through the PM's own signed-in `gh`, which agents-ui already uses); browser and desktop notifications only

### OPP-002: An alert that names the agent and lets you answer from where you are
- For: solo developer · Job: unblock a waiting agent without going back to the desk or the right pane
- Supported by: SIG-002, SIG-004, SIG-005, SIG-010, SIG-012 · CON-002, CON-004, CON-005
- In their words: QUO-003, QUO-004, QUO-009
- Evidence: 5 signals (4 Evidence), 2 source types · strongest: 59-thumbs-up issue (SIG-002) · counter-evidence: SIG-009 (alerts become noise beyond five sessions, looked F-012) · constraints: CON-002 hurts, CON-004 hurts, CON-005 hurts
- Evidence strength: strongest single items in the record, mostly from one crowd (Claude Code issue filers) / user impact: high, unblocking from anywhere saves the trip back / fit with the brief: strong, but "answer" goes beyond "never miss" / delivery constraints: herdr agents cannot be answered from agents-ui (CON-002), Temporal has no signal path and read-only mode refuses writes (CON-005) / uncertainty: how much answering from a phone needs authentication the product does not have
- Exclusions check: tested against all seven case exclusions: no install, build or test command run (none); paid or feature-flagged functionality (none); billing or seats (none); third-party key or OAuth integrations (a Slack or SMS channel would trigger it; scoped to browser and desktop notifications and agents-ui's own pages); legal or compliance claims (none); PM: billing and paid tiers (none); PM: third-party key integrations (a Slack or SMS channel would trigger it; scoped to browser and desktop notifications and agents-ui's own pages)

### OPP-003: Trust the waiting signal
- For: solo developer · Job: believe the "needs you" marker, so a missed or false alarm doesn't train them to ignore it
- Supported by: SIG-003, SIG-007, SIG-013 (Evidence; SIG-003 and SIG-013 are the same issue), SIG-014 (Inference) · CON-003
- In their words: QUO-007
- Evidence: 4 signals (3 Evidence), 2 source types · strongest: 26-thumbs-up issue (SIG-003) · counter-evidence: SIG-006 and SIG-011 (herdr's pattern matching works well enough for some users, looked F-013) · constraints: CON-003 hurts
- Evidence strength: thin; 3 Evidence items from 2 rungs, one of them a single Hacker News comment, and sources disagree / user impact: medium, invisible when it works / fit with the brief: supports it rather than delivering it / delivery constraints: tmux detection is a fixed phrase list (CON-003); herdr and Temporal report state directly / uncertainty: how often detection actually misses, which nothing here measures
- Exclusions check: tested against all seven case exclusions: no install, build or test command run (none); paid or feature-flagged functionality (none); billing or seats (none); third-party key or OAuth integrations (none); legal or compliance claims (none); PM: billing and paid tiers (none); PM: third-party key integrations (none)

## 4. Evidence

### 4a. Coverage
| Kind | Target | Logged | Channels used | Who those channels mostly hear from |
|---|---|---|---|---|
| Customer or community signals | 3 | 9 Evidence + 3 Inference | GitHub issues (rung 1), Hacker News, Lemmy (rung 2) | power users who file bugs; tool builders and comparison shoppers |
| Market or competitor observations | 2 | 2 (SIG-011 herdr, Evidence; SIG-015 claude-squad, Inference), no Wayback or adoption number | competitor README and issue list (rung 1, read as community) | what makers choose to publish |
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
| A. Nobody tells you an agent is waiting | SIG-002 (59 👍), SIG-001, SIG-003, SIG-004, SIG-014 | SIG-005, SIG-012 (HN), SIG-008 (Lemmy) | none | none | recurring | no: GitHub filers and HN tool builders differ | SIG-002's vendor reply (shipped Aug 2026 for one session), looked (F-011); also the theme's lead item |
| B. Tell me which one and let me answer from here | SIG-002 (comment), SIG-001 (comment), SIG-010 (own) | SIG-005, SIG-006 (HN) | none | none | recurring | partly | SIG-009 (alerts become noise), looked (F-012) |
| C. Is it waiting or thinking | SIG-013, SIG-003 (same issue); SIG-011 (herdr README, maker documentation) | SIG-007 (HN) | none | none | recurring, sources disagree | no | SIG-011 and SIG-006 (herdr detection praised), looked (F-013); SIG-011 also cited as support |

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

SIG-004 · Inference (titles only, from a search listing) · customer/community · GitHub issue search, Claude Code (rung 1)
  Claim: 989 Claude Code issues carry "notification" in the title; page 1 lists requests for system notifications when Claude needs attention (#26581), a configurable idle-prompt timeout (#13922), and a 10-second Notification hook delay (#5186, open). Bodies not read.
  Source: https://github.com/anthropics/claude-code/issues?q=is%3Aissue%20in%3Atitle%20notification%20sort%3Areactions-%2B1-desc · accessed 2026-11-02
  Weight: 989 issues with "notification" in the title, against 5,794 with the word anywhere (F-001); the per-row numbers on the listing are not thumbs-up counts (#29438 shows 15 there, 59 on its page) and are not used
  Saved: F-002

SIG-005 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: A maker built a dashboard after missing permission prompts across six tmux panes, with no idea which agent was waiting; wanted to approve from a phone.
  Source: https://news.ycombinator.com/item?id=47901927 · published 2026-04-25 · accessed 2026-11-02
  Weight: comment on a Show HN; points not shown
  Fetched as: https://hn.algolia.com/api/v1/search?query=claude%20code%20parallel%20agents%20permission%20prompt&tags=comment&hitsPerPage=10 (full text saved)
  Fetched as: https://hn.algolia.com/api/v1/search?query=claude%20code%20parallel%20agents%20permission%20prompt&tags=comment&hitsPerPage=10 (full text saved)
  Saved: F-008

SIG-006 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: A long-time tmux user switched to herdr first for its visual indication of which agent is working and which is waiting.
  Source: https://news.ycombinator.com/item?id=50302116 · published 2026-10-30 · accessed 2026-11-02
  Weight: comment; points not shown
  Fetched as: https://hn.algolia.com/api/v1/search?query=agents%20waiting%20for%20input%20tmux&tags=comment&hitsPerPage=10 (full text saved)
  Fetched as: https://hn.algolia.com/api/v1/search?query=agents%20waiting%20for%20input%20tmux&tags=comment&hitsPerPage=10 (full text saved)
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
  Fetched as: https://lemmy.world/api/v3/search?q=coding%20agents%20notification&type_=Posts&sort=TopYear&limit=10 (full text saved)
  Saved: F-009

SIG-009 · Evidence · customer/community · GitHub issue, Claude Code (rung 1) · counter-evidence
  Claim: With five or more sessions, notifications become noise (stale "Ready for input" from idle tabs, pings for the focused tab); people want only "needs input" and "finished", focus awareness, and debounce. Disabling shipped 2026-08-17.
  Source: https://github.com/anthropics/claude-code/issues/12046 · published 2025-11-20 · accessed 2026-11-02
  Weight: 41 thumbs-up; top comment 23
  Saved: F-012

SIG-010 · Evidence · customer/community (maintainer) · GitHub issues, agents-ui (rung 1)
  Claim: The maintainer filed and closed: alerts should name the blocked role (#344), alert click should deep-link to the blocked agent (#340), and the web view's text field cannot answer arrow-key pickers or numbered prompts (#345, #343).
  Source: https://github.com/laird/agents-ui/issues?q=is%3Aissue · published 2026-09-01 to 2026-09-02 · accessed 2026-11-02
  Weight: 0 reactions each; 80 issues among the first 100 items the API returned (the repository's total is not known from this page)
  Fetched as: https://api.github.com/repos/laird/agents-ui/issues?state=all&per_page=100 (full text saved)
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
  Weight: unknown; the listing shows 144 beside it, but listing numbers are not thumbs-up counts (see SIG-004); closed
  Saved: F-001

SIG-016 · Inference (forum search blurbs, not full posts) · customer/community · Temporal community forum search (rung 1, PM's extra fetch)
  Claim: Temporal users model a human wait as a workflow paused for a signal (or async activity completion) that carries the person's decision; one post (19653) builds its own UI for business users who will not use the Temporal Web UI; one (758) asks how to request a human approval by email. 18 matching posts since 2020; engagement is low (0 to 1 likes per post).
  Source: https://community.temporal.io/search.json?q=human%20approval · posts 2020-10-30 to 2026-07-08 · accessed 2026-11-02
  Weight: 18 posts, top 1 like
  Saved: F-015 (F-014 was the same search with a longer query and returned nothing)

SIG-015 · Inference (titles only, from a search listing) · market/competitor · GitHub issues, claude-squad (rung 1)
  Claim: claude-squad's top 25 issues by reactions are about pane capture, worktrees and sessions; none is about noticing a waiting agent; one asks to fix auto-approve ("yolo mode").
  Source: https://github.com/smtg-ai/claude-squad/issues?q=is%3Aissue%20sort%3Areactions-%2B1-desc · accessed 2026-11-02
  Weight: 134 issues; per-row numbers on the listing are not used (see SIG-004)
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
- OPEN-002: should GitHub blocking labels (needs-design and the rest) count as "waiting on you" in the same signal model as agent states, or stay a separate kind? (PM; changes the scope of OPP-003 and OPP-001.)
- OPEN-001: what does "answering" a waiting Temporal workflow mean: a signal with a typed reply, an approve/deny, or opening AgentEx? (PM and engineering; changes scope.)

## 6. Decision record
| Check-in | Recommendation | Your decision | Reason |
|---|---|---|---|
| Case check | Confirm the case, web surface fed by herdr and Temporal waiting states | Confirm | "Source of truth is herdr sessions and temporal/agentex" |
| Research check | Continue to opportunities | One more fetch: Temporal | "Temporal is one source, along with herdr, and GitHub issues." |
| Evidence review (RV-008) | (asked without a recommendation) | Yes, gh is fine | "Your own signed-in gh is not a new integration; agents-ui already uses it." (the option chosen) |
| Priority check | Now OPP-001, Next OPP-002, Later OPP-003 (fit with the brief tipped it) | Now OPP-003 Trust the signal | "Cleanest more reliable signals." |
