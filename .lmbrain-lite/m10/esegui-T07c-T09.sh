#!/bin/bash
# M-10 a VGM 64: T-07c (Q3_K_XL), T-08 (contesto lungo), T-09 (riuso), tutti con mmap + lazy auto.
# Sorvegliante: file di paging +2 GiB o motore non pronto dopo 300 s (scelta dell'operatore del 16-09).
set -u
M10=/c/Git/Aethera/.lmbrain-lite/m10; OUT=/c/AetheraData/m10; BENCH=/c/Git/Aethera/src-tauri/target/release/examples/m08_bench.exe
for s in T-07c-q3 T-08-contesto T-09-riuso; do
  m=${s%%-[a-z]*}
  stop=$OUT/stop-$m; rm -f $stop
  powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/campiona-memoria.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 14400 -Out "C:/AetheraData/m10/$m-memoria.csv" &
  powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/sorveglia-ram.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 14400 -MinGiB 0 -MaxPagefileMB 2048 -MaxLoadSeconds 300 -Log "C:/AetheraData/m10/sorveglia-$m.log" &
  sleep 5
  echo "=== $m $(date +%T)"
  "$BENCH" 'C:\AetheraData' "$(cygpath -w $M10/$s.toml)" 2>&1
  touch $stop; wait
  sleep 60
done
echo "VGM64 FINITO $(date +%T)"
