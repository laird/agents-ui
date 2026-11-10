#!/usr/bin/env bash
# Tests for install.sh --screensaver, in a throwaway HOME with stand-ins for
# the omarchy commands: what it writes, what it changes in shell.json, and
# that --uninstall puts the idle settings back. Also the launcher's --dry-run.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/test_install.sh
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALL="$ROOT/install.sh"
LAUNCHER="$ROOT/agents-village-screensaver/village-screensaver"
FAILS=0
fail() { echo "FAIL: $*"; FAILS=$((FAILS + 1)); }
ok() { echo "ok: $*"; }

sandbox() {
  T=$(mktemp -d)
  export HOME="$T/home"
  mkdir -p "$HOME/.config/omarchy" "$T/bin"
  # Stand-ins that record what was asked of them.
  for c in omarchy omarchy-shell; do
    printf '#!/bin/sh\necho "%s $*" >> "%s/calls"\n' "$c" "$T" > "$T/bin/$c"; chmod +x "$T/bin/$c"
  done
  export PATH="$T/bin:$ORIG_PATH"
}
ORIG_PATH=$PATH
ORIG_HOME=$HOME

# --- install, then uninstall, with an idle section of the user's own
sandbox
cat > "$HOME/.config/omarchy/shell.json" <<'JSON'
{ "bar": { "position": "top" }, "idle": { "screensaver": 120, "lock": 600 } }
JSON
out=$(bash "$INSTALL" --screensaver --never-sleep --server http://box:7890/ --cycle 20 --dim 23-7 2>&1) || fail "install exited $?: $out"
cfg="$HOME/.config/agents-ui/screensaver.json"
[ "$(jq -r .server "$cfg")" = "http://box:7890" ] && ok "server written" || fail "server: $(cat "$cfg")"
[ "$(jq -r .cycle "$cfg")" = 20 ] && [ "$(jq -r .dim "$cfg")" = 23-7 ] && ok "cycle and dim written" || fail "cycle/dim: $(cat "$cfg")"
[ "$(jq -r .timeout "$cfg")" = 120 ] && ok "takes Omarchy's screensaver timeout" || fail "timeout: $(cat "$cfg")"
s="$HOME/.config/omarchy/shell.json"
[ "$(jq -c .idle "$s")" = '{"screensaver":0,"lock":0}' ] && ok "never-sleep: screensaver and lock off" || fail "idle: $(jq -c .idle "$s")"
[ "$(jq -r .bar.position "$s")" = top ] && ok "rest of shell.json kept" || fail "bar lost"
[ -L "$HOME/.config/omarchy/plugins/agents-village-screensaver" ] && ok "plugin linked" || fail "plugin not linked"
grep -q "omarchy plugin enable agents-ui.village-screensaver" "$T/calls" && ok "service enabled" || fail "not enabled: $(cat "$T/calls")"
grep -q "will no longer lock itself" <<<"$out" && ok "says plainly it will not lock" || fail "no lock warning: $out"

# Installing again keeps the first backup: uninstall still restores the original.
bash "$INSTALL" --screensaver >/dev/null 2>&1 || fail "second install"
[ "$(jq -r .timeout "$cfg")" = 120 ] && ok "reinstall keeps the original timeout" || fail "reinstall timeout: $(cat "$cfg")"
jq '.bar.position = "bottom"' "$s" > "$s.tmp" && mv "$s.tmp" "$s"
bash "$INSTALL" --screensaver --uninstall >/dev/null 2>&1 || fail "uninstall exited"
[ "$(jq -c .idle "$s")" = '{"screensaver":120,"lock":600}' ] && ok "uninstall restores idle" || fail "restored idle: $(jq -c .idle "$s")"
[ "$(jq -r .bar.position "$s")" = bottom ] && ok "uninstall keeps later changes" || fail "later change lost"
[ ! -e "$cfg" ] && [ ! -e "$HOME/.config/agents-ui/shell.json.before-screensaver" ] && ok "settings and backup removed" || fail "left files behind"
[ ! -e "$HOME/.config/omarchy/plugins/agents-village-screensaver" ] && ok "plugin removed" || fail "plugin left"
grep -q "omarchy plugin disable agents-ui.village-screensaver" "$T/calls" && ok "service disabled" || fail "not disabled"
rm -rf "$T"

# --- without --never-sleep the lock stays; no idle section at all is restored as none
sandbox
echo '{ "bar": {} }' > "$HOME/.config/omarchy/shell.json"
out=$(bash "$INSTALL" --screensaver 2>&1) || fail "install exited: $out"
s="$HOME/.config/omarchy/shell.json"
[ "$(jq -c .idle "$s")" = '{"screensaver":0}' ] && ok "lock left to Omarchy" || fail "idle: $(jq -c .idle "$s")"
[ "$(jq -r .timeout "$HOME/.config/agents-ui/screensaver.json")" = 150 ] && ok "default timeout 150" || fail "timeout"
grep -q "still locks" <<<"$out" && ok "says the lock stays" || fail "no lock note: $out"
bash "$INSTALL" --screensaver --uninstall >/dev/null 2>&1
[ "$(jq -c .idle "$s")" = null ] && ok "no idle section restored as none" || fail "idle: $(jq -c .idle "$s")"
rm -rf "$T"

# --- a dry run changes nothing
sandbox
echo '{ "idle": { "screensaver": 150, "lock": 300 } }' > "$HOME/.config/omarchy/shell.json"
out=$(bash "$INSTALL" --screensaver --never-sleep --dry-run 2>&1) || fail "dry run exited: $out"
[ "$(jq -c .idle "$HOME/.config/omarchy/shell.json")" = '{"screensaver":150,"lock":300}' ] && [ ! -e "$HOME/.config/agents-ui" ] && [ ! -e "$T/calls" ] \
  && ok "dry run changes nothing" || fail "dry run changed things"
grep -q "would: write ~/.config/agents-ui/screensaver.json" <<<"$out" && grep -q "idle.lock to 0" <<<"$out" && ok "dry run says what it would do" || fail "dry run: $out"
rm -rf "$T"

# --- --host: commands reach `omarchy` in a login shell (#403)
# A remote host as ssh sees it: each command runs in a clean environment
# (env -i), as sshd's non-login shell does. Only a login shell (bash -l) gets
# OMARCHY_PATH and Omarchy's PATH, as /etc/profile.d/omarchy.sh gives it; the
# stand-in omarchy fails without it, like the real one. NO_PROFILE=1 is a host
# whose login shell does not set it either.
remote_sandbox() {
  sandbox
  REMOTE="$T/remote"; mkdir -p "$REMOTE/home/.config/omarchy" "$T/rbin" "$T/omarchy-bin"
  for c in omarchy omarchy-shell; do
    cat > "$T/omarchy-bin/$c" <<EOF
#!/bin/sh
[ -n "\${OMARCHY_PATH:-}" ] || { echo "OMARCHY_PATH is not set" >&2; exit 1; }
echo "$c \$* (OMARCHY_PATH=\$OMARCHY_PATH)" >> "$T/remote-calls"
EOF
    chmod +x "$T/omarchy-bin/$c"
  done
  cat > "$T/rbin/bash" <<EOF
#!/bin/sh
if [ "\$1" = -lc ]; then
  shift
  [ -n "\${NO_PROFILE:-}" ] || { export OMARCHY_PATH=/fake/omarchy; PATH="$T/omarchy-bin:\$PATH"; }
  exec "$(command -v bash)" -c "\$@"
fi
exec "$(command -v bash)" "\$@"
EOF
  chmod +x "$T/rbin/bash"
  cat > "$T/bin/ssh" <<EOF
#!/bin/sh
# ssh HOST COMMAND: COMMAND in a clean environment on the remote
shift
cd "$REMOTE/home" && exec env -i HOME="$REMOTE/home" PATH="$T/rbin:/usr/bin:/bin" \${NO_PROFILE:+NO_PROFILE=1} sh -c "\$1"
EOF
  cat > "$T/bin/scp" <<EOF
#!/bin/sh
# scp -q -p FILES... HOST:DIR (DIR relative to the remote home)
shift 2
for last; do :; done
n=\$#; i=0
for f; do i=\$((i+1)); if [ \$i -lt \$n ]; then cp -p "\$f" "$REMOTE/home/\${last#*:}/"; fi; done
EOF
  # The local omarchy stand-ins must not be what answers.
  rm -f "$T/bin/omarchy" "$T/bin/omarchy-shell"
  chmod +x "$T/bin/ssh" "$T/bin/scp"
  rs="$REMOTE/home/.config/omarchy/shell.json"
  echo '{ "idle": { "screensaver": 90 } }' > "$rs"
}

remote_sandbox
out=$(bash "$INSTALL" --screensaver --host box --dry-run 2>&1) || fail "remote dry run exited: $out"
grep -qF "would run on box: bash -lc 'sh -c '\''omarchy plugin enable agents-ui.village-screensaver'\'''" <<<"$out" \
  && ok "--dry-run --host shows the login-shell form" || fail "dry run: $out"
# The harness itself: without a login shell, omarchy fails as it does over ssh.
ssh box 'omarchy plugin list' >/dev/null 2>&1 && fail "harness: omarchy ran without a login shell" || ok "harness: no OMARCHY_PATH outside a login shell"
out=$(bash "$INSTALL" --screensaver --host box 2>&1) || fail "remote install exited $?: $out"
grep -q "omarchy plugin enable agents-ui.village-screensaver (OMARCHY_PATH=/fake/omarchy)" "$T/remote-calls" 2>/dev/null \
  && ok "remote install enables the service with OMARCHY_PATH set" || fail "remote enable: $(cat "$T/remote-calls" 2>&1) / $out"
[ -f "$REMOTE/home/.config/omarchy/plugins/agents-village-screensaver/Service.qml" ] && ok "remote plugin copied" || fail "remote plugin not copied"
[ "$(jq -c .idle "$rs")" = '{"screensaver":0}' ] && ok "remote shell.json configured" || fail "remote idle: $(jq -c .idle "$rs")"
out=$(bash "$INSTALL" --screensaver --uninstall --host box 2>&1) || fail "remote uninstall exited: $out"
grep -q "omarchy plugin disable agents-ui.village-screensaver (OMARCHY_PATH=/fake/omarchy)" "$T/remote-calls" \
  && ok "remote uninstall disables the service with OMARCHY_PATH set" || fail "remote disable: $(cat "$T/remote-calls")"
[ "$(jq -c .idle "$rs")" = '{"screensaver":90}' ] && [ ! -e "$REMOTE/home/.config/omarchy/plugins/agents-village-screensaver" ] \
  && ok "remote uninstall puts things back" || fail "remote uninstall: $(jq -c .idle "$rs")"
out=$(bash "$INSTALL" --enable --host box 2>&1) || fail "remote widget install exited: $out"
grep -q "omarchy plugin enable agents-ui.village (OMARCHY_PATH=/fake/omarchy)" "$T/remote-calls" \
  && ok "remote --enable adds the widget with OMARCHY_PATH set" || fail "remote widget: $(cat "$T/remote-calls")"
rm -rf "$T"

# A host where omarchy cannot run: stop before changing anything.
remote_sandbox
out=$(NO_PROFILE=1 bash "$INSTALL" --screensaver --host box 2>&1) && fail "install went ahead without omarchy: $out" || ok "refuses when omarchy cannot run on the host"
grep -q "omarchy does not run on box: OMARCHY_PATH is not set" <<<"$out" && ok "says why, plainly" || fail "message: $out"
[ "$(jq -c .idle "$rs")" = '{"screensaver":90}' ] && [ ! -e "$REMOTE/home/.config/omarchy/plugins/agents-village-screensaver" ] && [ ! -e "$REMOTE/home/.config/agents-ui" ] \
  && ok "nothing changed on the host" || fail "half-installed: $(jq -c .idle "$rs")"
NO_PROFILE=1 bash "$INSTALL" --enable --host box >/dev/null 2>&1 && fail "widget --enable went ahead without omarchy" || ok "--enable refuses too"
rm -rf "$T"
export HOME=$ORIG_HOME PATH=$ORIG_PATH

# --- bad arguments
bash "$INSTALL" --never-sleep >/dev/null 2>&1 && fail "--never-sleep without --screensaver accepted" || ok "--never-sleep needs --screensaver"
bash "$INSTALL" --screensaver --dim 25-3 --dry-run >/dev/null 2>&1 && fail "bad --dim accepted" || ok "bad --dim refused"
bash "$INSTALL" --screensaver --cycle x --dry-run >/dev/null 2>&1 && fail "bad --cycle accepted" || ok "bad --cycle refused"

# --- the launcher's dry run: the page it opens, from the settings file
sandbox
mkdir -p "$HOME/.config/agents-ui"
echo '{ "server": "http://127.0.0.1:9", "cycle": 45, "dim": "22-6" }' > "$HOME/.config/agents-ui/screensaver.json"
out=$(bash "$LAUNCHER" --dry-run 2>&1) || fail "launcher dry run exited: $out"
# Port 9 does not answer, so it opens the local "can't reach" page with the village after #.
grep -q "opens:   file://.*/offline.html#http%3A%2F%2F127.0.0.1%3A9%2Fvillage%3Fscreensaver%3D1%26dismiss%3D1%26cycle%3D45%26dim%3D22-6" <<<"$out" \
  && ok "launcher opens the offline page for a server that does not answer" || fail "launcher: $out"
grep -q "class:   org.omarchy.screensaver" <<<"$out" && ok "launcher window is Omarchy's screensaver class" || fail "class: $out"
rm -rf "$T"
export HOME=$ORIG_HOME

[ "$FAILS" -eq 0 ] && echo "all passed" || { echo "$FAILS failed"; exit 1; }
