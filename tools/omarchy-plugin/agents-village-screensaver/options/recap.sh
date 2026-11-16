# --recap M: "Since you last looked" (#479). When the machine was last
# unlocked at least M minutes ago (default 30; 0 turns it off), the page opens
# with a summary of what happened since then: `since=<unix seconds>`, never
# `recap=` itself. Service.qml watches Omarchy's lock and keeps the time of the
# last unlock in ${XDG_STATE_HOME:-~/.local/state}/agents-village-screensaver/last-unlock.
# With no unlock recorded yet, or on a forced run (someone is at the
# keyboard), there is no card.
saver_option recap --recap 30 number '^(0|[1-9][0-9]*)$' M \
  "a \"since you last looked\" card after M minutes away (default 30, 0: never)" \
  "--recap takes whole minutes"

# The launcher's query part: since=<the last unlock>, when the spell was long
# enough. Reads the launcher's FORCE and NOW.
saver_query_recap() {
  local recap=$1 since="" file
  [ "${FORCE:-false}" = true ] && return 0
  [[ $recap =~ ^[0-9]+$ ]] && [ "$recap" -gt 0 ] || return 0
  file="${XDG_STATE_HOME:-$HOME/.local/state}/agents-village-screensaver/last-unlock"
  # A unix time, as `date +%s` writes it: anything else (hand-edited, cut
  # short) is no record, rather than octal or an overflow in $(( )).
  [ -f "$file" ] && since=$(head -c 32 "$file" | tr -d '[:space:]')
  [[ $since =~ ^[1-9][0-9]{8,10}$ ]] || return 0
  ((since <= NOW && NOW - since >= recap * 60)) && echo "since=$since"
  return 0
}
