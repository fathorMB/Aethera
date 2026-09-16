#!/usr/bin/env bash
# M-08 T-06, seconda metà — la coerenza dell'output con la cache KV quantizzata.
#
# Confrontare il testo generato a temperatura 0 non basta: su Vulkan due giri identici danno già
# testi diversi (non determinismo del backend, visto in T-04). Serve un numero che non dipenda dal
# campionamento, e quello è la perplessità: stesso testo, stesso contesto, si cambia solo il tipo
# della cache KV. Se q8_0 costasse qualità, la perplessità salirebbe.
#
# Il testo è il prompt congelato da 21k token: codice vero di questo repository, non wikitext, così
# la misura parla del carico che questa macchina vive davvero.

set -u
BUILD="${BUILD:-C:/Nonio/llama-b10809-vulkan}"
MODEL="${MODEL:-C:/Git/minis-config/models/Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf}"
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${1:-C:/AetheraData/m08}"
mkdir -p "$OUT"
RES="$OUT/T-06-perplexity.txt"
: > "$RES"

for kv in f16 q8_0; do
  echo "--- cache KV $kv ---" | tee -a "$RES"
  "$BUILD/llama-perplexity.exe" -m "$MODEL" -f "$HERE/prompt-21k.txt" \
    -ngl 999 -fa on -c 8192 -ub 2048 -b 2048 -ctk "$kv" -ctv "$kv" \
    >> "$RES" 2>&1
  grep -E "^Final estimate|^\[[0-9]+\]" "$RES" | tail -3
done
echo "risultati in $RES"
