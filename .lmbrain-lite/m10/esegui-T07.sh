#!/bin/bash
# M-10 T-07: VGM 64, mmap + lazy auto, con campionatore e sorvegliante (RAM disponibile, file di paging).
set -u
. "$(dirname "$0")/../percorsi.sh"
R=$(cygpath -u "$AETHERA_RADICE"); RW=$(cygpath -w "$AETHERA_RADICE")
P=$(cygpath -u "${AETHERA_PESI:-.}"); D=$(cygpath -u "${AETHERA_DOWNLOAD:-.}")
M10=$(cygpath -u "$AETHERA_REPO")/.lmbrain-lite/m10; OUT=$R/m10; BENCH=$(cygpath -u "$AETHERA_REPO")/src-tauri/target/release/examples/m08_bench.exe
stop=$OUT/stop-T07; rm -f $stop
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/campiona-memoria.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 10800 -Out "$RW\m10\T-07-memoria.csv" &
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/sorveglia-ram.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 10800 -MinGiB 2 -MaxPagefileMB 4096 -Log "$RW\m10\sorveglia-T07.log" &
sleep 5
echo "=== T-07 $(date +%T)"
"$BENCH" "$RW" "$(cygpath -w $M10/T-07-vgm64.toml)" 2>&1
touch $stop; wait
echo "T-07 FINITO $(date +%T)"
