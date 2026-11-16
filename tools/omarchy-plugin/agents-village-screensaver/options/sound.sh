# --sound: quiet sound cues on transitions (#489): a bell when something
# starts waiting on you, thunder and birdsong as the build breaks and mends, a
# horn when a deploy reaches production. Silent in the dim hours. Off by
# default. The page reads it as `sound=1` (soundcues.js).
saver_option sound --sound false flag '^(true|false)$' "" \
  "quiet sound cues on transitions (default: off; see features/sound.md)" \
  "--sound takes no value"

saver_query_sound() { [ "$1" = true ] && echo "sound=1"; return 0; }
# Chromium holds audio until a click; a screensaver never gets one.
saver_browser_sound() { [ "$1" = true ] && echo "--autoplay-policy=no-user-gesture-required"; return 0; }
