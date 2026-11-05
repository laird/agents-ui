# Design brief: agents-ui · A feature from spec to production

## Context
- Product: agents-ui watches swarms of AI coding agents, drawn as a 16-bit village.
- Persona: Tech lead · Mandate: Know when a feature is live
- Selected opportunity: OPP-005: "Live" per feature and per environment: know, without asking anyone, that a whole feature has reached production.
- Discovery evidence this flow answers: SIG-023 ("what's in production" is hard across environments), SIG-033 ("done" means many things), SIG-022 (environment state lives nowhere), SIG-030 (GitHub records the sha per environment), CON-007, CON-009.

## The flow
- Actor: the tech lead running the swarm
- Job: "Is feature X live in production yet, and if not, where is it?"
- Starts when: a feature's spec arrives as a parent issue
- Ends when: the whole feature is marked "Live in prod"
- Main action: follow one feature through the village by its banner
- Main path:
  1. Spec: "Saved carts" (#460) arrives as a banner with a spec scroll, split into 4 subtask crates (#461 to #464).
  2. Follow: the tech lead clicks the banner. A trail links it to its subtasks; a strip reads Spec, Build 0/4, Dev, UAT, Prod.
  3. Build: workers claim the subtasks; each lands on the canal; Build counts to 4/4.
  4. Promote: all 4 landed and green, the feature moves as one barge through the dev lock, the UAT lock, then the castle (prod).
  5. Live: prod runs a release containing all 4; the banner reads "Live in prod".
- Alternate state A (error, Stuck): #463 is blocked (needs design). Red pennant on the banner, trail to the worker waving "!", strip reads "Build 3/4: #463 needs design".
- Alternate state B (loading, Can't tell): no deployment record for UAT. The UAT lock is fogged with "?"; strip reads "UAT: unknown", not "not live".

## Surface
- Surface: desktop web
- Viewport: 1440×900
- Screens: village with the feature banner; feature followed (trail and strip); stuck; can't tell; live

## Look and feel
- Follow the product's established patterns:
  - pixel-art village canvas, medieval or modern theme;
  - orange "!" and a waving arm when an agent needs a person;
  - canal with one lock per environment, castle last;
  - counters strip above the canvas; dialog with link buttons.
- Repository references read (pinned e4422c5): src/web/village.html, docs/village.md.
- Public UI references: https://github.com/maxjendrall/herdr
- Deliberate departures: a feature banner and trail (the product has no feature object, CON-007); a per-feature strip; a fogged lock for unknown.

## Data
- All names and records are synthetic (Larkspur Labs, repository `shop`). Placeholder imagery: the village's own pixel art.
- Example records: #460 Saved carts with #461 cart table, #462 save on logout, #463 restore banner, #464 tests; worker-1 to worker-4; environments dev, uat, prod; release 3f9c2a1.

## Out of scope
- Attention and answering, the bug-to-fix loop, approving or promoting, launching swarms, spec authoring, billing, third-party keys.

## Output contract
- One standalone file at `outputs/prototype/index.html`
- Opens from the file system with no server, install, or build step
- No external network requests (no CDN scripts, fonts, or images)
- Main path clickable end to end; both alternate states reachable
- Keyboard: Tab reaches every control; Enter/Space activates
