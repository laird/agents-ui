# --cycle S: seconds on each swarm when there are several (#382).
# 0 stays on one. The page reads it as `cycle=` (saver.js, saverOptions).
saver_option cycle --cycle 30 number '^(0|[1-9][0-9]*)$' S \
  "seconds on each swarm when there are several (default 30, 0 stays)" \
  "--cycle takes whole seconds"
