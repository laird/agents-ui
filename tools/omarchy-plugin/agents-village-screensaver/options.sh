# options.sh — the screensaver's page options, one file each in options/ (#522).
#
# Sourced by install.sh (to parse, check and save them) and by the launcher
# (to read them back and build the page's URL). Each options/<key>.sh calls
#
#   saver_option <key> <flag> <default> <type> <check> <hint> <help> <error>
#
#   key      its name in screensaver.json, and in the page's query
#   flag     install.sh's flag for it (--cycle)
#   default  its value when the flag is not given
#   type     number or string, as screensaver.json holds it
#   check    an extended regex the value must match ([[ =~ ]])
#   hint     what the flag takes, for --help (S, H-H)
#   help     one line for install.sh --help
#   error    what install.sh says when the value does not match
#
# The launcher puts `key=value` in the page's query when the value is not
# empty. An option file that wants something else defines
# `saver_query_<key> <value>`, which prints the query part (or nothing).
#
# So a new option is one new file in options/, and its tests one new file in
# tests/install/. Nothing here, in install.sh or in the launcher changes.
# bash 3.2 (macOS): indexed arrays only.

SAVER_KEYS=() SAVER_FLAGS=() SAVER_DEFAULTS=() SAVER_TYPES=() SAVER_CHECKS=() SAVER_HINTS=() SAVER_HELPS=() SAVER_ERRORS=()

saver_option() {
  SAVER_KEYS+=("$1"); SAVER_FLAGS+=("$2"); SAVER_DEFAULTS+=("$3"); SAVER_TYPES+=("$4")
  SAVER_CHECKS+=("$5"); SAVER_HINTS+=("$6"); SAVER_HELPS+=("$7"); SAVER_ERRORS+=("$8")
}

# Load every options/*.sh beside this file, in name order.
saver_load_options() {
  local dir f
  dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/options"
  for f in "$dir"/*.sh; do
    [ -f "$f" ] || continue
    # shellcheck disable=SC1090
    . "$f"
  done
}

# The index of the option with this flag, or nothing.
saver_option_index() {  # saver_option_index <flag>
  local i=0
  while [ "$i" -lt "${#SAVER_FLAGS[@]}" ]; do
    [ "${SAVER_FLAGS[$i]}" = "$1" ] && { echo "$i"; return 0; }
    i=$((i + 1))
  done
  return 1
}

# The page's query part for one option's value.
saver_query() {  # saver_query <key> <value>
  if declare -F "saver_query_$1" >/dev/null; then "saver_query_$1" "$2"
  elif [ -n "$2" ]; then echo "$1=$2"
  fi
}
