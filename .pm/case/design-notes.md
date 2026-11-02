# agents-ui: design notes

## Established visual patterns

- **Web dashboard** (`src/web/ui.html`): dark theme, GitHub-like palette, a header, breadcrumb
  navigation, cards per swarm, one coloured dot per agent. Amber means waiting for input,
  blue working, grey idle, red error. Dense, developer-oriented. **Verified** (CSS custom
  properties at lines 9 to 22, `.dot-waiting` at line 466).
- **TUI**: ratatui terminal screens with a status bar of key hints; yellow is the attention
  colour. **Verified** (`src/ui/theme.rs`).
- **Village** (`src/web/village.html`): pixel-art canvas, medieval or modern style; an open
  door and orange "!" mean an agent is waiting. **Verified** (`docs/village.md`).
- Notifications: none found. There is no browser notification, sound, or push in the lane's
  files. **Inferred** (from a search of the entry points; to confirm).

## Where the design system lives

- `src/web/ui.html` lines 9 to 22: colour, radius, and shadow tokens. **Verified**.
- `src/ui/theme.rs`: TUI styles. **Verified**.
- No shared component library; plain HTML, CSS, and JavaScript embedded in the binary.
  **Verified** (no front-end manifest in the repository).

## Public UI references

- herdr's sidebar of agent states: https://github.com/maxjendrall/herdr
- GitHub's dark theme (the dashboard follows its palette): https://primer.style

## Synthetic data guidance

One fictional organisation (for example "Larkspur Labs"), repositories named like `shop` and
`billing`, workers `worker-1` to `worker-4`, issue numbers in the 400s, prompts copied in
spirit, not from real sessions.

## Prototype scope reminders

One HTML file, desktop web at 1440×900, the main path plus two states (nothing waiting; several
waiting at once). Do not copy source files from the repository.
