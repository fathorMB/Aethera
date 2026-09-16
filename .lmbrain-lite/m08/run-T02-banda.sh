#!/usr/bin/env bash
# M-08 T-02 — banda raggiungibile dalla 890M.
#
# llama-bench in decode (tg) e prefill (pp) su un denso piccolo e sul MoE, stessa build, stesso
# backend, cinque ripetizioni. La banda utile si ottiene moltiplicando i tok/s per i byte che il
# motore legge davvero a ogni token, calcolati da m08_bytes dall'intestazione GGUF.
#
# Il denso dice il tetto della piattaforma: legge tutti i suoi pesi a ogni token, senza router.
# Se il denso arriva vicino ai 60-67 GB/s teorici della LPDDR5X-8000 e il MoE resta molto sotto,
# il divario è nei kernel; se restano vicini, è la piattaforma.

set -u
. "$(dirname "$0")/../percorsi.sh"
BUILD="${BUILD:-${AETHERA_BUILD:?imposta AETHERA_BUILD}}"
MODELS="${MODELS:-${AETHERA_PESI:?imposta AETHERA_PESI}}"
OUT="${1:-$AETHERA_RADICE/m08}"
mkdir -p "$OUT"

BENCH="$BUILD/llama-bench.exe"
DENSE="$MODELS/Qwen3-8B-Q4_K_M.gguf"
MOE="$MODELS/Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf"

for m in "$DENSE" "$MOE"; do
  name=$(basename "$m" .gguf)
  echo "=== $name ==="
  # -p 512 prefill, -n 128 decode; -d 4096 ripete il decode con 4096 token già in cache,
  # perché il decode a contesto vuoto non è il decode che si vive.
  "$BENCH" -m "$m" -ngl 999 -fa 1 -t 12 -r 5 -p 512 -n 128 -pg 4096,128 \
    -o jsonl >> "$OUT/T-02-llama-bench.jsonl" 2>> "$OUT/T-02-llama-bench.err"
  tail -4 "$OUT/T-02-llama-bench.jsonl"
done
echo "righe in $OUT/T-02-llama-bench.jsonl"
