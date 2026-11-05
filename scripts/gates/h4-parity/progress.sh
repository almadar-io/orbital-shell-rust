#!/bin/zsh
# progress.sh <outdir> [step]: print a line every <step> paired captures and on DONE.
L=$1/run.log; STEP=${2:-100}; last=0
while true; do
  n=$(grep -c "delta=" $L 2>/dev/null)
  f=$(grep -cE "_FAIL" $L 2>/dev/null)
  if [ $(( n / STEP )) -gt $(( last / STEP )) ]; then echo "pairs=$n fails=$f $(date +%H:%M)"; last=$n; fi
  if grep -q DONE $L; then echo "DONE pairs=$n fails=$f"; break; fi
  sleep 20
done
