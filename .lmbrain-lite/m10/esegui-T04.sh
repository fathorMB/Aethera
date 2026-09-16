#!/bin/bash
# M-10 T-04: a download finito collega Q3_K_XL, poi misura IQ3_XXS con mmap (sorvegliato) e con none.
set -u
M10=/c/Git/Aethera/.lmbrain-lite/m10; OUT=/c/AetheraData/m10; BENCH=/c/Git/Aethera/src-tauri/target/release/examples/m08_bench.exe
until grep -q FINITO $OUT/scarica-pesi.log; do sleep 30; done
if [ "$(grep -c '^OK .*Q3_K_XL' $OUT/scarica-pesi.log)" = 2 ]; then
  cd /c/Git/minis-config/models
  for f in /c/models/UD-Q3_K_XL/*.gguf; do n=$(basename "$f"); [ -e "$n" ] || cmd //c mklink //H "$n" "$(cygpath -w "$f")"; done
  cp -n $M10/profilo-flash-next.q3_k_xl.toml /c/AetheraData/profiles/qwen3.8-flash-next.q3_k_xl.vulkan.toml
fi
sleep 60   # il disco si calma dopo l'hash
for m in mmap none; do
  stop="$OUT/stop-$m"; rm -f "$stop"
  powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/campiona-memoria.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 7200 -Out "C:\AetheraData\m10\T-04-$m-memoria.csv" &
  [ $m = mmap ] && powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w $M10/sorveglia-ram.ps1)" -StopFile "$(cygpath -w $stop)" -Seconds 7200 -MinGiB 2 -Log "C:\AetheraData\m10\sorveglia.log" &
  sleep 5
  echo "=== T-04-$m $(date +%T)"
  "$BENCH" 'C:\AetheraData' "$(cygpath -w $M10/T-04-$m.toml)" 2>&1
  touch "$stop"; wait
  sleep 30
done
echo "T-04 FINITO $(date +%T)"
