#!/bin/bash
# M-11 T-04 a VGM 48: contesto lungo (30k e 60k, solo Q4_K_M) e riuso del prefisso (Q4_K_M e Q8_0).
# Sorvegliante: RAM libera sotto 2 GiB, file di paging +2 GiB o motore non pronto dopo 300 s.
set -u
. "$(dirname "$0")/../percorsi.sh"
R=$(cygpath -u "$AETHERA_RADICE"); RW=$(cygpath -w "$AETHERA_RADICE")
M10=$(cygpath -u "$AETHERA_REPO")/.lmbrain-lite/m10; M11=$(cygpath -u "$AETHERA_REPO")/.lmbrain-lite/m11
OUT=$R/m11; BENCH=$(cygpath -u "$AETHERA_REPO")/src-tauri/target/release/examples/m08_bench.exe
for s in T-04a-contesto T-04b-riuso; do
  m=${s%%-[a-z]*}
  stop=$OUT/stop-$m; rm -f $stop
  powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/campiona-memoria.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 14400 -Out "$RW/m11/$m-memoria.csv" &
  powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/sorveglia-ram.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 14400 -MinGiB 2 -MaxPagefileMB 2048 -MaxLoadSeconds 300 -Log "$RW/m11/sorveglia-$m.log" &
  sleep 5
  echo "=== $m $(date +%T)"
  "$BENCH" "$RW" "$(cygpath -w $M11/$s.toml)" 2>&1
  touch $stop; wait
  sleep 60
done
echo "M-11 T-04 FINITO $(date +%T)"
