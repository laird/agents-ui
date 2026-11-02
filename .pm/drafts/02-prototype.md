# Prototype notes: agents-ui · Attention and answering

| Field | Value |
|---|---|
| Status | draft |
| Based on | outputs/01-discovery.md rev 1 · OPP-003 |
| Path used | HTML fallback (Claude Design available, but its export would land outside this cloud session; PM chose HTML) |
| File | outputs/prototype/index.html |
| Surface · viewport | desktop web · 1440×900 |
| Run summary | (written at close) |

## 1. Actor, job, and user story
- Actor: solo developer · Job: believe the "needs you" marker
- As a solo developer, when an agent is marked as needing me, I want to see which source reported it, what it asked and since when, so that I trust the marker and open the right agent.

## 2. Approved flow and states
- Starts when / ends when: a worker card turns amber with "Needs you" / the developer knows why and opens the agent's session
- Main path: 1. Dashboard card "Needs you · herdr · 3 min". 2. Why panel: source, the ask, since, agreement. 3. Solid "Reported" badge. 4. Open agent.
- Alternate state A (error): Source unreachable: grey card, "State unknown · herdr unreachable since 14:05", last known state faded, Retry and Open agent anyway.
- Alternate state B (recovery): Guess only: dashed "Guessed from screen" badge with the matched line and its age.

## 3. Linked discovery evidence
| Flow element | Evidence or constraint |
|---|---|
| Source named on every marker (Reported vs Guessed) | SIG-007, SIG-013, CON-003 |
| Waiting time on the marker | SIG-009 (stale "Ready for input" alerts) |
| Agreement line (other sources agree) | SIG-006, SIG-011 (herdr detection works for some, guessing for others) |
| Source unreachable state | CON-001 (Temporal and herdr reach alerts differently), SIG-003 |
| Guess only state | CON-003 (tmux detection is a fixed phrase list) |

## 4. Scope boundaries
- In: the dashboard marker, the Why panel, opening the agent, the two states.
- Out: answering from the alert, notifications, the single waiting list (OPP-001), alert-and-answer (OPP-002), the village view, settings.

## 5. Design brief
<details>
<summary>Design brief as used to build the prototype</summary>

#### Design brief: agents-ui · Attention and answering

##### Context
- Product: agents-ui, one place to launch, watch and supervise swarms of AI coding agents (terminal UI, web dashboard, village view).
- Persona: Solo developer · Mandate: Never miss a blocked agent
- Selected opportunity: OPP-003: make the "needs you" marker trustworthy by showing the state each source reports itself, not a guess from screen text.
- Discovery evidence this flow answers: SIG-007 (silence can't tell waiting from thinking), SIG-013 (hook-based alerts miss some prompts), SIG-003 (notifications through tmux broke and were patched), SIG-009 (stale "Ready for input" alerts flood people with many sessions), CON-003 (tmux waiting is a fixed phrase list).

##### The flow
- Actor: solo developer running two swarms (shop, billing)
- Job: "Tell me which agent really needs me, and why, so I can believe the marker."
- Starts when: a worker card on the dashboard turns amber with "Needs you"
- Ends when: the developer knows which source reported it, what the agent asked and since when, and opens that agent's session
- Main action: Open agent
- Main path: 1. Dashboard: worker-2 in shop reads "Needs you · herdr · 3 min". 2. Select the marker: a "Why" panel opens with four facts: source ("herdr reports blocked"), the ask ("Allow `npm publish`? (y/n)"), since (14:02), agreement ("Screen also shows a prompt"). 3. A solid "Reported" badge marks a state the source reported itself. 4. Select "Open agent": the agent's session view opens, prompt visible, ready to answer.
- Alternate state A (error): Source unreachable. herdr can't be read: the card goes grey, "State unknown · herdr unreachable since 14:05", last known state shown faded, with "Retry" and "Open agent anyway".
- Alternate state B (recovery): Guess only. No reported state: a dashed "Guessed from screen" badge, the matched line ("Allow? (y/n)" seen 40 s ago), and a note that it may be wrong.

##### Surface
- Surface: desktop web · Viewport: 1440×900
- Screens: Dashboard (swarm cards), Why panel (side panel over the dashboard), Agent session view

##### Look and feel
- Follow the dashboard's dark theme: background #0d1117, surface #161b22, card #21262d, border #30363d, accent #58a6ff, text #e6edf3, dim #8b949e, warning #d29922, error #f85149, radius 6px, system sans font.
- Swarm cards with a header, path line and one coloured dot per agent; amber dot = waiting; pill badges; uppercase panel headers.
- Repository references read (paths, pinned commit 0019fe8): src/web/ui.html lines 9 to 22 (tokens), 129 to 190 (cards, badges), 335 to 360 (panels), 455 to 467 (agent dots).
- Public UI references: https://primer.style (GitHub's dark palette the dashboard follows)
- Deliberate departures from the current UI: a source badge (Reported, solid / Guessed, dashed) and a waiting-time label on every marker; a Why side panel.

##### Data
- All names, records, images, and media are synthetic. Placeholder imagery: neutral blocks or simple shapes.
- Example records: Larkspur Labs; swarm "shop" (manager, worker-1 working #41, worker-2 waiting via herdr, worker-3 idle); swarm "billing" (worker-1 guessed waiting from screen, worker-2 working); Temporal app "support" (run refund-8812 waiting_for_human 6 min).

##### Out of scope
- Answering from the alert, notifications, the single waiting list (OPP-001, OPP-002), launching swarms, the village view, settings.

##### Output contract
- One standalone file at `outputs/prototype/index.html`
- Opens from the file system with no server, install, or build step
- No external network requests (no CDN scripts, fonts, or images)
- Main path clickable end to end; both alternate states reachable
- Keyboard: Tab reaches every control; Enter/Space activates

</details>

## 6. Repository UI references
| What | Path at pinned commit | Used for |
|---|---|---|
| Colour, radius and shadow tokens | src/web/ui.html lines 9 to 22 (Verified) | page palette |
| Swarm cards and pill badges | src/web/ui.html lines 129 to 190 (Verified) | dashboard cards, badges |
| Panels with uppercase headers | src/web/ui.html lines 335 to 360 (Verified) | Why panel |
| Agent dots (amber = waiting) | src/web/ui.html lines 455 to 467 (Verified) | agent markers |

## 7. Where the prototype differs from the product UI
- Markers carry their source and waiting time ("Needs you · herdr · 3 min"); the product shows an amber dot and "needs input" with no source (deliberate).
- A dashed "Guessed from screen" marker and badge for states read from screen text (deliberate).
- A "Why" side panel listing source, ask, since and agreement; the product has no such panel (deliberate).
- A Temporal app card on the dashboard; the product shows Temporal only in the village view (deliberate, per the PM's sources).
- A banner and greyed card when a source cannot be read; the product shows nothing (deliberate).
- Swarm cards are simplified: no stats row, no issue panel, no stop or resume buttons (limitation).

## 8. Walkthrough findings
This is a design critique, not customer validation.
| Area | Claude's check | PM's click-through | Severity |
|---|---|---|---|
| Labels | Pass: markers read "Needs you · source · time"; buttons say Open agent, Close, Retry, Open agent anyway | | |
| Navigation | Pass: every panel has Close; the session view has Back to swarms; demo bar reaches both states | | |
| Keyboard (Tab order, Enter and Space, focus moves to the new screen's heading) | Pass: Tab reaches the first marker, Enter opens the panel, focus lands on its heading (checked in a headless browser) | | |
| State clarity (each state visibly different, each offers a next step, error uses role="alert") | Found and fixed before the walkthrough: long "last known" text spilled into the next card, and markers wrapped behind the panel. Now pass: unreachable banner has role="alert" and Retry; guess state has a dashed badge and a next step | | minor (fixed) |
| Contrast (body text and primary buttons at least 4.5 to 1) | Finding: faded "last known" text in the unreachable state is under 4.5:1 by design (opacity .55); everything else passes | | minor |
| Scope (nothing from another focus area) | Pass: no answering, notifications or waiting list | | |

## 9. Revision made
(Filled after the walkthrough.)

## 10. Remaining limitations
(Filled at close.)

## 11. Decision record
| Check-in | Claude recommended | PM decided | Reason / trade-off |
|---|---|---|---|
| Flow check | Approve the flow with Source unreachable and Guess only; cut dismiss-a-false-alarm | Approve | matched the recommendation |
| Prototype check | | | |
