#!/bin/bash
# M-10 T-07: VGM 64, mmap + lazy auto, con campionatore e sorvegliante (RAM disponibile, file di paging).
set -u
. "$(dirname "$0")/../percorsi.sh"
R=$(cygpath -u "$AETHERA_RADICE"); RW=$(cygpath -w "$AETHERA_RADICE")
P=$(cygpath -u "${AETHERA_PESI:-.}"); D=$(cygpath -u "${AETHERA_DOWNLOAD:-.}")
M10=$(cygpath -u "$AETHERA_REPO")/.lmbrain-lite/m10; OUT=$R/m10; BENCH=$(cygpath -u "$AETHERA_REPO")/src-tauri/target/release/examples/m08_bench.exe
stop=$OUT/stop-T07b; rm -f $stop
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/campiona-memoria.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 10800 -Out "$RW\m10\T-07b-memoria.csv" &
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/sorveglia-ram.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 10800 -MinGiB 0 -MaxPagefileMB 2048 -MaxLoadSeconds 300 -Log "$RW\m10\sorveglia-T07b.log" &
sleep 5
echo "=== T-07b $(date +%T)"
"$BENCH" "$RW" "$(cygpath -w $M10/T-07b-senza-soglia.toml)" 2>&1
touch $stop; wait
echo "T-07b FINITO $(date +%T)"
