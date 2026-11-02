# Design brief: agents-ui · Attention and answering

## Context
- Product: agents-ui, one place to launch, watch and supervise swarms of AI coding agents (terminal UI, web dashboard, village view).
- Persona: Solo developer · Mandate: Never miss a blocked agent
- Selected opportunity: OPP-003: make the "needs you" marker trustworthy by showing the state each source reports itself, not a guess from screen text.
- Discovery evidence this flow answers: SIG-007 (silence can't tell waiting from thinking), SIG-013 (hook-based alerts miss some prompts), SIG-003 (notifications through tmux broke and were patched), SIG-009 (stale "Ready for input" alerts flood people with many sessions), CON-003 (tmux waiting is a fixed phrase list).

## The flow
- Actor: solo developer running two swarms (shop, billing)
- Job: "Tell me which agent really needs me, and why, so I can believe the marker."
- Starts when: a worker card on the dashboard turns amber with "Needs you"
- Ends when: the developer knows which source reported it, what the agent asked and since when, and opens that agent's session
- Main action: Open agent
- Main path: 1. Dashboard: worker-2 in shop reads "Needs you · herdr · 3 min". 2. Select the marker: a "Why" panel opens with four facts: source ("herdr reports blocked"), the ask ("Allow `npm publish`? (y/n)"), since (14:02), agreement ("Screen also shows a prompt"). 3. A solid "Reported" badge marks a state the source reported itself. 4. Select "Open agent": the agent's session view opens, prompt visible, ready to answer.
- Alternate state A (error): Source unreachable. herdr can't be read: the card goes grey, "State unknown · herdr unreachable since 14:05", last known state shown faded, with "Retry" and "Open agent anyway".
- Alternate state B (recovery): Guess only. No reported state: a dashed "Guessed from screen" badge, the matched line ("Allow? (y/n)" seen 40 s ago), and a note that it may be wrong.

## Surface
- Surface: desktop web · Viewport: 1440×900
- Screens: Dashboard (swarm cards), Why panel (side panel over the dashboard), Agent session view

## Look and feel
- Follow the dashboard's dark theme: background #0d1117, surface #161b22, card #21262d, border #30363d, accent #58a6ff, text #e6edf3, dim #8b949e, warning #d29922, error #f85149, radius 6px, system sans font.
- Swarm cards with a header, path line and one coloured dot per agent; amber dot = waiting; pill badges; uppercase panel headers.
- Repository references read (paths, pinned commit 0019fe8): src/web/ui.html lines 9 to 22 (tokens), 129 to 190 (cards, badges), 335 to 360 (panels), 455 to 467 (agent dots).
- Public UI references: https://primer.style (GitHub's dark palette the dashboard follows)
- Deliberate departures from the current UI: a source badge (Reported, solid / Guessed, dashed) and a waiting-time label on every marker; a Why side panel.

## Data
- All names, records, images, and media are synthetic. Placeholder imagery: neutral blocks or simple shapes.
- Example records: Larkspur Labs; swarm "shop" (manager, worker-1 working #41, worker-2 waiting via herdr, worker-3 idle); swarm "billing" (worker-1 guessed waiting from screen, worker-2 working); Temporal app "support" (run refund-8812 waiting_for_human 6 min).

## Out of scope
- Answering from the alert, notifications, the single waiting list (OPP-001, OPP-002), launching swarms, the village view, settings.

## Output contract
- One standalone file at `outputs/prototype/index.html`
- Opens from the file system with no server, install, or build step
- No external network requests (no CDN scripts, fonts, or images)
- Main path clickable end to end; both alternate states reachable
- Keyboard: Tab reaches every control; Enter/Space activates
