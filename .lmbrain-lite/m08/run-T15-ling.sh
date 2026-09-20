#!/usr/bin/env bash
# M-19 T-15 — la banda utile di Ling-3.0-flash Q3_K_M, e la scala del contesto.
#
# Verifica una previsione fatta PRIMA di scaricare i pesi, dall'intestazione GGUF (m08_bytes):
#   byte per token  3,393 GB  (G1 Q8: 3,276 GB)  ->  decode atteso 14-18 tok/s
# Se il numero cade fuori, e' il modello della macchina a essere sbagliato, non il modello.
#
# Il G1 Q8 viene rimisurato nello stesso giro: i numeri del 18-09 sono di un'altra sera, e due
# avvii con condizioni diverse non si confrontano (regola della pagina degli avvii).
#
# ATTENZIONE: llama-bench prende la GPU tutta. Lo script aspetta che nessun llama-server giri.

set -u
. "$(dirname "$0")/../percorsi.sh"
BUILD="${BUILD:-${AETHERA_BUILD:?imposta AETHERA_BUILD}}"
MODELS="${MODELS:-${AETHERA_PESI:?imposta AETHERA_PESI}}"
OUT="${1:-$AETHERA_RADICE/m19}"
mkdir -p "$OUT"

BENCH="$BUILD/llama-bench.exe"
LING="$MODELS/Ling-3.0-flash-Q3_K_M-00001-of-00002.gguf"
G1="$MODELS/Qwen_Qwen3.6-35B-A3B-Q8_0.gguf"

# Aspetta che il motore sia libero: qui non si interrompe il lavoro di nessuno.
atteso=0
while tasklist 2>/dev/null | grep -qi "llama-server"; do
  [ $atteso -eq 0 ] && echo "motore occupato: aspetto che si liberi"
  atteso=$((atteso+1)); sleep 60
  [ $atteso -gt 240 ] && { echo "quattro ore di attesa: rinuncio"; exit 1; }
done
[ $atteso -gt 0 ] && echo "motore libero dopo $atteso minuti"

for m in "$LING" "$G1"; do
  name=$(basename "$m" .gguf)
  echo "=== $name ==="
  # Tre giri e non cinque: la domanda e' un ordine di grandezza, e ogni avvio qui costa minuti.
  # -p 512 prefill, -n 128 decode, -pg 4096,128 il decode con 4096 token gia' in cache.
  "$BENCH" -m "$m" -ngl 999 -fa 1 -t 12 -r 3 -p 512 -n 128 -pg 4096,128 \
    -o jsonl >> "$OUT/T-15-llama-bench.jsonl" 2>> "$OUT/T-15-llama-bench.err"
  tail -4 "$OUT/T-15-llama-bench.jsonl"
done

# La scala del contesto: il decode a profondita' crescente. Con 5:1 KDA/MLA la KV dovrebbe
# crescere molto meno che su un'attenzione piena. La VRAM per contesto NON si misura qui
# (serve un avvio da Aethera, che legge [memory.after_load]): resta per una sessione a parte.
echo "=== scala del contesto su Ling ==="
"$BENCH" -m "$LING" -ngl 999 -fa 1 -t 12 -r 3 -n 128 -d 32768,131072,262144 \
  -o jsonl >> "$OUT/T-15-ctx.jsonl" 2>> "$OUT/T-15-ctx.err"
tail -6 "$OUT/T-15-ctx.jsonl"

echo "righe in $OUT/T-15-llama-bench.jsonl e $OUT/T-15-ctx.jsonl"
