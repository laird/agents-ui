# Discovery brief: agents-ui · A feature from spec to production

| Field | Value |
|---|---|
| Status | draft |
| Product · focus area | agents-ui · feature-to-production (run 2; run 1 archived in outputs/attention-and-answering/) |
| User you design for · your brief | Tech lead · Know when a feature is live |
| Code snapshot | laird/agents-ui @ e4422c5 (read in place, working tree clean; moved on from run 1's 0019fe8) |
| Research window | 2026-11-02T03:59Z to (open) · fetches 17/20 · searches 0/8 |
| Chosen opportunity | (pending priority check-in) |
| Evidence review | (pending) |
| Run summary | (written at close) |

## 1. The decision

(Written last, after the priority check-in.)

## 2. What we learned

A feature is a tree: a parent task broken into subtasks that are built separately and only
mean something to users once all of them are done, validated together and deployed. The
tools people use show the leaves. GitHub added sub-issues in 2025 and a hierarchy view in
2026, and its users immediately asked to filter out finished work and to see the hierarchy
on roadmaps; a developer running about a hundred Claude Code sessions says the agents cope
but nobody holds the whole picture. Knowing what is in which environment (dev, UAT,
production) is a second, separate problem people solve by convention (tagging deployed
commits), and coding agents make it worse by confusing dev with production. agents-ui
today tracks single issues: a decomposed feature drops out of the picture, and "landed"
means the latest release runs, not that a given feature is in it.

Themes:
- Theme A, a feature is a tree but tools show the leaves: one sentence: people lose track of
  what a whole effort amounts to once it branches into sub-issues and pull requests · who
  says so: GitHub's hierarchy-view discussion (69 votes), the maintainer's own two open
  autocoder proposals (#146, #148), 2 Hacker News comments, 1 Lemmy post · fixed or open:
  partly fixed in trackers (GitHub sub-issues 2025, hierarchy view 2026, with filtering and
  roll-up still asked for); open in the swarm, where parent and child exist only as labels ·
  why it matters: the tech lead's question is about the feature, and the picture only shows
  its parts.
- Theme B, what is live where: one sentence: with several environments, knowing which
  commit (and so which feature) is in production is hard enough that teams adopt tagging
  conventions · who says so: 3 Hacker News comments, 1 Lemmy post · fixed or open: open;
  looked for a tool praised for solving it and found none · why it matters: "live" is the
  brief.
- Theme C, validate the whole feature before production: one sentence: subtasks pass one
  by one, but siblings drift apart and the whole needs its own check (a soak in dev,
  review before production) · who says so: the maintainer's autocoder proposal (#148), 1
  Lemmy post, 1 Hacker News comment · fixed or open: sources disagree: teams with feature
  flags skip staging entirely · why it matters: a feature is only live when the whole of
  it works there.

Who we heard from: GitHub's community discussions sample people who manage work in
GitHub Projects, often for teams; the laird/agents issues are the swarm's maintainer;
Hacker News and Lemmy sample engineers and tool builders, several of them running coding
agents at scale. Missing: product managers and other non-engineers who ask "is it live",
teams on Jira or Linear (no source read), and anyone running a UAT sign-off by hand.

## 3. The three opportunities in detail

### OPP-004: Features as one thing in the village
- For: tech lead · Job: see where each feature stands at a glance, not just its parts
- Supported by: SIG-017, SIG-018, SIG-019, SIG-020, SIG-021, SIG-022, SIG-031 · CON-006, CON-007, CON-008
- In their words: QUO-011, QUO-013, QUO-017
- Evidence: 7 signals, 3 source types · strongest: 69-vote GitHub discussion (SIG-017) · counter-evidence: SIG-028 (sub-issues as "endless discussions", looked F-024) · constraints: CON-006, CON-007, CON-008 hurt; SIG-031 helps (GitHub already counts a parent's completed sub-issues)
- Evidence strength: good, across GitHub discussions, the swarm's own maintainer, Hacker News and Lemmy / user impact: high, it changes what the picture is about / fit with the brief: indirect, a prerequisite for knowing when a feature is live / delivery constraints: parent and child aren't in the model at all; autocoder records them only as labels / uncertainty: whether the picture should group a feature's houses together or draw the feature as its own object
- Exclusions check: tested against all seven case exclusions: no install, build or test command run (none); paid or feature-flagged functionality (none); billing or seats (none); third-party key or OAuth integrations (reads GitHub sub-issues through the PM's own signed-in gh, allowed in D-003); legal or compliance claims (none); PM: billing and paid tiers (none); PM: third-party key integrations (as above)

### OPP-005: "Live" per feature and per environment
- For: tech lead · Job: know, without asking anyone, that a whole feature has reached production
- Supported by: SIG-022, SIG-023, SIG-024, SIG-025, SIG-030 · CON-009, CON-010
- In their words: QUO-012, QUO-014, QUO-016
- Evidence: 5 signals, 2 source types · strongest: a 2024 Hacker News comment (SIG-023; points not shown) · counter-evidence: looked (F-029), none · constraints: CON-009 hurts, CON-010 helps; SIG-030 helps (GitHub records which sha reached which environment)
- Evidence strength: moderate; the demand is concentrated in one kind of source, and GitHub's own documentation shows the data exists (SIG-030) / user impact: high, it is the brief / fit with the brief: direct / delivery constraints: worlds already match a release to the platform's builds (CON-009) and environments are already generic (CON-010), but a feature's subtasks are not linked (CON-007); teams deploying through GitHub Environments record the sha per environment, others need the image-tag or build-ID match / uncertainty: what "live" means with feature flags (SIG-026)
- Exclusions check: tested against all seven case exclusions: none triggered beyond the gh read already allowed (D-003)

### OPP-006: Validate the whole feature before production
- For: tech lead · Job: see that a feature passed its own check (UAT, a soak) as a whole before it ships
- Supported by: SIG-019, SIG-022, SIG-029 · CON-010
- In their words: QUO-016
- Evidence: 3 signals, 2 source types · strongest: the maintainer's open proposal on sibling drift (SIG-019; no reactions shown) · counter-evidence: SIG-026 (no staging, feature flags instead, looked F-030) · constraints: CON-010 hurts (the requires and test fields that gate stages are not read)
- Evidence strength: thin, and contradicted / user impact: medium / fit with the brief: supports it / delivery constraints: no feature-level gate exists anywhere in the pipeline / uncertainty: high, since some teams skip staging entirely
- Exclusions check: tested against all seven case exclusions: none triggered

## 4. Evidence

### 4a. Coverage
| Kind | Target | Logged | Channels used | Who those channels mostly hear from |
|---|---|---|---|---|
| Customer or community signals | 3 | 12 Evidence | GitHub discussions and issues (rung 1), Hacker News, Lemmy (rung 2) | people managing work in GitHub; the swarm's maintainer; engineers and tool builders |
| Market or competitor observations | 2 | 1 (SIG-017: GitHub's own hierarchy view), no adoption or Wayback number | GitHub community | what a vendor chooses to ship |
| Official product or release signal | 1 | 2 (SIG-030, SIG-031; PM allowed official sources for run 2, D-007) | GitHub REST documentation (rung 6) | the maker's own framing |
| Repository constraints | 2 | 5 | agents-ui source at e4422c5 | the code |

Queries used:
- Hacker News comments: `sub-issues epic progress tracking` (0), `know when feature deployed production`, `breaking features into tasks coding agents`, `"sub-issues" github`, `"is it deployed"`, `"staging" "UAT" feature release`, `"what's in production"`, `"github deployments" environment`, `"feature flags" "no staging"`
- GitHub: community discussions `sub-issues progress` (top); laird/agents issues `decompose`
- Lemmy posts: `sub-issues`

Sources reached: GitHub community discussions, laird/agents issues, Hacker News, Lemmy, and (D-007) GitHub's REST documentation for Deployments and sub-issues. Empty
or off-topic: three Hacker News queries (F-016 empty; F-017, F-025 off-topic). The fetch tool
returns summaries, not text, for GitHub pages (F-019 to F-023), so their claims are logged but
not quotable. Dropped by the PM (run 1): app reviews, numbers and competitors, official and search.

Triangulation:
| Theme | Forum, GitHub | Lemmy, Mastodon, HN | Reviews | Numbers, competitors, official | Recurrence | Same crowd? | Counter-evidence |
|---|---|---|---|---|---|---|---|
| A. A feature is a tree | SIG-017, SIG-018, SIG-019 | SIG-020, SIG-021, SIG-022 | none | none | recurring | partly: GitHub Projects users and agent builders differ | SIG-028, looked (F-024); GitHub shipped sub-issues and hierarchy view (SIG-017) |
| B. What is live where | none | SIG-022, SIG-023, SIG-024, SIG-025 | none | SIG-030 (official: the data exists) | concentrated (demand in one rung; the official rung shows supply, not demand) | yes | looked (F-029), none |
| C. Validate the whole | SIG-019 | SIG-022, SIG-029 | none | none | recurring, sources disagree | no | SIG-026, looked (F-030) |

Gaps:
- GAP: GitHub page text, fetch tool returns summaries only (F-019 to F-023): claims logged, no quotes.
- GAP: three Hacker News queries empty or off-topic (F-016, F-017, F-025).
- GAP: Jira and Linear users, product managers, UAT owners: no source read.
- Official sources: GitHub's Deployments and sub-issues docs read with the PM's permission for this run (D-007); nothing else official.

Contradictions:
- SIG-019 and SIG-022 (validate or soak the whole before production) against SIG-026 (no staging environment; feature flags gate the rollout instead).
- SIG-017 and SIG-020 (trackers now model parent and child) against SIG-018 (the swarm doesn't: labels only).

### 4b. Quote bank

> "The agents cope fine. I didn't. Long efforts branch into many PRs and sub-issues, ops work pulls me sideways, and after a few weeks nobody holds the whole picture, including me."

QUO-011 · a Lemmy poster running about a hundred Claude Code sessions · date not shown · supports SIG-022 · see F-028 for score · Lemmy cross-post of r/ClaudeCode · accessed 2026-11-02 · from F-028

> "If there are multiple environments it quickly gets impossible to keep track of "what's in production" as a branch."

QUO-012 · a Hacker News commenter · 2024-01-22 · supports SIG-023 · comment, no points shown · https://news.ycombinator.com/item?id=39084693 · accessed 2026-11-02 · from F-027

> "...a framework for modeling proper parent-child relationships in their system, which is something quite hard to get right..."

QUO-013 · a Hacker News commenter · 2025-01-19 · supports SIG-020 · comment, no points shown · https://news.ycombinator.com/item?id=42761674 · accessed 2026-11-02 · from F-024

> "the inability of the LLM from confusing whats in dev, whats in production, whats in localhost and whats remote"

QUO-014 · a Hacker News commenter · 2026-05-05 · supports SIG-024 · comment, no points shown · https://news.ycombinator.com/item?id=48023339 · accessed 2026-11-02 · from F-027

> "At Facebook too there was no staging environment. Engineers had their dev VM and then after PR review things just went into prod"

QUO-015 · a Hacker News commenter · 2022-04-03 · supports SIG-026 (counter-evidence) · comment, no points shown · https://news.ycombinator.com/item?id=30899520 · accessed 2026-11-02 · from F-030

> "Session URLs, "blocked until I decide X" and "sitting in dev waiting for a soak" are scratch state"

QUO-016 · a Lemmy poster running about a hundred Claude Code sessions · date not shown · supports SIG-022 · see F-028 · accessed 2026-11-02 · from F-028

> "VS code organizes their work with parent sprint issues ("iteration plan", see [1])."

QUO-017 · a Hacker News commenter · 2025-01-19 · supports SIG-021 · comment, no points shown · https://news.ycombinator.com/item?id=42759814 · accessed 2026-11-02 · from F-024

> "PRs still need to be reviewed for production."

QUO-018 · a Hacker News commenter · 2026-03-04 · supports SIG-029 · comment, no points shown · https://news.ycombinator.com/item?id=47248147 · accessed 2026-11-02 · from F-018 · only voice for this point

### 4c. Evidence log

```text
SIG-017 · Evidence · market/competitor · GitHub community discussion (rung 1)
  Claim: GitHub's hierarchy view for sub-issues (public preview, Jan 2026) drew requests to filter out finished sub-issues, to stop sub-issues appearing twice, and to show the hierarchy on roadmaps; epics are rarely closed, so done work clutters nested views. The thread doesn't discuss parent progress or roll-up.
  Source: https://github.com/orgs/community/discussions/184225 · published 2026-01-13 · accessed 2026-11-02
  Weight: 69 votes on the post; top replies 21 and 20 votes; 88 comments (F-019)
  Saved: F-021 (summary by the fetch tool; no verbatim text)

SIG-018 · Evidence · customer/community (maintainer) · laird/agents issue (rung 1)
  Claim: autocoder's issue contract has no link verb; parent and child exist only as decomposed and subtask labels; workers can claim subtasks whose prerequisites are still open. Proposed: real dependency edges (blocks, parent, discovered-from). Rated by its author the highest-leverage item in the set.
  Source: https://github.com/laird/agents/issues/146 · published 2026-09-17 · accessed 2026-11-02
  Weight: open, labelled proposal and needs-design; no comments
  Saved: F-023 (summary)

SIG-019 · Evidence · customer/community (maintainer) · laird/agents issue (rung 1)
  Claim: subtasks carry only raw issue text, so each worker rebuilds context and siblings risk inconsistent interpretations and interface drift; proposed a spec per subtask and a shared parent design note.
  Source: https://github.com/laird/agents/issues/148 · published 2026-09-17 · accessed 2026-11-02
  Weight: open, labelled proposal and needs-design; no comments
  Saved: F-022 (summary)

SIG-020 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: GitHub's sub-issues are really a framework for parent-child relationships, which is hard to get right; this was GitHub's third or fourth attempt.
  Source: https://news.ycombinator.com/item?id=42761674 · published 2025-01-19 · accessed 2026-11-02
  Weight: comment; points not shown; 16 matching comments (F-024)
  Fetched as: https://hn.algolia.com/api/v1/search?query=%22sub-issues%22%20github&tags=comment&hitsPerPage=10 (full text saved)
  Saved: F-024

SIG-021 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: VS Code plans work with parent "iteration plan" issues; the commenter adopted the pattern and prefers first-class sub-issues to markdown checklists.
  Source: https://news.ycombinator.com/item?id=42759814 · published 2025-01-19 · accessed 2026-11-02
  Weight: comment; points not shown
  Fetched as: as SIG-020
  Saved: F-024

SIG-022 · Evidence · customer/community · Lemmy post (rung 2)
  Claim: running four tmux sessions of 20 to 30 Claude Code chats each, the author lost track of long efforts that branch into many PRs and sub-issues: what is blocked on what, which session did what, and environment state such as "sitting in dev waiting for a soak", which he keeps off GitHub; adopted Beads (a dependency-graph tracker) with efforts as top-level items and tasks under them.
  Source: https://www.reddit.com/r/ClaudeCode/comments/1wdrgz0/ (via Lemmy search) · accessed 2026-11-02
  Weight: see F-028 for score and comments
  Fetched as: https://lemmy.world/api/v3/search?q=sub-issues&type_=Posts&sort=TopAll&limit=10 (full text saved)
  Saved: F-028

SIG-023 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: with several environments, tracking "what's in production" as a branch becomes impossible; tag deployed commits or record the deployed hash instead.
  Source: https://news.ycombinator.com/item?id=39084693 · published 2024-01-22 · accessed 2026-11-02
  Weight: comment; points not shown; 26 matching comments (F-027)
  Fetched as: https://hn.algolia.com/api/v1/search?query=%22what%27s%20in%20production%22&tags=comment&hitsPerPage=12 (full text saved)
  Saved: F-027

SIG-024 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: coding agents confuse dev, production, localhost and remote environments.
  Source: https://news.ycombinator.com/item?id=48023339 · published 2026-05-05 · accessed 2026-11-02
  Weight: comment; points not shown
  Fetched as: as SIG-023
  Saved: F-027

SIG-025 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: shipping small changes often keeps what is being worked on close to what is in production, which makes delivery quicker and less risky.
  Source: https://news.ycombinator.com/item?id=44070416 · published 2025-05-23 · accessed 2026-11-02
  Weight: comment; points not shown
  Fetched as: as SIG-023
  Saved: F-027

SIG-026 · Evidence · customer/community · Hacker News comment (rung 2) · counter-evidence
  Claim: Facebook had no staging environment; after review, changes went to production gated by feature flags and rolled out slowly.
  Source: https://news.ycombinator.com/item?id=30899520 · published 2022-04-03 · accessed 2026-11-02
  Weight: comment; points not shown
  Fetched as: https://hn.algolia.com/api/v1/search?query=%22feature%20flags%22%20%22no%20staging%22&tags=comment&hitsPerPage=10 (full text saved)
  Saved: F-030

SIG-027 · Inference (listing) · customer/community · GitHub community discussions listing (rung 1)
  Claim: "Issue fields: structured issue metadata" is among the top Projects and Issues discussions for sub-issues.
  Source: https://github.com/orgs/community/discussions/189141 · listed 2026-03-10 · accessed 2026-11-02
  Weight: 93 upvotes, 999 comments shown on the listing
  Saved: F-019

SIG-030 · Evidence · official · GitHub REST docs, Deployments (rung 6, allowed by the PM for this run, D-007)
  Claim: a GitHub deployment records the ref, the exact sha, the environment (e.g. staging or production) and when it was created; deployments can be listed by environment or by sha; statuses run error, failure, pending, in_progress, queued, success, and a success marks earlier deployments to that environment inactive.
  Source: https://docs.github.com/en/rest/deployments/deployments · accessed 2026-11-02
  Weight: official documentation
  Saved: F-031

SIG-031 · Evidence · official · GitHub REST docs, sub-issues (rung 6, allowed by the PM for this run, D-007)
  Claim: sub-issues can be listed for a parent and the parent found for a sub-issue; every issue carries a sub_issues_summary with total, completed and percent_completed.
  Source: https://docs.github.com/en/rest/issues/sub-issues · accessed 2026-11-02
  Weight: official documentation
  Saved: F-032

SIG-028 · Evidence · customer/community · Hacker News comment (rung 2) · counter-evidence
  Claim: sub-issues invite endless debate about whether to use subtasks or sub-issues.
  Source: https://news.ycombinator.com/item?id=42755748 · published 2025-01-19 · accessed 2026-11-02
  Weight: comment; points not shown
  Fetched as: as SIG-020
  Saved: F-024

SIG-029 · Evidence · customer/community · Hacker News comment (rung 2)
  Claim: a team delegates whole features to Claude Code, but its pull requests still need review before production.
  Source: https://news.ycombinator.com/item?id=47248147 · published 2026-03-04 · accessed 2026-11-02
  Weight: comment; points not shown
  Fetched as: https://hn.algolia.com/api/v1/search?query=breaking%20features%20into%20tasks%20coding%20agents&tags=comment&hitsPerPage=10 (full text saved)
  Saved: F-018
```

### 4d. Repository constraints

```text
CON-006 · Constraint · repository · Status: to confirm with engineering
  Claim: a decomposed parent issue is treated as taken and leaves the backlog; nothing in the village represents it afterwards.
  Where: src/web/framework.rs, line 745, commit e4422c5
  Quote: `} else if i.is_working || i.labels.iter().any(|l| l == "decomposed") {`
  Implication: the feature itself disappears from the picture the moment it is broken up.

CON-007 · Constraint · repository · Status: to confirm with engineering
  Claim: the neutral report's Task has an id, title, labels, priority, state and url, and no parent or children.
  Where: src/web/framework.rs, lines 339 to 353 (struct Task), commit e4422c5
  Quote: `pub id: String,`
  Implication: no source (autocoder labels, GitHub sub-issues, Beads) can tell the village which subtasks make a feature.

CON-008 · Constraint · repository · Status: to confirm with engineering
  Claim: landed work is tracked per issue, from the issue number at the end of each tip ref's name.
  Where: src/web/village.rs, lines 494 to 497, commit e4422c5
  Quote: `let (sha, issue) = rest.rsplit_once('-')?;`
  Implication: a feature's subtasks travel the canal as separate crates; nothing marks when the last of them passes a stage.

CON-009 · Constraint · repository · Status: to confirm with engineering
  Claim: a world's "landed" says whether any platform worker runs the latest release, not whether a given feature is in what runs.
  Where: src/web/world.rs, lines 108 to 112, commit e4422c5
  Quote: `(Some(r), false) => Some(platform.builds.iter().any(|b| same_build(b, r))),`
  Implication: "live" exists at release level only; a feature-level answer needs which release first carried each subtask.

CON-010 · Constraint · repository · Status: to confirm with engineering
  Claim: environments come from .autocoder.json in order, each with a name and a ref; autocoder's requires and test fields (stage order and gate) are not read.
  Where: src/web/village.rs, lines 468 to 480, commit e4422c5
  Quote: `branch: Option<String>,`
  Implication: dev, UAT and production already work as locks; a feature-level validation gate has nothing to read yet.
```

## 5. Assumptions and open questions
- ASSUMPTION: the tech lead's swarm decomposes features with autocoder's labels or GitHub sub-issues, not in a separate planning tool.
- OPEN-003: with feature flags, is a feature "live" when its code is in production or when its flag is on? (PM; changes the definition of done.)
- OPEN-004: where do specs live (an issue body, a PLAN.md, a spec file), and should the village show the spec as the feature's origin? (PM.)

## 6. Decision record
| Check-in | Recommendation | Your decision | Reason |
|---|---|---|---|
| Case check (run 2) | Confirm the focus area and plan | Confirm | "A new feature is a high level task, often broken into subtracts to get build, then when all subtracts are done, the whole thing gets validated and deployed." |
| Research check | Continue to opportunities | Fold GitHub's own SDLC data into the brief, with one official check | "Yes" (to folding it in and checking GitHub's documentation) |
| Evidence review (only if a finding needed your call) | | | |
| Priority check | | | |
