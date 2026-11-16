# Cycling between swarms (#382)

With several swarms the screensaver cycles through each world, then each
village that is not part of a world, `cycle` seconds on each (default 30).
`cycle=0` stays on one. A swarm named in the hash (`#shop&screensaver`) is
pinned: it stays unless `cycle` is given too.

Option: `--cycle S` (`options/cycle.sh`).
