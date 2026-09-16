#!/bin/bash
# M-11: a download finito installa i profili, legge i metadati, fa la sonda di coerenza e poi T-03.
set -u
M11=/c/Git/Aethera/.lmbrain-lite/m11; OUT=/c/AetheraData/m11; EX=/c/Git/Aethera/src-tauri/target/release/examples
mkdir -p $OUT
until grep -q FINITO /c/AetheraData/m10/scarica-coder.log; do sleep 30; done
for q in q4_k_m q8_0; do
  f=/c/Git/minis-config/models/qwen3.8-flash-coder-*-$q.gguf
  if ls $f >/dev/null 2>&1; then cp -n $M11/profilo-flash-coder.$q.toml /c/AetheraData/profiles/qwen3.8-flash-coder.$q.vulkan.toml; fi
done
"$EX/gguf_dump.exe" $(cygpath -w /c/Git/minis-config/models/qwen3.8-flash-coder-26gb-q4_k_m.gguf) > $OUT/T-02-gguf.txt 2>&1
sleep 30
for q in q4_k_m q8_0; do
  [ -f /c/AetheraData/profiles/qwen3.8-flash-coder.$q.vulkan.toml ] || { echo "manca il profilo $q"; continue; }
  rm -f /c/AetheraData/stop-m08
  "$EX/m08_hold.exe" 'C:\AetheraData' qwen3.8-flash-coder.$q.vulkan > $OUT/T-02-hold-$q.log 2>&1 &
  for i in $(seq 1 120); do curl -sf -m 2 http://127.0.0.1:8080/health >/dev/null && break; sleep 5; done
  echo "=== coerenza $q $(date +%T)"
  python "$(cygpath -w $M11/coerenza.py)" qwen3.8-flash-coder.$q.vulkan "$(cygpath -w $OUT/T-02-$q.jsonl)"
  touch /c/AetheraData/stop-m08; wait; rm -f /c/AetheraData/stop-m08
  sleep 30
done
stop=$OUT/stop-T03; rm -f $stop
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M11/campiona-memoria.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 10800 -Out 'C:\AetheraData\m11\T-03-memoria.csv' &
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M11/sorveglia-ram.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 10800 -MinGiB 2 -Log 'C:\AetheraData\m11\sorveglia.log' &
sleep 5
echo "=== T-03 $(date +%T)"
"$EX/m08_bench.exe" 'C:\AetheraData' "$(cygpath -w $M11/T-03-vgm48.toml)" 2>&1
touch $stop; wait
echo "M-11 T-03 FINITO $(date +%T)"
