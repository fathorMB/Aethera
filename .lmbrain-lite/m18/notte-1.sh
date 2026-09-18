#!/bin/bash
# M-18, notte del 18-09, nell'ordine scelto dall'operatore dopo il parere del critico:
#   sentinella · T-12a speculativa a n-grammi sul G3 · sentinella · T-12b MTP e n-grammi sul G1 ·
#   sentinella · T-09 costo per turno a ~64k e ~128k (G1 con MTP, G1 senza, G3; alternati) ·
#   sentinella · T-13 KLD di Q4_K_M e Q6_K del G1 contro Q8_0 (se i download sono finiti) · sentinella
# Parte dopo la coda T-03 di M-17 e dopo i download (li aspetta al massimo 90 minuti, poi va avanti
# lo stesso e lo scrive). Ogni passo aspetta da solo che il motore sia libero.
set -u
. "$(dirname "$0")/../percorsi.sh"
QUI="$(cd "$(dirname "$0")" && pwd)"
R=$(cygpath -u "$AETHERA_RADICE"); RW=$(cygpath -w "$AETHERA_RADICE"); P=$(cygpath -u "${AETHERA_PESI:?}")
OUT=$R/m18; mkdir -p $OUT
BENCH=$(cygpath -u "$AETHERA_REPO")/src-tauri/target/release/examples/m08_bench.exe
S="$(cygpath -w "$QUI/../m17/scenario.py")"
G1=qwen3.6-35b-a3b.q4_k_m.vulkan; G3=qwen3-coder-next.q4_k_m.vulkan

until grep -q "M-17 T-03 FINITO" $R/m17/T-03.out 2>/dev/null; do sleep 30; done
for i in $(seq 1 180); do grep -q "DOWNLOAD G1 FINITO" $OUT/scarica-g1.log 2>/dev/null && break; sleep 30; done
grep -q "DOWNLOAD G1 FINITO" $OUT/scarica-g1.log 2>/dev/null || echo "ATTENZIONE: download ancora in corso, le misure partono lo stesso $(date +%T)"

bash "$QUI/sentinella.sh" inizio-notte
for s in T-12a-spec-g3 T-12b-mtp-g1; do
  echo "=== $s $(date +%T)"
  "$BENCH" "$RW" "$(cygpath -w "$QUI/$s.toml")" 2>&1
  sleep 30
  bash "$QUI/sentinella.sh" dopo-$s
done

# T-09: profilo di prova del G1 senza speculazione (copia del profilo standard, solo il tipo cambia).
NOSPEC=$R/profiles/$G1.nospec.toml
if [ ! -f "$NOSPEC" ]; then
  python - "$R/profiles/$G1.toml" "$NOSPEC" <<'PY'
import re, sys
t = open(sys.argv[1], encoding="utf-8").read()
t = t.replace('name = "qwen3.6-35b-a3b.q4_k_m.vulkan"', 'name = "qwen3.6-35b-a3b.q4_k_m.vulkan.nospec"', 1)
t = re.sub(r'\[speculative\][^\[]*', '[speculative]\ntype = "none"\n\n', t, count=1)
t = re.sub(r'^gate = .*\n', '', t, count=1, flags=re.M)
open(sys.argv[2], "w", encoding="utf-8", newline="\n").write("# M-18 T-09: profilo di prova, G1 senza speculazione.\n" + t)
PY
fi
python "$(cygpath -w "$QUI/sorgente-lungo.py")" --caratteri 1000000
L="$RW\m18\sorgente-lungo.txt"
for prof in 210000:64k 400000:128k; do
  car=${prof%%:*}; tag=${prof##*:}
  python "$S" --misura T-09 --profilo $G1        --nome g1-mtp-$tag    --sorgente "$L" --prefisso $car --ctx 163840 --giri 1 --turni 10 --n-predict 128 --senza-thinking
  python "$S" --misura T-09 --profilo $G1.nospec --nome g1-nospec-$tag --sorgente "$L" --prefisso $car --ctx 163840 --giri 1 --turni 10 --n-predict 128 --senza-thinking
  python "$S" --misura T-09 --profilo $G3        --nome g3-$tag        --sorgente "$L" --prefisso $car --ctx 163840 --giri 1 --turni 10 --n-predict 128
  bash "$QUI/sentinella.sh" dopo-T-09-$tag
done

# T-13: KLD dei quant del G1 contro Q8_0 (metodo di M-16: prompt congelato 21k, contesto 8192).
PPL="$R/builds/llama-b10991+moro0-win-vulkan-x64/llama-perplexity.exe"
Q8=$P/Qwen_Qwen3.6-35B-A3B-Q8_0.gguf; Q6=$P/Qwen_Qwen3.6-35B-A3B-Q6_K.gguf; Q4=$P/Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf
if [ -f "$Q8" ] && [ -f "$Q6" ]; then
  while tasklist | grep -qi "llama-server\|m08_bench\|m15_hold"; do sleep 30; done
  comuni="-ngl 999 -fa on -c 8192 -ub 4096 -b 4096 -ctk f16 -ctv f16"
  echo "=== T-13 base Q8_0 $(date +%T)"
  "$PPL" -m "$(cygpath -w $Q8)" $comuni -f "$(cygpath -w "$QUI/prompt-21k.txt")" --kl-divergence-base "$RW\m18\T-13-q8.kld" > $OUT/T-13-q8-base.txt 2>&1
  for q in q8-ripetuta:$Q8 q6_k:$Q6 q4_k_m:$Q4; do
    n=${q%%:*}; f=${q#*:}
    echo "=== T-13 $n $(date +%T)"
    "$PPL" -m "$(cygpath -w $f)" $comuni --kl-divergence-base "$RW\m18\T-13-q8.kld" --kl-divergence > $OUT/T-13-$n.txt 2>&1
  done
  rm -f $OUT/T-13-q8.kld
  bash "$QUI/sentinella.sh" dopo-T-13
else
  echo "T-13 saltato: mancano i pesi Q8_0 o Q6_K"
fi
echo "M-18 NOTTE-1 FINITA $(date +%T)"
