#!/usr/bin/env bash
# M-14 T-04 (e T-06, prima metà) — llama-bench di una build del fork contro la build di ggml-org.
#
# Stessi parametri di M-08 T-02 (run-T02-banda.sh): denso 8B e MoE 35B, tutto sulla GPU, flash
# attention, 12 thread, 5 ripetizioni, prefill 512, decode 128 e decode con 4096 token in cache.
# Le build si alternano modello per modello (A, B, A, B) così una deriva termica non cade tutta
# su una delle due. llama-bench scrive lo stesso commit per moro0 e per ggml-org: per questo ogni
# build ha il suo file, T-04-<nome>.jsonl.
#
# Uso: run-T04-banda.sh <nome-A> <cartella-A> <nome-B> <cartella-B> [uscita]
#   per esempio  run-T04-banda.sh ggml-org "$AETHERA_RADICE/builds/llama-b10991-win-vulkan-x64" \
#                               moro0 "$AETHERA_RADICE/builds/llama-b10991+moro0-win-vulkan-x64"

set -u
. "$(dirname "$0")/../percorsi.sh"
MODELS="${MODELS:-${AETHERA_PESI:?imposta AETHERA_PESI}}"
NAME_A="${1:?nome della build A}"; BUILD_A="${2:?cartella della build A}"
NAME_B="${3:?nome della build B}"; BUILD_B="${4:?cartella della build B}"
OUT="${5:-$AETHERA_RADICE/m14}"
PREFIX="${PREFIX:-T-04}"
mkdir -p "$OUT"

DENSE="$MODELS/Qwen3-8B-Q4_K_M.gguf"
MOE="$MODELS/Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf"

free_gib() {
  powershell.exe -NoProfile -Command "[math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1MB,1)" | tr -d '\r' | tr ',' '.'
}

bench() { # nome cartella modello
  local name="$1" build="$2" model="$3"
  local free; free=$(free_gib)
  if awk "BEGIN{exit !($free < 16)}"; then
    echo "RAM libera $free GiB sotto 16: mi fermo" >&2; exit 3
  fi
  echo "=== $name · $(basename "$model" .gguf) · RAM libera $free GiB · $(date +%H:%M:%S) ==="
  "$build/llama-bench.exe" -m "$model" -ngl 999 -fa 1 -t 12 -r 5 -p 512 -n 128 -pg 4096,128 \
    -o jsonl >> "$OUT/$PREFIX-$name.jsonl" 2>> "$OUT/$PREFIX-$name.err"
  tail -3 "$OUT/$PREFIX-$name.jsonl" | sed -E 's/.*"n_prompt": ([0-9]+), "n_gen": ([0-9]+).*"avg_ts": ([0-9.]+), "stddev_ts": ([0-9.]+).*/  p\1 g\2: \3 ± \4 tok\/s/'
}

# MODELLI=moe (o denso) per ripetere un modello solo, per esempio a ordine invertito.
case "${MODELLI:-tutti}" in
  denso) LIST=("$DENSE") ;;
  moe) LIST=("$MOE") ;;
  *) LIST=("$DENSE" "$MOE") ;;
esac
for m in "${LIST[@]}"; do
  bench "$NAME_A" "$BUILD_A" "$m"
  bench "$NAME_B" "$BUILD_B" "$m"
done
echo "righe in $OUT/$PREFIX-$NAME_A.jsonl e $OUT/$PREFIX-$NAME_B.jsonl"
