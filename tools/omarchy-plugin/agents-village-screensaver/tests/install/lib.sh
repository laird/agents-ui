# lib.sh — what every install test shares (#522): a throwaway HOME with
# stand-ins for the omarchy commands, a fake remote host, ok/fail, and the
# summary. Each tests/install/<feature>.sh sources it and ends with `finish`.
# bash 3.2 and BSD tools (CI runs these on macOS too).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
INSTALL="$ROOT/install.sh"
LAUNCHER="$ROOT/agents-village-screensaver/village-screensaver"
FAILS=0
fail() { echo "FAIL: $*"; FAILS=$((FAILS + 1)); }
ok() { echo "ok: $*"; }
finish() { export HOME=$ORIG_HOME PATH=$ORIG_PATH; [ "$FAILS" -eq 0 ] || { echo "$FAILS failed in $(basename "$0")"; exit 1; }; }

sandbox() {
  T=$(mktemp -d)
  export HOME="$T/home"
  mkdir -p "$HOME/.config/omarchy" "$T/bin"
  # Stand-ins that record what was asked of them. `omarchy plugin enable`
  # fails as "not known" while $T/enable-fails counts down (#416), as the real
  # one does until the shell has registered a new plugin.
  printf '#!/bin/sh\necho "omarchy-shell $*" >> "%s/calls"\n' "$T" > "$T/bin/omarchy-shell"
  cat > "$T/bin/omarchy" <<EOF
#!/bin/sh
echo "omarchy \$*" >> "$T/calls"
if [ "\$1 \$2" = "plugin enable" ]; then
  n=\$(cat "$T/enable-fails" 2>/dev/null || echo 0)
  if [ "\$n" -gt 0 ]; then
    echo \$((n - 1)) > "$T/enable-fails"
    echo "omarchy-plugin-enable: plugin '\$3' is not known; run: omarchy-shell shell rescanPlugins" >&2
    exit 1
  fi
fi
EOF
  chmod +x "$T/bin/omarchy" "$T/bin/omarchy-shell"
  export PATH="$T/bin:$ORIG_PATH"
}
ORIG_PATH=$PATH
ORIG_HOME=$HOME


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
remote_sandbox() {
  sandbox
  REMOTE="$T/remote"; mkdir -p "$REMOTE/home/.config/omarchy" "$T/rbin" "$T/omarchy-bin" "$T/sysbin"
  for f in /usr/bin/* /bin/*; do
    n=${f##*/}
    case $n in omarchy*) continue ;; esac
    [ -e "$T/sysbin/$n" ] || [ -L "$T/sysbin/$n" ] || ln -s "$f" "$T/sysbin/$n"
  done
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
  [ -n "\${NO_PROFILE:-}" ] || export OMARCHY_PATH=/fake/omarchy
  exec "$(command -v bash)" -c "\$@"
fi
exec "$(command -v bash)" "\$@"
EOF
  chmod +x "$T/rbin/bash"
  cat > "$T/bin/ssh" <<EOF
#!/bin/sh
# ssh HOST COMMAND: COMMAND in a clean environment on the remote
shift
P="$T/rbin:$T/omarchy-bin:$T/sysbin"; [ -z "\${NO_OMARCHY:-}" ] || P="$T/rbin:$T/sysbin"
cd "$REMOTE/home" && exec env -i HOME="$REMOTE/home" PATH="\$P" \${NO_PROFILE:+NO_PROFILE=1} sh -c "\$1"
EOF
  cat > "$T/bin/scp" <<EOF
#!/bin/sh
# scp -q -p [-r] FILES... HOST:DIR (DIR relative to the remote home)
shift 2; [ "\$1" != -r ] || shift
for last; do :; done
n=\$#; i=0
for f; do i=\$((i+1)); if [ \$i -lt \$n ]; then cp -pR "\$f" "$REMOTE/home/\${last#*:}/"; fi; done
EOF
  # The local omarchy stand-ins must not be what answers.
  rm -f "$T/bin/omarchy" "$T/bin/omarchy-shell"
  chmod +x "$T/bin/ssh" "$T/bin/scp"
  rs="$REMOTE/home/.config/omarchy/shell.json"
  echo '{ "idle": { "screensaver": 90 } }' > "$rs"
}

