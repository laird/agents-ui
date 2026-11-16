# Sound cues (#489)

Off unless asked for: `--sound` at install (`"sound": true` in
`screensaver.json`), or `sound=1` in the page's query. The cues mark changes,
never a state that lasts, and play at most once each in two minutes:

- a soft bell when more starts waiting on you;
- thunder when the build breaks, and birdsong when it is green again;
- a ship's horn when a deploy docks at the production (right-most) environment.

Each theme has its own voice (a medieval bell, an office chime, a sci-fi
tone), synthesized in the page; there are no sound files. They are silent in
the `dim` hours. Chromium holds audio until the first click, which a
screensaver never gets, so with sound on the launcher starts its kiosk windows
with `--autoplay-policy=no-user-gesture-required`. A plain browser tab plays
them only after you have clicked in it.

Option: `--sound` (`options/sound.sh`).
