# Prototype notes: agents-ui · A feature from spec to production

| Field | Value |
|---|---|
| Status | draft |
| Started | 2026-11-05T16:27:05Z |
| Based on | outputs/01-discovery.md rev 2 · OPP-005 (provisional) |
| Path used | (step 5) |
| File | outputs/prototype/index.html |
| Surface · viewport | desktop web · 1440×900 |
| Run summary | (written at close) |

## 1. Actor, job, and user story
- Actor: the tech lead running the swarm · Job: "Is feature X live in production yet, and if not, where is it?"
- As a tech lead, when a feature's spec has been broken into subtasks for the swarm, I want to follow the whole feature through dev, UAT and production so that I know it is live without asking anyone, and see where it is stuck when it is not.

## 2. Approved flow and states
- Starts when / ends when: the feature's spec arrives as a parent issue / the whole feature is marked "Live in prod"
- Main path:
  1. Spec: "Saved carts" (#460) arrives as a banner with a spec scroll, split into 4 subtasks (#461 to #464).
  2. Follow: the tech lead clicks the banner; a trail links it to its subtasks; a strip reads Spec, Build 0/4, Dev 0/4, UAT 0/4, Prod 0/4.
  3. Build: workers claim and land the subtasks; Build counts to 4/4.
  4. Promote: subtasks pass the dev lock, the UAT lock and the castle (prod); each stage counts n/4 and is reached only at 4/4. Per-subtask projects (the default) move each crate on its own, so a feature can read "UAT 2/4"; whole-feature projects hold the crates at the quay until all have landed, then move them as one barge.
  5. Live: prod runs code containing all 4; the banner reads "Live in prod", then fades after a while.
- Alternate state A (error, Stuck): #463 needs design; red pennant on the banner, the trail leads to the waving worker, the strip reads "Build 3/4: #463 needs design".
- Alternate state B (loading, Can't tell): no Deployment and no branch ref for UAT; the UAT lock is fogged with "?", the strip reads "UAT: unknown", not "not live".
- Several features: each feature in flight has its own banner; following one draws only its trail and strip.
- PM's answers in step 1: actor "Tech lead: is it live?"; start and end "Spec to 'live'" (reason: "Whole lifecycle"); main action, in the PM's words: "I want to view the overall flow from spec to feature in prod, or bug report to fix in prod." Narrowed to one flow: "Feature: spec to prod"; states "Can't tell" and "Stuck".
- Spec review (D-011), the PM's answers: partial progress "Both, per project"; linking "Sub-issues, fall back to body"; environment source "Deployments, then branches"; live "Code in prod", and "we can figure out feature flags later"; several features "All, one per feature"; dropped subtasks "Drop it from the total"; spec "could be either just in the issue body or in some cases a document linked from the issue body"; next "Update spec, then prototype".

### Definitions settled in the spec review (D-011)
- Feature: a parent issue with subtasks, from GitHub sub-issues when present, else from issues whose body says `Sub-task of #N` (autocoder creates no sub-issues; it writes that line and labels the parent `decomposed`, the children `subtask`).
- Subtask total: open plus closed-as-completed; closed as not planned leaves the total and is named in the dialog.
- Reached an environment: the newest successful GitHub Deployment's sha, else the environment's branch ref from `.autocoder.json`, else unknown. A feature reaches a stage when every subtask in the total has.
- Live: all the feature's code runs in prod. Feature flags out of scope: apps differ in flag mechanism; follow issues deployed to dev, UAT and prod (D-013).
- Spec: the parent issue's body, or a document it links to (OPEN-004 answered).
- Promotion mode: per subtask (default) or whole feature, set per project. The setting's name and home (likely `.autocoder.json` beside the environments) are for the PRD.

## 3. Linked discovery evidence
| Flow element | Evidence or constraint |
|---|---|
| The feature as one banner with its subtasks | SIG-022, SIG-031 · CON-006, CON-007 |
| Subtasks from sub-issues, else `Sub-task of #N` | SIG-031 · CON-007 (autocoder writes the body line, not sub-issues) |
| Strip: Spec, Build n/4, Dev n/4, UAT n/4, Prod n/4 | SIG-023, SIG-033 · CON-008 (subtasks promote one by one) |
| Promotion through locks per environment | SIG-032, SIG-034 · CON-010 |
| "Live in prod" from what prod runs | SIG-030 · CON-009 |
| Can't tell (unknown, not false) | SIG-024, SIG-030 · CON-010 (branch refs alone always answer; unknown needs neither a Deployment nor a ref) |
| Stuck (blocked subtask holds the feature) | SIG-018, SIG-034 · CON-008 |

## 4. Scope boundaries
- In: features from spec to live, several at once; their subtasks; the integration gate; dev, UAT and prod; both promotion modes; the two states.
- Out: attention and answering (run 1); the bug-to-fix loop (already the story replay; the PM asked for it, kept for another run); approving or promoting (not a console); launching swarms; writing specs; feature flags (deferred by the PM); billing; third-party keys.

## 5. Design brief
<details>
<summary>Design brief as used to build the prototype (revised after the spec review, D-011)</summary>

#### Design brief: agents-ui · A feature from spec to production

##### Context
- Product: agents-ui watches swarms of AI coding agents, drawn as a 16-bit village.
- Persona: Tech lead · Mandate: Know when a feature is live
- Selected opportunity: OPP-005: "Live" per feature and per environment: know, without asking anyone, that a whole feature has reached production.
- Discovery evidence this flow answers: SIG-023 ("what's in production" is hard across environments), SIG-033 ("done" means many things), SIG-022 (environment state lives nowhere), SIG-030 (GitHub records the sha per environment), SIG-031 (GitHub sub-issues), CON-006 to CON-010.
- Revised after the PM's spec review (D-011): partial promotion, feature linking, the environment source, "live", several features, dropped subtasks, and where the spec lives.

##### Definitions (D-011)
- **Feature**: a parent issue with subtasks. Subtasks are found from GitHub sub-issues when the parent has any; otherwise from issues whose body says `Sub-task of #N` (autocoder's convention, with its `decomposed` and `subtask` labels). Every such parent that is not yet live is a feature.
- **Subtask total**: open subtasks plus those closed as completed. A subtask closed as not planned (won't fix, duplicate, folded into a sibling) leaves the total, so 4 becomes 3, and the feature's dialog names it.
- **Reached an environment**: per subtask, its landed commit is in what the environment runs. Source, in order: the newest successful GitHub Deployment to that environment (its sha); else the environment's branch ref from `.autocoder.json`; else **unknown**. A feature reaches an environment when every subtask in the total has.
- **Live**: the whole feature has reached prod; that is, all its code runs in production. Feature flags are out of scope for now (OPEN-003 deferred by the PM: "we can figure out feature flags later").
- **Spec**: the parent issue's body, or a document that body links to. The spec scroll opens the linked document when there is one, else the issue.

##### The flow
- Actor: the tech lead running the swarm
- Job: "Is feature X live in production yet, and if not, where is it?"
- Starts when: a feature's spec arrives as a parent issue
- Ends when: the whole feature is marked "Live in prod"
- Main action: follow one feature through the village by its banner
- Main path:
  1. Spec: "Saved carts" (#460) arrives as a banner with a spec scroll, split into 4 subtask crates (#461 to #464).
  2. Follow: the tech lead clicks the banner. A trail links it to its subtasks; a strip reads Spec, Build 0/4, Dev 0/4, UAT 0/4, Prod 0/4.
  3. Build: workers claim the subtasks; each lands on the canal; Build counts to 4/4.
  4. Promote: subtasks pass the dev lock, the UAT lock and the castle (prod); each stage counts n/4 and is reached only at 4/4. How the crates travel depends on the project's promotion mode:
     - **Per subtask** (default; how branch-ref projects promote today): each subtask's crate moves on its own, so the feature can be partly in a stage, e.g. "UAT 2/4", with the trail still reaching the crates behind.
     - **Whole feature** (projects that release features in batches): crates wait at the quay until all have landed, then travel together as one barge.
  5. Live: prod runs code containing all 4; the banner reads "Live in prod". It stays a while, then fades.
- Alternate state A (error, Stuck): #463 is blocked (needs design). Red pennant on the banner, trail to the worker waving "!", strip reads "Build 3/4: #463 needs design".
- Alternate state B (loading, Can't tell): no Deployment and no branch ref for UAT. The UAT lock is fogged with "?"; strip reads "UAT: unknown", not "not live".
- Several features at once: every feature in flight has its own banner; following one draws only its trail and strip.

##### Surface
- Surface: desktop web
- Viewport: 1440×900
- Screens: village with feature banners; feature followed (trail and strip); partly promoted (per subtask); promoted as one barge (whole feature); stuck; can't tell; live

##### Look and feel
- Follow the product's established patterns:
  - pixel-art village canvas, medieval or modern theme;
  - orange "!" and a waving arm when an agent needs a person;
  - canal with one lock per environment, castle last;
  - counters strip above the canvas; dialog with link buttons.
- Repository references read (pinned e4422c5): src/web/village.html, docs/village.md.
- Public UI references: https://github.com/maxjendrall/herdr
- Deliberate departures: feature banners and trail (the product has no feature object, CON-007); a per-feature strip with a count per stage; a fogged lock for unknown; a barge for whole-feature promotion.

##### Data
- All names and records are synthetic (Larkspur Labs, repository `shop`). Placeholder imagery: the village's own pixel art.
- Example records: #460 Saved carts with #461 cart table, #462 save on logout, #463 restore banner, #464 tests; a second feature #470 Gift cards with 3 subtasks (one closed as not planned) to show several banners and a dropped subtask; worker-1 to worker-4; environments dev, uat, prod; release 3f9c2a1.

##### Out of scope
- Attention and answering, the bug-to-fix loop, approving or promoting, launching swarms, spec authoring, feature flags, billing, third-party keys.

##### Output contract
- One standalone file at `outputs/prototype/index.html`
- Opens from the file system with no server, install, or build step
- No external network requests (no CDN scripts, fonts, or images)
- Main path clickable end to end in both promotion modes; both alternate states reachable
- Keyboard: Tab reaches every control; Enter/Space activates

</details>

## 6. Repository UI references
| What | Path at pinned commit | Used for |
|---|---|---|
| Village layout: lots, quay, canal, locks, castle | src/web/village.html `layout()` (Verified) | where the banner, trail and barge go |
| Canal locks and their crates per stop | src/web/village.html `paint()`, the stops loop (Verified) | dev and UAT locks, castle as prod |
| Counters strip | src/web/village.html `renderFlow()` (Verified) | the per-feature strip |
| Waving and "!" for a waiting worker | src/web/village.html `bubble()`, wave poses (Verified) | the Stuck state |
| Dialog with link buttons | src/web/village.html `show()`, `links()` (Verified) | the feature's detail |
| Worlds: dev village with its platform | src/web/village.html `drawWorld()` (Verified) | prod running the release |
