# Prototype notes: agents-ui · Attention and answering

| Field | Value |
|---|---|
| Status | draft |
| Based on | outputs/01-discovery.md rev 1 · OPP-003 |
| Path used | HTML fallback, on the product's own village renderer (Claude Design available, but its export would land outside this cloud session; PM chose HTML) |
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
- Built on the product's own village renderer (`src/web/village.html`, the Worktree Hollow page), fed with synthetic data instead of `/api/village`; regenerate with `python3 .pm/prototype-build.py outputs/prototype/index.html` (deliberate).
- A sign over each house that needs you names who reported it and how long it has waited: solid gold for reported ("herdr · 3 min", "Temporal · 6 min"), dashed for guessed ("screen guess · 40 s"); the product shows an orange "!" with no source (deliberate).
- A guessed wait also gets a dashed rim on its "!" bubble (deliberate).
- Selecting a sign or a house opens a "Why" dialog: source, what it asked, since when, other sources, with Open agent; the product's dialog shows only issue and status (deliberate).
- An unreachable source puts that village's houses in fog with "?" signs and a banner; the product shows nothing (deliberate).
- Open agent shows a static session excerpt; answering is out of scope (limitation).
- The demo bar switches scenarios; the picker also shows a Temporal app, "temporal:support", drawn as a runtime village (deliberate).

## 8. Walkthrough findings
This is a design critique, not customer validation. Revision 0 was a conventional dashboard; the PM's click-through led to revision 1, the village.
| Area | Claude's check | PM's click-through | Severity |
|---|---|---|---|
| Labels | Pass (rev 1): signs read "source · wait"; dialog buttons say Open agent, Close, Back to the village | "The data model looks fine, but it needs to be visualized as a village, not a traditional app gui." | major (rev 0), fixed in rev 1 |
| Navigation | Pass: dialog closes with ×, Close or Esc; changing village closes it (fixed in rev 1: a stale dialog stayed open) | as above | minor (fixed) |
| Keyboard (Tab order, Enter and Space, focus moves to the new screen's heading) | Pass: the canvas cannot take focus, so each waiting house has a real button sign; Tab reaches it, Enter opens the dialog, focus lands on its title | as above | |
| State clarity (each state visibly different, each offers a next step, error uses role="alert") | Pass: unreachable = fog, "?" signs and a role="alert" banner; guess = dashed sign, dashed bubble rim and "this may be wrong"; each offers Open agent | as above | |
| Contrast (body text and primary buttons at least 4.5 to 1) | Pass: gold signs with dark text, dialog text on the menu blue; faded fog is deliberate | as above | |
| Scope (nothing from another focus area) | Pass: no answering, notifications or waiting list | as above | |

## 9. Revision made
- Change: rebuilt the prototype as the 16-bit village instead of a conventional dashboard: source-and-wait signs over waiting houses (solid reported, dashed guessed), a Why dialog with Open agent, fog for an unreachable source.
- Tied to: the PM's walkthrough finding (section 8, Labels) and SIG-007, SIG-013, CON-003 (show whether a wait is reported or guessed).
- Before and after: a three-card dashboard with pill markers → the village with signs over the houses that need you.

## 10. Remaining limitations
- Static data per scenario; waits do not tick and agents do not move between states.
- The real village page's picker does not follow a link change (found here; fixed only in the prototype).
- Answering, freshness ("checked 5 s ago") and the single waiting list are not shown.
- Only the medieval style was checked; the modern style draws the same signs.

## 11. Decision record
| Check-in | Claude recommended | PM decided | Reason / trade-off |
|---|---|---|---|
| Flow check | Approve the flow with Source unreachable and Guess only; cut dismiss-a-false-alarm | Approve | matched the recommendation |
| Prototype check | | | |
