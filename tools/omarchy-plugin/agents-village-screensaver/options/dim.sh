# --dim H-H: dim the display between these hours, e.g. 23-7 (#382), for
# burn-in. Empty never dims. The page reads it as `dim=` (saver.js).
saver_option dim --dim "" string '^(([01]?[0-9]|2[0-3])-([01]?[0-9]|2[0-3]))?$' H-H \
  "dim between these hours, e.g. 23-7 (default: never dims)" \
  "--dim takes hours like 23-7"
