#!/bin/zsh
# pair.sh <behavior.lolo (relative to std behaviors/lolo)> <theme> <port> <outdir>
# Same program, same declared theme, same running server, two clients: web (headless Chrome) and desktop
# (orb-desktop Screenshot with route + server), each opening the program's first route. Resumable: a finished pair is skipped.
. ${0:A:h}/env.sh || exit 2
REL=$1; THEME=$2; PORT=$3; OUT=$4
NAME=${REL:t:r}
[ -f $OUT/$NAME--$THEME.pair.png ] && exit 0
free_gb=$(df -g $OUT:h | awk 'NR==2{print $4}')
[ ${free_gb:-0} -ge ${H4_MIN_FREE_GB:-50} ] || { echo "DISK_LOW ${free_gb}GB free < ${H4_MIN_FREE_GB:-50}GB: refusing $NAME $THEME"; exit 5; }
mkdir -p $OUT/src $OUT/log
SRC=$OUT/src/$THEME/$NAME.lolo
$PY $SCR/themed.py $STD/$REL $THEME $SRC || { echo "RESOLVE_FAIL $NAME $THEME"; exit 3; }
cd $ROOT
orbg resolve $SRC -o $OUT/src/$NAME--$THEME.orb > $OUT/log/$NAME--$THEME.resolve.log 2>&1 || { echo "RESOLVE_FAIL $NAME $THEME"; exit 4; }
ROUTE=$($PY -c "
import json,sys
d=json.load(open('$OUT/src/$NAME--$THEME.orb'))
paths=[p.get('path') for o in d['orbitals'] for p in o.get('pages',[]) if p.get('path') and not p.get('protocol')]
print(paths[0] if paths else '/')")
# orb resolves behavior packages by walking up from its cwd: serve from inside ROOT so the web leg compiles the same
# std the desktop leg resolved (a cwd outside ROOT silently picks the installed std).
SERVE_DIR=$ROOT/.parity-serve/${OUT:t}-$PORT
mkdir -p $SERVE_DIR && cd $SERVE_DIR
ORB_GUARD_SECS=${ORB_GUARD_SECS:-600} orbg serve $SRC --port $PORT > $OUT/log/$NAME--$THEME.serve.log 2>&1 &
SPID=$!
up=0
for i in {1..180}; do
  if curl -sf -o /dev/null http://127.0.0.1:$PORT/; then up=1; break; fi
  kill -0 $SPID 2>/dev/null || break
  sleep 1
done
if [ $up = 1 ]; then
  $PY $SCR/desktop_shot.py $OUT/src/$NAME--$THEME.orb $NAME--$THEME.desktop $OUT "$ROUTE" "http://127.0.0.1:$PORT" > $OUT/log/$NAME--$THEME.desktop.log 2>&1 || echo "DESKTOP_FAIL $NAME $THEME"
  node $SCR/cdp_shot.mjs "http://127.0.0.1:$PORT$ROUTE" $OUT/$NAME--$THEME.web.png $SERVE_DIR/chrome ${CDP_PORT:-$(( PORT + 1000 ))} > $OUT/log/$NAME--$THEME.chrome.log 2>&1
else
  echo "SERVE_FAIL $NAME $THEME"
fi
pkill -P $SPID 2>/dev/null; kill $SPID 2>/dev/null
for pid in $(lsof -ti tcp:$PORT -sTCP:LISTEN); do kill $pid; done
wait $SPID 2>/dev/null
# orb serve scaffolds a whole app (~100 MB) per program under .orbital/serve and keeps it: drop it with the pair.
rm -rf $SERVE_DIR/.orbital/serve/$NAME
$PY $SCR/composite.py $OUT/$NAME--$THEME.web.png $OUT/$NAME--$THEME.desktop.png $OUT/$NAME--$THEME.pair.png \
  && rm -f $OUT/src/$NAME--$THEME.orb
