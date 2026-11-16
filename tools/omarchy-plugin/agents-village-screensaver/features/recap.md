# Since you last looked (#479)

After a quiet spell the screensaver opens with a "since you last looked" card
over the dimmed map for 20 seconds: issues landed and closed, pull requests
merged, deploys (with their versions), what bounced at the gate, and what is
waiting on you, in red. A spell with nothing in it says "Nothing happened".

The spell runs from the last unlock of the machine: the service watches
Omarchy's lock (every 5 seconds) and keeps the time of each unlock in
`~/.local/state/agents-village-screensaver/last-unlock`. The card only shows
when that was at least `recap` minutes ago (default 30, 0 turns it off); with
no unlock seen yet there is none. Any input closes it.

Option: `--recap M` (`options/recap.sh`, which also turns it into the page's
`since=`).
