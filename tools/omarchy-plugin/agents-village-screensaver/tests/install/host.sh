#!/usr/bin/env bash
# install.sh --host: commands reach `omarchy` in a login shell (#403); it refuses on a host where omarchy cannot run.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/host.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

# --- --host: commands reach `omarchy` in a login shell (#403)
# A remote host as ssh sees it: each command runs in a clean environment
# (env -i), as sshd's non-login shell does. Only a login shell (bash -l) gets
# OMARCHY_PATH, as /etc/profile.d/omarchy.sh gives it; the stand-in omarchy
# is installed but fails without it, like the real one. NO_PROFILE=1 is a host
# whose login shell does not set it either; NO_OMARCHY=1 is a host without
# Omarchy at all.
#
# The fake host never sees this machine's own omarchy (#408): its system
# commands come from $T/sysbin, every command in /usr/bin and /bin except
# omarchy*. Otherwise the result would depend on whether the machine running
# the tests has Omarchy (a developer machine might; CI does not).

remote_sandbox
out=$(bash "$INSTALL" --screensaver --host box --dry-run 2>&1) || fail "remote dry run exited: $out"
grep -qF "would run on box: bash -lc 'sh -c '\''omarchy plugin enable agents-ui.village-screensaver'\'''" <<<"$out" \
  && ok "--dry-run --host shows the login-shell form" || fail "dry run: $out"
# The harness itself: the host's omarchy is the stand-in, never this machine's (#408).
[ "$(ssh box 'command -v omarchy')" = "$T/omarchy-bin/omarchy" ] && ok "harness: the host's omarchy is the stand-in" || fail "harness: omarchy is $(ssh box 'command -v omarchy')"
# The harness itself: without a login shell, omarchy fails as it does over ssh.
ssh box 'omarchy plugin list' >/dev/null 2>&1 && fail "harness: omarchy ran without a login shell" || ok "harness: no OMARCHY_PATH outside a login shell"
out=$(bash "$INSTALL" --screensaver --host box 2>&1) || fail "remote install exited $?: $out"
grep -q "omarchy plugin enable agents-ui.village-screensaver (OMARCHY_PATH=/fake/omarchy)" "$T/remote-calls" 2>/dev/null \
  && ok "remote install enables the service with OMARCHY_PATH set" || fail "remote enable: $(cat "$T/remote-calls" 2>&1) / $out"
[ -f "$REMOTE/home/.config/omarchy/plugins/agents-village-screensaver/Service.qml" ] && ok "remote plugin copied" || fail "remote plugin not copied"
rp="$REMOTE/home/.config/omarchy/plugins/agents-village-screensaver"
[ -f "$rp/options.sh" ] && [ -f "$rp/options/cycle.sh" ] && ok "remote copy has the page options" || fail "remote options: $(ls -R "$rp" 2>&1)"
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
grep -q "nothing was changed" <<<"$out" && ok "says nothing was changed" || fail "message: $out"
[ "$(jq -c .idle "$rs")" = '{"screensaver":90}' ] && [ ! -e "$REMOTE/home/.config/omarchy/plugins/agents-village-screensaver" ] && [ ! -e "$REMOTE/home/.config/agents-ui" ] \
  && ok "nothing changed on the host" || fail "half-installed: $(jq -c .idle "$rs")"
NO_PROFILE=1 bash "$INSTALL" --enable --host box >/dev/null 2>&1 && fail "widget --enable went ahead without omarchy" || ok "--enable refuses too"
rm -rf "$T"

# A host without Omarchy installed at all: the same refusal, for that reason.
remote_sandbox
NO_OMARCHY=1 ssh box 'command -v omarchy' >/dev/null 2>&1 && fail "harness: the host without Omarchy has an omarchy" || ok "harness: no omarchy on the host"
out=$(NO_OMARCHY=1 bash "$INSTALL" --screensaver --host box 2>&1) && fail "install went ahead without omarchy installed: $out" || ok "refuses when omarchy is not installed"
grep -q "omarchy does not run on box: omarchy: command not found" <<<"$out" && ok "says omarchy is not there" || fail "message: $out"
grep -q "nothing was changed" <<<"$out" && ok "says nothing was changed" || fail "message: $out"
[ "$(jq -c .idle "$rs")" = '{"screensaver":90}' ] && [ ! -e "$REMOTE/home/.config/omarchy/plugins/agents-village-screensaver" ] \
  && ok "nothing changed on the host without Omarchy" || fail "half-installed: $(jq -c .idle "$rs")"
rm -rf "$T"
export HOME=$ORIG_HOME PATH=$ORIG_PATH

finish
