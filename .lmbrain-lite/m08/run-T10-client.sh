#!/usr/bin/env bash
# M-08 T-10 — quanto prefisso conserva un client fra un turno e l'altro.
#
# Il motore resta acceso e fermo; fra il client e il motore si mette il ponte `prefix_proxy.py`,
# che per ogni richiesta scrive quanto del prompt precedente e' sopravvissuto. Un client che
# riscrive l'inizio del prompt a ogni turno (un'ora, un identificativo, i messaggi riordinati)
# azzera la cache del motore, e su questa macchina la cache del prefisso vale piu' del decode.
#
# Uso: bash run-T10-client.sh <etichetta> <comando del client…>
# Il client va configurato per parlare con http://127.0.0.1:8081.

set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
LABEL="${1:?etichetta del client}"; shift
OUT="${OUT:-C:/AetheraData/m08/T-10-prefissi.jsonl}"

python "$HERE/prefix_proxy.py" --port 8081 --upstream http://127.0.0.1:8080 \
  --label "$LABEL" --out "$OUT" ${DUMP:+--dump "$DUMP"} ${FOLD_SYSTEM:+--fold-system} &
PROXY=$!
sleep 2
echo "--- $LABEL ---"
"$@"
STATUS=$?
sleep 1
kill $PROXY 2>/dev/null
echo "$LABEL: uscita $STATUS · righe in $OUT"
