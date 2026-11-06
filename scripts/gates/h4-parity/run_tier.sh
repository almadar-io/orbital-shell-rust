#!/bin/zsh
# run_tier.sh <list-file> <outdir> [workers] [themes-file]
# Every behavior x theme through pair.sh. Work is split into `workers` shards; each worker runs its shard in order on
# its own serve port and keeps one headless Chrome (its own DevTools port) for all of its web captures.
SCR=${0:A:h}
. $SCR/env.sh || exit 2
LIST=$1; OUT=$2; WORKERS=${3:-4}; THEMES=${4:-$SCR/themes.txt}
PORT_BASE=${H4_PORT_BASE:-4911}; CDP_BASE=${H4_CDP_BASE:-9300}
mkdir -p $OUT/shards
touch $OUT/run.log
i=0
rm -f $OUT/shards/*(N)
for b in ${(f)"$(cat $LIST)"}; do
  for t in ${(f)"$(cat $THEMES)"}; do
    echo "$b $t" >> $OUT/shards/$(( i % WORKERS ))
    i=$(( i + 1 ))
  done
done
echo "jobs=$i workers=$WORKERS" >> $OUT/run.log
for w in {0..$(( WORKERS - 1 ))}; do
  [ -f $OUT/shards/$w ] || continue
  (
    port=$(( PORT_BASE + w * 3 ))
    cdp=$(( CDP_BASE + w ))
    "$CHROME" --headless=new --hide-scrollbars --remote-debugging-port=$cdp --user-data-dir=$OUT/chrome-$w \
      --window-size=1280,900 --no-first-run --no-default-browser-check about:blank > /dev/null 2>&1 &
    cpid=$!
    while read -r b t; do
      CDP_REUSE=1 CDP_PORT=$cdp zsh $SCR/pair.sh $b $t $port $OUT >> $OUT/run.log 2>&1
    done < $OUT/shards/$w
    kill $cpid 2>/dev/null
    wait $cpid 2>/dev/null
    rm -rf $OUT/chrome-$w $ROOT/.parity-serve/${OUT:t}-$port
  ) &
done
wait
echo "DONE" >> $OUT/run.log
