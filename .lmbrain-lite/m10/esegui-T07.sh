#!/bin/bash
# M-10 T-07: VGM 64, mmap + lazy auto, con campionatore e sorvegliante (RAM disponibile, file di paging).
set -u
M10=/c/Git/Aethera/.lmbrain-lite/m10; OUT=/c/AetheraData/m10; BENCH=/c/Git/Aethera/src-tauri/target/release/examples/m08_bench.exe
stop=$OUT/stop-T07; rm -f $stop
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/campiona-memoria.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 10800 -Out 'C:\AetheraData\m10\T-07-memoria.csv' &
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/sorveglia-ram.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 10800 -MinGiB 2 -MaxPagefileMB 4096 -Log 'C:\AetheraData\m10\sorveglia-T07.log' &
sleep 5
echo "=== T-07 $(date +%T)"
"$BENCH" 'C:\AetheraData' "$(cygpath -w $M10/T-07-vgm64.toml)" 2>&1
touch $stop; wait
echo "T-07 FINITO $(date +%T)"
