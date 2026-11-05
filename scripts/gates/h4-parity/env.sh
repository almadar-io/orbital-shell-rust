# Sourced (zsh) by pair.sh and run_tier.sh. Resolves ROOT, ORB, ORB_DESKTOP, PY, STD, CHROME; exports what children need.
HERE=${${(%):-%x}:A:h}
[ -n "${ROOT:-}" ] || ROOT=${ALMADAR_ROOT:-${HERE:h:h:h:h:h:h}}
export ROOT ALMADAR_ROOT=$ROOT
ORB=${ORB:-orb}
orb_words=(${=ORB})
ORB_BIN=${orb_words[-1]}
ORB_PATH=$(command -v $ORB_BIN) || { echo "h4: orb not found ($ORB_BIN); set \$ORB" >&2; return 1 2>/dev/null || exit 1; }
if [ -z "${ORB_DESKTOP:-}" ]; then ORB_DESKTOP=${ORB_PATH:h}/orb-desktop; fi
[ -x "$ORB_DESKTOP" ] || { echo "h4: orb-desktop not found at $ORB_DESKTOP; set \$ORB_DESKTOP" >&2; return 1 2>/dev/null || exit 1; }
[ -z "${ORBITAL_BIN:-}" ] && [ -x "${ORB_PATH:h}/orbital" ] && export ORBITAL_BIN=${ORB_PATH:h}/orbital
export ORB ORB_DESKTOP
STD=$ROOT/packages/almadar-std/behaviors/lolo
PY=${PARITY_PY:-python3}
$PY -c 'import PIL' 2>/dev/null || { echo "h4: $PY cannot import PIL (Pillow); install it or set \$PARITY_PY" >&2; return 1 2>/dev/null || exit 1; }
export PY STD
CHROME=${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}
export CHROME
SCR=$HERE
orbg() { ${=ORB_GUARD:-} ${=ORB} "$@"; }
