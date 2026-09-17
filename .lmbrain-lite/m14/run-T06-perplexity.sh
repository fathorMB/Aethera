#!/usr/bin/env bash
# M-14 T-06, coerenza — perplessità della build patchata contro la base, stesso testo.
#
# Come M-08 T-06 (run-T06-perplexity.sh di m08): il testo generato a temperatura 0 non basta,
# perché su Vulkan due giri identici divergono già fra loro. La perplessità non dipende dal
# campionamento: se lo shader int8 cambiasse i numeri in modo che conta, salirebbe.
# Testo: il prompt congelato da 21k token (codice vero di Aethera), contesto 8192, cache KV f16,
# ubatch e batch 4096 come il profilo G1 (UB e BATCH per cambiarli: il Coder-Next G3 usa 512 e 2048).
#
# Uso: run-T06-perplexity.sh <nome> <cartella> [<nome> <cartella> ...]

set -u
. "$(dirname "$0")/../percorsi.sh"
MODEL="${MODEL:-${AETHERA_PESI:?imposta AETHERA_PESI}/Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf}"
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${OUT:-$AETHERA_RADICE/m14}"
mkdir -p "$OUT"
RES="$OUT/${RES_NAME:-T-06-perplexity.txt}"

while [ $# -ge 2 ]; do
  name="$1"; build="$2"; shift 2
  echo "--- $name · $(basename "$MODEL" .gguf) · $(date +%H:%M:%S) ---" | tee -a "$RES"
  "$build/llama-perplexity.exe" -m "$MODEL" -f "$HERE/../m08/prompt-21k.txt" \
    -ngl 999 -fa on -c 8192 -ub "${UB:-4096}" -b "${BATCH:-4096}" -ctk f16 -ctv f16 \
    >> "$RES" 2>&1
  grep -E "^Final estimate" "$RES" | tail -1
done
echo "risultati in $RES"
